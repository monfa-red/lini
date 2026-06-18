//! Canonical source formatter (SPEC §14). Parses to the v4 AST and re-emits a
//! normalized form: the three phases in order (stylesheet → instances → wires),
//! `key: value;` declarations in `{ }` blocks, `name::base` defines, 2-space
//! indent, space-separated value groups (comma between groups). Comments and
//! blank-line groupings are preserved; sibling nodes align their id/type
//! columns. Idempotent: `fmt(fmt(x)) == fmt(x)`.

use crate::syntax::ast::{
    Block, Decl, Define, Endpoint, File, Node, Rule, SelPart, Selector, StyleItem, TextChild,
    Value, Wire, WireBlock,
};
use crate::ast::{Side, WireOp};
use crate::error::Error;
use crate::lexer;
use crate::span::Span;
use crate::syntax::parser;

mod trivia;
use trivia::{Trivia, TriviaToken, scan_trivia};

const INDENT: &str = "  ";

/// A declarations-only block collapses onto one line (`.hot { stroke: red; }`)
/// when the whole line fits within this budget; past it, or once the block holds
/// a child node/wire, it breaks across lines. Prettier's print-width, give or take.
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
    }
    .emit_file(&file, src.len());
    Ok(out)
}

/// Emit an AST with no source to draw trivia from — for a synthesized `File`
/// (the desugar pass), whose nodes carry no real spans. Same emitter, empty
/// trivia: clean output, comments dropped.
pub(crate) fn print_file(file: &File) -> String {
    let mut out = String::new();
    Emitter {
        trivia: &[],
        cursor: 0,
        out: &mut out,
        align: false,
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
}

impl Emitter<'_> {
    fn emit_file(&mut self, file: &File, src_len: usize) {
        let mut phases_emitted = 0;
        if !file.stylesheet.is_empty() {
            // Root config declarations group on one line like a block's, the same
            // CSS-shaped style (SPEC §20); rules, defines, and `--var`s each take
            // their own line.
            let items = &file.stylesheet;
            let mut i = 0;
            while i < items.len() {
                if matches!(items[i], StyleItem::RootDecl(_)) {
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
                    self.emit_grouped_decls(&run, 0);
                } else {
                    self.emit_trivia_before(style_item_span(&items[i]).start, 0);
                    self.emit_style_item(&items[i], 0);
                    self.cursor = style_item_span(&items[i]).end;
                    i += 1;
                }
            }
            phases_emitted += 1;
        }
        if !file.instances.is_empty() {
            self.section_break(phases_emitted);
            self.emit_nodes(&file.instances, 0);
            phases_emitted += 1;
        }
        if !file.wires.is_empty() {
            self.section_break(phases_emitted);
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

    /// One blank line between two non-empty phases (only once any phase has been
    /// written), unless the running output already ends in one.
    fn section_break(&mut self, phases_emitted: usize) {
        if phases_emitted > 0 && !self.out.is_empty() && !self.out.ends_with("\n\n") {
            self.out.push('\n');
        }
    }

    // ───────── Stylesheet ─────────

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
        self.emit_decl_block(&rule.decls, rule.span.end, depth);
        self.out.push('\n');
    }

    fn emit_define(&mut self, def: &Define, depth: usize) {
        self.indent(depth);
        self.out.push_str(&def.name);
        self.out.push_str("::");
        self.out.push_str(&def.base);
        self.emit_block(&def.body, def.span.end, depth);
        self.out.push('\n');
    }

    fn emit_selector(&mut self, sel: &Selector) {
        // The wire-defaults rule carries the reserved `wire` selector internally
        // but is written with the wire glyph.
        if let [SelPart::Type(t)] = sel.parts.as_slice()
            && t == "wire"
        {
            self.out.push_str("->");
            return;
        }
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
    }

    // ───────── Instances ─────────

    fn emit_nodes(&mut self, nodes: &[Node], depth: usize) {
        let widths = if self.align {
            align::node_widths(nodes, self.trivia)
        } else {
            vec![align::NodeWidths::default(); nodes.len()]
        };
        for (i, n) in nodes.iter().enumerate() {
            self.emit_trivia_before(n.span.start, depth);
            self.emit_node(n, depth, widths[i]);
            self.out.push('\n');
            self.cursor = n.span.end;
        }
    }

    fn emit_node(&mut self, node: &Node, depth: usize, w: align::NodeWidths) {
        self.indent(depth);
        // Head tokens — `id |type| "labels" .classes` — each separated from the
        // last by one space, with the alignment widths padding the id/type
        // columns out to the group's max. `wrote` tracks whether any token (or a
        // reserved-but-empty column) precedes, so the separator never leads.
        let mut wrote = false;
        if let Some(id) = &node.id {
            self.out.push_str(id);
            pad(self.out, w.id.saturating_sub(id.len()));
            wrote = true;
        } else if w.id > 0 {
            pad(self.out, w.id);
            wrote = true;
        }
        if let Some(ty) = &node.ty {
            self.space_if(wrote);
            let t = format!("|{}|", ty);
            self.out.push_str(&t);
            pad(self.out, w.ty.saturating_sub(t.len()));
            wrote = true;
        } else if w.ty > 0 {
            self.space_if(wrote);
            pad(self.out, w.ty);
            wrote = true;
        }
        for label in &node.labels {
            self.space_if(wrote);
            self.emit_string(label);
            wrote = true;
        }
        for class in &node.classes {
            self.space_if(wrote);
            self.out.push('.');
            self.out.push_str(class);
            wrote = true;
        }
        if let Some(block) = &node.block {
            self.emit_block(block, node.span.end, depth);
        }
    }

    fn space_if(&mut self, cond: bool) {
        if cond {
            self.out.push(' ');
        }
    }

    /// Emit a run of declarations grouped onto as few lines as the source's
    /// trivia allows (SPEC §20): consecutive decls with nothing between them
    /// share one line (`cell: 1 2; layout: column; gap: 16;`), and a comment or
    /// blank line starts a fresh one. They never share the opening brace's line.
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

    /// Try to collapse a declarations-only block onto the current line —
    /// ` { a; b; }`. Returns `true` (and keeps it) when there's no comment/blank
    /// inside and the finished line fits [`MAX_LINE`]; otherwise emits nothing
    /// and returns `false`, so the caller falls through to the multi-line form.
    /// `end` is the block's source end (no trivia past the decls, by the check).
    fn try_inline_block(&mut self, decls: &[&Decl], end: usize) -> bool {
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

    fn emit_block(&mut self, block: &Block, end: usize, depth: usize) {
        let empty = block.decls.is_empty() && block.nodes.is_empty() && block.wires.is_empty();
        if empty && !self.has_comment_in(self.cursor, end) {
            self.out.push_str(" {}");
            self.cursor = end;
            return;
        }
        let decls: Vec<&Decl> = block.decls.iter().collect();
        // A block with a child node or wire always breaks; a declarations-only
        // body may collapse onto one line when it fits.
        if block.nodes.is_empty() && block.wires.is_empty() && self.try_inline_block(&decls, end) {
            return;
        }
        self.out.push_str(" {\n");
        self.emit_grouped_decls(&decls, depth + 1);
        self.emit_nodes(&block.nodes, depth + 1);
        for wire in &block.wires {
            self.emit_trivia_before(wire.span.start, depth + 1);
            self.emit_wire(wire, depth + 1);
            self.out.push('\n');
            self.cursor = wire.span.end;
        }
        self.emit_trivia_before(end, depth + 1);
        self.indent(depth);
        self.out.push('}');
    }

    /// A rule body: only declarations, same braces.
    fn emit_decl_block(&mut self, decls: &[Decl], end: usize, depth: usize) {
        if decls.is_empty() && !self.has_comment_in(self.cursor, end) {
            self.out.push_str(" {}");
            self.cursor = end;
            return;
        }
        let refs: Vec<&Decl> = decls.iter().collect();
        if self.try_inline_block(&refs, end) {
            return;
        }
        self.out.push_str(" {\n");
        self.emit_grouped_decls(&refs, depth + 1);
        self.emit_trivia_before(end, depth + 1);
        self.indent(depth);
        self.out.push('}');
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
        for label in &w.labels {
            self.out.push(' ');
            self.emit_string(label);
        }
        for class in &w.classes {
            self.out.push_str(" .");
            self.out.push_str(class);
        }
        if let Some(block) = &w.block {
            self.emit_wire_block(block, w.span.end, depth);
        }
    }

    fn emit_endpoint(&mut self, ep: &Endpoint) {
        self.out.push_str(&ep.path.join("."));
        if let Some(side) = ep.side {
            self.out.push('.');
            self.out.push_str(side_str(side));
        }
    }

    fn emit_wire_block(&mut self, block: &WireBlock, end: usize, depth: usize) {
        let empty = block.decls.is_empty() && block.texts.is_empty();
        if empty && !self.has_comment_in(self.cursor, end) {
            self.out.push_str(" {}");
            self.cursor = end;
            return;
        }
        let decls: Vec<&Decl> = block.decls.iter().collect();
        if block.texts.is_empty() && self.try_inline_block(&decls, end) {
            return;
        }
        self.out.push_str(" {\n");
        self.emit_grouped_decls(&decls, depth + 1);
        for t in &block.texts {
            self.emit_trivia_before(t.span.start, depth + 1);
            self.indent(depth + 1);
            self.emit_text_child(t);
            self.out.push('\n');
            self.cursor = t.span.end;
        }
        self.emit_trivia_before(end, depth + 1);
        self.indent(depth);
        self.out.push('}');
    }

    fn emit_text_child(&mut self, t: &TextChild) {
        self.out.push_str("|text| ");
        self.emit_string(&t.text);
        for class in &t.classes {
            self.out.push_str(" .");
            self.out.push_str(class);
        }
        if !t.decls.is_empty() {
            self.out.push_str(" {");
            for d in &t.decls {
                self.out.push(' ');
                self.emit_decl(d, false);
            }
            self.out.push_str(" }");
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
        self.out.push('"');
        for c in s.chars() {
            match c {
                '"' => self.out.push_str("\\\""),
                '\\' => self.out.push_str("\\\\"),
                '\n' => self.out.push_str("\\n"),
                '\t' => self.out.push_str("\\t"),
                _ => self.out.push(c),
            }
        }
        self.out.push('"');
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

fn wire_op_str(op: WireOp) -> String {
    format!("{}{}{}", op.start.start_str(), op.line.as_str(), op.end.end_str())
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
