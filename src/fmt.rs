//! Canonical source formatter (SPEC §14). Parses to the AST and re-emits a
//! normalized form: the three phases in order (the stylesheet `{ }`, then the
//! instances, then the wires), `{ }` style blocks and `[ ]` child lists, bar-wrapped
//! type selectors and `|name::base|` defines, 2-space indent, space-separated value
//! groups. Comments and blank-line groupings are preserved; a group of plain
//! sibling nodes aligns its id and type columns. Idempotent: `fmt(fmt(x)) == fmt(x)`.

use crate::ast::{Side, WireOp};
use crate::error::Error;
use crate::lexer;
use crate::span::Span;
use crate::syntax::ast::{
    Child, Decl, Define, Endpoint, File, Node, Rule, SelPart, Selector, StyleItem, Value, Wire,
};
use crate::syntax::parser;

mod trivia;
use trivia::{Trivia, TriviaToken, scan_trivia};

const INDENT: &str = "  ";

/// A block collapses onto one line (`|box| { radius: 6; }`) when the whole line
/// fits within this budget; past it, or once it holds a child node/wire, it
/// breaks across lines. Prettier's print-width, give or take.
const MAX_LINE: usize = 80;

pub fn format(src: &str) -> Result<String, Error> {
    let tokens = lexer::lex(src)?;
    let file = parser::parse(&tokens)?;
    let trivia = scan_trivia(src);
    let mut out = String::new();
    Emitter {
        trivia: &trivia,
        cursor: 0,
        out: &mut out,
        align: true,
        terse: true,
    }
    .emit_file(&file, src.len());
    Ok(out)
}

/// Emit an AST with no source to draw trivia from — for a synthesized `File`
/// (the desugar pass), whose nodes carry no real spans. Same emitter, empty
/// trivia: clean output, comments dropped, sugar expanded (`terse: false`).
pub(crate) fn print_file(file: &File) -> String {
    let mut out = String::new();
    Emitter {
        trivia: &[],
        cursor: 0,
        out: &mut out,
        align: false,
        terse: false,
    }
    .emit_file(file, 0);
    out
}

struct Emitter<'a> {
    trivia: &'a [TriviaToken],
    cursor: usize,
    out: &'a mut String,
    /// Column-align sibling id/type columns. On for canonical `fmt`; off for
    /// `print_file` (synthesized ASTs, where mixing anonymous sugar children
    /// with named nodes would pad oddly).
    align: bool,
    /// Contract a text-only `[ ]` to trailing labels (`api |box| "API"`). On for
    /// `fmt`; off for `print_file` (desugar), which keeps the explicit `[ ]`.
    terse: bool,
}

impl Emitter<'_> {
    fn emit_file(&mut self, file: &File, src_len: usize) {
        let mut phases = 0;
        if !file.stylesheet.is_empty() {
            self.emit_stylesheet(file);
            phases += 1;
        }
        if !file.instances.is_empty() {
            self.section_break(phases);
            self.emit_children(&file.instances, 0);
            phases += 1;
        }
        if !file.wires.is_empty() {
            self.section_break(phases);
            for w in &file.wires {
                self.emit_trivia_before(w.span.start, 0);
                self.emit_wire(w, 0);
                self.out.push('\n');
                self.cursor = w.span.end;
            }
        }
        self.emit_trivia_before(src_len, 0);
        if self.out.is_empty() {
            return;
        }
        if !self.out.ends_with('\n') {
            self.out.push('\n');
        }
    }

    /// One blank line between two non-empty phases, unless the output already
    /// ends in one.
    fn section_break(&mut self, phases_emitted: usize) {
        if phases_emitted > 0 && !self.out.is_empty() && !self.out.ends_with("\n\n") {
            self.out.push('\n');
        }
    }

    // ───────── Stylesheet ─────────

    fn emit_stylesheet(&mut self, file: &File) {
        let span = file.stylesheet_span;
        self.emit_trivia_before(span.start, 0);
        self.out.push_str("{\n");
        self.cursor = span.start.saturating_add(1);

        let items = &file.stylesheet;
        let mut i = 0;
        while i < items.len() {
            if matches!(items[i], StyleItem::RootDecl(_)) {
                // Root config declarations group on one line, the CSS-shaped style.
                let start = i;
                while i < items.len() && matches!(items[i], StyleItem::RootDecl(_)) {
                    i += 1;
                }
                let run: Vec<&Decl> = items[start..i]
                    .iter()
                    .map(|it| match it {
                        StyleItem::RootDecl(d) => d,
                        _ => unreachable!(),
                    })
                    .collect();
                self.emit_grouped_decls(&run, 1);
            } else {
                self.emit_trivia_before(style_item_span(&items[i]).start, 1);
                self.emit_style_item(&items[i], 1);
                self.cursor = style_item_span(&items[i]).end;
                i += 1;
            }
        }
        self.emit_trivia_before(span.end.saturating_sub(1), 1);
        self.out.push_str("}\n");
        self.cursor = span.end;
    }

    fn emit_style_item(&mut self, item: &StyleItem, depth: usize) {
        match item {
            StyleItem::RootDecl(d) => {
                self.indent(depth);
                self.emit_decl(d, false);
                self.out.push('\n');
            }
            StyleItem::Var(d) => {
                self.indent(depth);
                self.emit_decl(d, true);
                self.out.push('\n');
            }
            StyleItem::Rule(r) => self.emit_rule(r, depth),
            StyleItem::Define(d) => self.emit_define(d, depth),
        }
    }

    fn emit_rule(&mut self, rule: &Rule, depth: usize) {
        self.indent(depth);
        self.emit_selector(&rule.selector);
        // A rule body is declarations only — always written, even when empty.
        self.emit_style_block(&rule.decls, rule.span.end, depth, true);
        self.out.push('\n');
    }

    fn emit_define(&mut self, def: &Define, depth: usize) {
        self.indent(depth);
        self.out.push('|');
        self.out.push_str(&def.name);
        self.out.push_str("::");
        self.out.push_str(&def.base);
        self.out.push('|');
        if !def.style.is_empty() {
            let end = def.style_span.map_or(def.span.end, |s| s.end);
            self.emit_style_block(&def.style, end, depth, false);
        }
        self.emit_body(&def.children, &def.wires, def.span.end, depth);
        self.out.push('\n');
    }

    fn emit_selector(&mut self, sel: &Selector) {
        // The wire-defaults rule carries the reserved `wire` selector internally
        // but is written with the wire glyph; a lone class stays bare.
        match sel.parts.as_slice() {
            [SelPart::Type(t)] if t == "wire" => {
                self.out.push_str("->");
                return;
            }
            [SelPart::Class(c)] => {
                self.out.push('.');
                self.out.push_str(c);
                return;
            }
            _ => {}
        }
        self.out.push('|');
        for (i, part) in sel.parts.iter().enumerate() {
            if i > 0 {
                self.out.push(' ');
            }
            match part {
                SelPart::Type(t) => self.out.push_str(t),
                SelPart::Class(c) => {
                    self.out.push('.');
                    self.out.push_str(c);
                }
            }
        }
        self.out.push('|');
    }

    // ───────── Instances ─────────

    fn emit_children(&mut self, children: &[Child], depth: usize) {
        let widths = if self.align {
            align::child_widths(children, self.trivia)
        } else {
            vec![align::NodeWidths::default(); children.len()]
        };
        for (i, c) in children.iter().enumerate() {
            let span = child_span(c);
            self.emit_trivia_before(span.start, depth);
            match c {
                Child::Box(n) => self.emit_node(n, depth, widths[i]),
                Child::Text(t) => {
                    self.indent(depth);
                    self.emit_string(&t.text);
                }
            }
            self.out.push('\n');
            self.cursor = span.end;
        }
    }

    fn emit_node(&mut self, node: &Node, depth: usize, w: align::NodeWidths) {
        self.indent(depth);
        // The id and `|type|` columns align within an all-plain group; `w` is
        // zero otherwise, so the line stays ragged. A `.class` chain and a `{ }`
        // block never align — they trail with a single space (SPEC §14).
        let bars = type_bars(&node.ty);
        let classes = class_str(&node.classes);
        let has_block = !node.style.is_empty();
        let has_body = !node.children.is_empty() || !node.wires.is_empty();
        let id = node.id.as_deref().unwrap_or("");

        let after_id = !bars.is_empty() || !classes.is_empty() || has_block || has_body;
        let after_ty = !classes.is_empty() || has_block || has_body;
        let mut wrote = self.emit_col(id, w.id, after_id, false);
        wrote = self.emit_col(&bars, w.ty, after_ty, wrote);
        if !classes.is_empty() {
            self.space_if(wrote);
            self.out.push_str(&classes);
        }

        if has_block {
            let end = node.style_span.map_or(node.span.end, |s| s.end);
            self.emit_style_block(&node.style, end, depth, false);
        }
        self.emit_content(node, depth);
    }

    /// Emit one head column (id or `|type|`): a separating space when the line
    /// already carries a segment, the segment text, then alignment padding to
    /// `width` when a later column or content `follows` (else ragged, to avoid
    /// trailing space). An empty segment with nothing reserved emits nothing.
    /// Returns whether the line now carries any head segment.
    fn emit_col(&mut self, seg: &str, width: usize, follows: bool, preceded: bool) -> bool {
        if seg.is_empty() && (width == 0 || !follows) {
            return preceded;
        }
        self.space_if(preceded);
        self.out.push_str(seg);
        if follows {
            pad(self.out, width.saturating_sub(seg.len()));
        }
        true
    }

    /// A node's content: a `|table|`'s aligned cells, a terse trailing label, an
    /// inline text `[ ]` (desugar), or the multi-line `[ ]` body.
    fn emit_content(&mut self, node: &Node, depth: usize) {
        if node.children.is_empty() && node.wires.is_empty() {
            return;
        }
        let end = node.span.end;
        if let Some(cols) = self.table_cols(node) {
            self.out.push_str(" [\n");
            self.emit_aligned_cells(&node.children, cols, depth + 1);
            self.emit_trivia_before(end.saturating_sub(1), depth + 1);
            self.indent(depth);
            self.out.push(']');
            return;
        }
        let text_only =
            node.wires.is_empty() && node.children.iter().all(|c| matches!(c, Child::Text(_)));
        if text_only && !self.has_trivia_between(self.cursor, end) {
            if self.terse {
                for c in &node.children {
                    if let Child::Text(t) = c {
                        self.out.push(' ');
                        self.emit_string(&t.text);
                    }
                }
                self.cursor = end;
                return;
            }
            if self.try_inline_text(&node.children, end) {
                return;
            }
        }
        self.emit_body(&node.children, &node.wires, end, depth);
    }

    /// `[ "a" "b" ]` on one line, when it fits (desugar's explicit text form).
    fn try_inline_text(&mut self, children: &[Child], end: usize) -> bool {
        let line_start = self.out.rfind('\n').map_or(0, |i| i + 1);
        let saved = self.out.len();
        self.out.push_str(" [ ");
        for (i, c) in children.iter().enumerate() {
            if i > 0 {
                self.out.push(' ');
            }
            if let Child::Text(t) = c {
                self.emit_string(&t.text);
            }
        }
        self.out.push_str(" ]");
        if self.out.len() - line_start <= MAX_LINE {
            self.cursor = end;
            true
        } else {
            self.out.truncate(saved);
            false
        }
    }

    /// The multi-line `[ children … wires … ]` body.
    fn emit_body(&mut self, children: &[Child], wires: &[Wire], end: usize, depth: usize) {
        if children.is_empty() && wires.is_empty() && !self.has_comment_in(self.cursor, end) {
            return;
        }
        self.out.push_str(" [\n");
        self.emit_children(children, depth + 1);
        for w in wires {
            self.emit_trivia_before(w.span.start, depth + 1);
            self.emit_wire(w, depth + 1);
            self.out.push('\n');
            self.cursor = w.span.end;
        }
        self.emit_trivia_before(end.saturating_sub(1), depth + 1);
        self.indent(depth);
        self.out.push(']');
    }

    /// The column count if this node is a grid whose children are *all* bare text
    /// (a `|table|`) with no interleaved comment — then the cells align into
    /// columns (SPEC §8/§14). Otherwise `None`, falling back to one child per line.
    fn table_cols(&self, node: &Node) -> Option<usize> {
        let cells = &node.children;
        if cells.is_empty()
            || !node.wires.is_empty()
            || !cells.iter().all(|c| matches!(c, Child::Text(_)))
        {
            return None;
        }
        let start = child_span(&cells[0]).start;
        let end = child_span(cells.last().unwrap()).end;
        if self.has_trivia_between(start, end) {
            return None;
        }
        count_columns(&node.style)
    }

    /// Emit bare-text cells as aligned rows: each column padded to its widest
    /// cell, a single space between columns, `columns` cells per row.
    fn emit_aligned_cells(&mut self, cells: &[Child], cols: usize, depth: usize) {
        let texts: Vec<String> = cells
            .iter()
            .map(|c| match c {
                Child::Text(t) => quoted(&t.text),
                Child::Box(_) => String::new(),
            })
            .collect();
        let mut widths = vec![0usize; cols];
        for (i, s) in texts.iter().enumerate() {
            widths[i % cols] = widths[i % cols].max(s.len());
        }
        for (i, s) in texts.iter().enumerate() {
            let col = i % cols;
            if col == 0 {
                self.indent(depth);
            } else {
                self.out.push(' ');
            }
            self.out.push_str(s);
            if col == cols - 1 || i == texts.len() - 1 {
                self.out.push('\n');
            } else {
                pad(self.out, widths[col] - s.len());
            }
        }
        self.cursor = child_span(cells.last().unwrap()).end;
    }

    fn space_if(&mut self, cond: bool) {
        if cond {
            self.out.push(' ');
        }
    }

    /// Emit a run of declarations grouped onto as few lines as the source's
    /// trivia allows (SPEC §20): consecutive decls with nothing between them
    /// share one line, and a comment or blank line starts a fresh one.
    fn emit_grouped_decls(&mut self, decls: &[&Decl], depth: usize) {
        let mut mid_line = false;
        for d in decls {
            if mid_line && self.has_trivia_between(self.cursor, d.span.start) {
                self.out.push('\n');
                mid_line = false;
            }
            self.emit_trivia_before(d.span.start, depth);
            if mid_line {
                self.out.push(' ');
            } else {
                self.indent(depth);
            }
            self.emit_decl(d, false);
            self.cursor = d.span.end;
            mid_line = true;
        }
        if mid_line {
            self.out.push('\n');
        }
    }

    fn has_trivia_between(&self, start: usize, end: usize) -> bool {
        self.trivia.iter().any(|t| t.pos >= start && t.pos < end)
    }

    /// A `{ }` style block: declarations only. Collapses to ` { a; b }` when it
    /// fits and has no interleaved comment, else breaks across lines. When empty,
    /// `keep_empty` decides ` {}` (rules / defines) vs nothing (a node's style).
    fn emit_style_block(&mut self, decls: &[Decl], end: usize, depth: usize, keep_empty: bool) {
        if decls.is_empty() {
            if keep_empty && !self.has_comment_in(self.cursor, end) {
                self.out.push_str(" {}");
                self.cursor = end;
            }
            return;
        }
        if self.try_inline_decls(decls, end) {
            return;
        }
        self.out.push_str(" {\n");
        let refs: Vec<&Decl> = decls.iter().collect();
        self.emit_grouped_decls(&refs, depth + 1);
        self.emit_trivia_before(end.saturating_sub(1), depth + 1);
        self.indent(depth);
        self.out.push('}');
    }

    /// Try to collapse a declaration block onto the current line — ` { a; b }`.
    /// Restores and returns `false` if there is a comment inside or the line
    /// exceeds [`MAX_LINE`].
    fn try_inline_decls(&mut self, decls: &[Decl], end: usize) -> bool {
        if self.has_trivia_between(self.cursor, end) {
            return false;
        }
        let line_start = self.out.rfind('\n').map_or(0, |i| i + 1);
        let saved = self.out.len();
        self.out.push_str(" { ");
        for (i, d) in decls.iter().enumerate() {
            if i > 0 {
                self.out.push(' ');
            }
            self.emit_decl(d, false);
        }
        self.out.push_str(" }");
        if self.out.len() - line_start <= MAX_LINE {
            self.cursor = end;
            true
        } else {
            self.out.truncate(saved);
            false
        }
    }

    // ───────── Wires ─────────

    fn emit_wire(&mut self, w: &Wire, depth: usize) {
        self.indent(depth);
        for (i, group) in w.chain.iter().enumerate() {
            if i > 0 {
                self.out.push(' ');
                self.out.push_str(&wire_op_str(w.op));
                self.out.push(' ');
            }
            for (j, ep) in group.endpoints.iter().enumerate() {
                if j > 0 {
                    self.out.push_str(" & ");
                }
                self.emit_endpoint(ep);
            }
        }
        if !w.classes.is_empty() {
            self.out.push(' ');
            self.out.push_str(&class_str(&w.classes));
        }
        if !w.style.is_empty() {
            let end = w.style_span.map_or(w.span.end, |s| s.end);
            self.emit_style_block(&w.style, end, depth, false);
        }
        // A wire is not a container: its labels always trail (SPEC §9).
        for label in &w.labels {
            self.out.push(' ');
            self.emit_string(&label.text);
        }
    }

    fn emit_endpoint(&mut self, ep: &Endpoint) {
        self.out.push_str(&ep.path.join("."));
        if let Some(side) = ep.side {
            self.out.push('.');
            self.out.push_str(side_str(side));
        }
    }

    // ───────── Declarations & values ─────────

    fn emit_decl(&mut self, decl: &Decl, is_var: bool) {
        if is_var {
            self.out.push_str("--");
        }
        self.out.push_str(&decl.name);
        self.out.push_str(": ");
        for (i, group) in decl.groups.iter().enumerate() {
            if i > 0 {
                self.out.push_str(", ");
            }
            for (j, v) in group.iter().enumerate() {
                if j > 0 {
                    self.out.push(' ');
                }
                self.emit_value(v);
            }
        }
        self.out.push(';');
    }

    fn emit_value(&mut self, v: &Value) {
        match v {
            Value::Number(n) => self.out.push_str(&format_number(*n)),
            Value::String(s) => self.emit_string(s),
            Value::Hex(h) => {
                self.out.push('#');
                self.out.push_str(h);
            }
            Value::Ident(s) => self.out.push_str(s),
            Value::Var(name) => {
                self.out.push_str("--");
                self.out.push_str(name);
            }
            Value::Call(c) => {
                self.out.push_str(&c.name);
                self.out.push('(');
                for (i, arg) in c.args.iter().enumerate() {
                    if i > 0 {
                        self.out.push_str(", ");
                    }
                    self.emit_value(arg);
                }
                self.out.push(')');
            }
        }
    }

    fn emit_string(&mut self, s: &str) {
        self.out.push_str(&quoted(s));
    }

    // ───────── Trivia ─────────

    fn indent(&mut self, depth: usize) {
        for _ in 0..depth {
            self.out.push_str(INDENT);
        }
    }

    fn has_comment_in(&self, start: usize, end: usize) -> bool {
        self.trivia
            .iter()
            .any(|t| matches!(t.kind, Trivia::Comment(_)) && t.pos >= start && t.pos < end)
    }

    fn emit_trivia_before(&mut self, until: usize, depth: usize) {
        let mut last_was_blank = false;
        for t in self.trivia {
            if t.pos < self.cursor {
                continue;
            }
            if t.pos >= until {
                break;
            }
            match &t.kind {
                Trivia::Comment(text) => {
                    self.indent(depth);
                    self.out.push_str(text);
                    self.out.push('\n');
                    last_was_blank = false;
                }
                Trivia::BlankLine => {
                    if !last_was_blank && !self.out.is_empty() && !self.out.ends_with("\n\n") {
                        self.out.push('\n');
                        last_was_blank = true;
                    }
                }
            }
        }
        self.cursor = until;
    }
}

// ─────────────────────────── Spans & token helpers ───────────────────────────

fn style_item_span(item: &StyleItem) -> Span {
    match item {
        StyleItem::RootDecl(d) | StyleItem::Var(d) => d.span,
        StyleItem::Rule(r) => r.span,
        StyleItem::Define(d) => d.span,
    }
}

/// The `|type|` bars, or empty when the node has no type.
fn type_bars(ty: &Option<String>) -> String {
    match ty {
        Some(t) => format!("|{t}|"),
        None => String::new(),
    }
}

/// The `.class` chain (`.a.b`), or empty when the node wears none.
fn class_str(classes: &[String]) -> String {
    let mut s = String::new();
    for c in classes {
        s.push('.');
        s.push_str(c);
    }
    s
}

fn wire_op_str(op: WireOp) -> String {
    format!(
        "{}{}{}",
        op.start.start_str(),
        op.line.as_str(),
        op.end.end_str()
    )
}

fn side_str(s: Side) -> &'static str {
    match s {
        Side::Top => "top",
        Side::Bottom => "bottom",
        Side::Left => "left",
        Side::Right => "right",
    }
}

fn pad(out: &mut String, n: usize) {
    for _ in 0..n {
        out.push(' ');
    }
}

fn child_span(c: &Child) -> Span {
    match c {
        Child::Box(n) => n.span,
        Child::Text(t) => t.span,
    }
}

/// A string as a Lini double-quoted literal, with the four escapes.
fn quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The grid column count from a `columns:` declaration — a track list where
/// `repeat(N)` counts as N and every other entry as one. `None` if absent.
fn count_columns(decls: &[Decl]) -> Option<usize> {
    let d = decls.iter().find(|d| d.name == "columns")?;
    let n: usize = d
        .groups
        .iter()
        .flatten()
        .map(|v| match v {
            Value::Call(c) if c.name == "repeat" => c
                .args
                .first()
                .and_then(|a| match a {
                    Value::Number(x) if *x >= 1.0 => Some(*x as usize),
                    _ => None,
                })
                .unwrap_or(1),
            _ => 1,
        })
        .sum();
    (n > 0).then_some(n)
}

fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.is_finite() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

mod align;

#[cfg(test)]
mod tests;
