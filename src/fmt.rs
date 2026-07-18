//! Canonical source formatter [SPEC 19]. Parses to the AST and re-emits a
//! normalized form: the three phases in order (the stylesheet `{ }`, then the
//! instances, then the links), `{ }` style blocks and `[ ]` child lists, bar-wrapped
//! type selectors and `|name::base|` defines, 2-space indent, space-separated value
//! groups. Comments and blank-line groupings are preserved; sibling nodes align
//! their id column (the bars line up), and a plain group aligns its type column
//! too (the labels). Idempotent: `fmt(fmt(x)) == fmt(x)`.

use crate::error::Error;
use crate::lexer;
use crate::span::Span;
use crate::syntax::ast::{
    Child, Decl, Define, Endpoint, File, LabelItem, Link, Node, Rule, SelUnit, Selector, StyleItem,
    TextNode, Value,
};
use crate::syntax::parser;

mod trivia;
use trivia::{Trivia, TriviaToken, scan_trivia};

const INDENT: &str = "  ";

/// A block collapses onto one line (`|box| { radius: 6; }`) when the whole line
/// fits within this budget; past it, or once it holds a child node/link, it
/// breaks across lines. Prettier's print-width, give or take.
const MAX_LINE: usize = 80;

pub fn format(src: &str) -> Result<String, Error> {
    let tokens = lexer::lex(src)?;
    let file = parser::parse(src, &tokens)?;
    let trivia = scan_trivia(src);
    let mut out = String::new();
    Emitter {
        trivia: &trivia,
        cursor: 0,
        out: &mut out,
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
        terse: false,
    }
    .emit_file(file, 0);
    out
}

struct Emitter<'a> {
    trivia: &'a [TriviaToken],
    cursor: usize,
    out: &'a mut String,
    /// Contract a lone bare-text child to the head label (`|box#api| "API"`). On
    /// for `fmt`; off for `print_file` (desugar), which keeps the explicit `[ ]`.
    terse: bool,
}

impl Emitter<'_> {
    fn emit_file(&mut self, file: &File, src_len: usize) {
        let mut phases = 0;
        if !file.stylesheet.is_empty() {
            self.emit_stylesheet(file);
            phases += 1;
        }
        // The drawn statements: instances and links in source order. A normal
        // (phased) file keeps the conventional blank line between the canvas and
        // the links; a `layout: sequence` interleaves them, so there is no split.
        if phased(&file.instances, &file.links) {
            if !file.instances.is_empty() {
                self.section_break(phases);
                self.emit_ordered(&file.instances, &[], 0);
                phases += 1;
            }
            if !file.links.is_empty() {
                self.section_break(phases);
                self.emit_ordered(&[], &file.links, 0);
            }
        } else {
            self.section_break(phases);
            self.emit_ordered(&file.instances, &file.links, 0);
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
            StyleItem::Binding(f) => {
                self.indent(depth);
                self.out.push_str(&f.name);
                if !f.params.is_empty() {
                    self.out.push('(');
                    self.out.push_str(&f.params.join(", "));
                    self.out.push(')');
                }
                self.out.push_str(" = ");
                // A scalar / call / name reads bare; math, locals, or a point wants a
                // group [SPEC 10.7].
                let atomic = crate::expr::Expr::parse(&f.body)
                    .map(|e| e.is_atomic())
                    .unwrap_or(false);
                if atomic {
                    self.out.push_str(f.body.trim());
                } else {
                    self.out.push('(');
                    self.out.push_str(f.body.trim());
                    self.out.push(')');
                }
                self.out.push_str(";\n");
            }
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
        self.emit_body(&def.children, &def.links, def.span.end, depth);
        self.out.push('\n');
    }

    fn emit_selector(&mut self, sel: &Selector) {
        // Juxtaposed units, single-spaced [SPEC 4]: a type `|box|` / `|table#main|`
        // keeps its bars, a class `.hot` and an id `#hero` keep their sigil.
        for (i, unit) in sel.units.iter().enumerate() {
            if i > 0 {
                self.out.push(' ');
            }
            match unit {
                SelUnit::Type { name, id } => {
                    self.out.push('|');
                    self.out.push_str(name);
                    if let Some(id) = id {
                        self.out.push('#');
                        self.out.push_str(id);
                    }
                    self.out.push('|');
                }
                SelUnit::Class(c) => {
                    self.out.push('.');
                    self.out.push_str(c);
                }
                SelUnit::Id(i) => {
                    self.out.push('#');
                    self.out.push_str(i);
                }
                // `|-|` — the link type [SPEC 9].
                SelUnit::Link => self.out.push_str("|-|"),
                // `(-)` — the dimension type [SPEC 15.6].
                SelUnit::Dimension => self.out.push_str("(-)"),
            }
        }
    }

    // ───────── Instances ─────────

    /// Emit a scope's children and internal links **interleaved in source order**
    /// [SPEC 3] — by span, so the formatter is faithful to a `layout: sequence`
    /// (where that order is time) and the trivia cursor advances monotonically.
    /// One emitter per item kind, shared by the file and every `[ ]` body.
    fn emit_ordered(&mut self, children: &[Child], links: &[Link], depth: usize) {
        enum Item<'a> {
            Child(&'a Child),
            Link(&'a Link),
        }
        let mut items: Vec<Item> = Vec::with_capacity(children.len() + links.len());
        items.extend(children.iter().map(Item::Child));
        items.extend(links.iter().map(Item::Link));
        items.sort_by_key(|it| match it {
            Item::Child(c) => c.span().start,
            Item::Link(w) => w.span.start,
        });
        for it in items {
            let (start, end) = match &it {
                Item::Child(c) => {
                    let s = c.span();
                    (s.start, s.end)
                }
                Item::Link(w) => (w.span.start, w.span.end),
            };
            self.emit_trivia_before(start, depth);
            match it {
                Item::Child(Child::Box(n)) => self.emit_node(n, depth),
                Item::Child(Child::Text(t)) => {
                    self.indent(depth);
                    self.emit_text_node(t, depth);
                }
                Item::Link(w) => self.emit_link(w, depth),
            }
            self.out.push('\n');
            self.cursor = end;
        }
    }

    /// A node head: `|type#id|`, then the head label, then classes, then a `{ }`
    /// block, then the `[ ]` content [SPEC 3/16].
    fn emit_node(&mut self, node: &Node, depth: usize) {
        self.indent(depth);
        self.out.push_str(&identity_bars(node));
        // The head label is exactly the source's, never contracted from a `[ ]`
        // text child — its meaning is type-dependent and fmt resolves no types.
        if let Some(label) = &node.label {
            // The head label takes no style of its own [SPEC 3].
            self.out.push(' ');
            self.emit_string(&label.text);
        }
        if !node.classes.is_empty() {
            self.out.push(' ');
            self.out.push_str(&class_str(&node.classes));
        }
        if !node.style.is_empty() {
            let end = node.style_span.map_or(node.span.end, |s| s.end);
            self.emit_style_block(&node.style, end, depth, false);
        }
        self.emit_content(node, &node.children, depth);
    }

    /// A node's `[ ]` content: a `|table|`'s aligned cells, an inline text `[ ]`,
    /// or the multi-line `[ ]` body. `body` is the children after the head label.
    fn emit_content(&mut self, node: &Node, body: &[Child], depth: usize) {
        if body.is_empty() && node.links.is_empty() {
            return;
        }
        let end = node.span.end;
        if node.links.is_empty() {
            if let Some(cols) = self.table_cols(body, &node.style) {
                self.out.push_str(" [\n");
                self.emit_aligned_cells(body, cols, depth + 1);
                self.emit_trivia_before(end.saturating_sub(1), depth + 1);
                self.indent(depth);
                self.out.push(']');
                return;
            }
            let text_only = !body.is_empty()
                && body
                    .iter()
                    .all(|c| matches!(c, Child::Text(t) if is_plain_text(t)));
            if text_only
                && !self.has_trivia_between(self.cursor, end)
                && self.try_inline_text(body, end)
            {
                return;
            }
        }
        self.emit_body(body, &node.links, end, depth);
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
                self.emit_text_node(t, 0);
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

    /// The multi-line `[ children … links … ]` body.
    fn emit_body(&mut self, children: &[Child], links: &[Link], end: usize, depth: usize) {
        if children.is_empty() && links.is_empty() && !self.has_comment_in(self.cursor, end) {
            return;
        }
        self.out.push_str(" [\n");
        self.emit_ordered(children, links, depth + 1);
        self.emit_trivia_before(end.saturating_sub(1), depth + 1);
        self.indent(depth);
        self.out.push(']');
    }

    /// The column count if these body cells are *all* bare text (a `|table|`) with
    /// no interleaved comment — then they align into columns [SPEC 8/16].
    /// Otherwise `None`, falling back to one child per line. The caller has already
    /// excluded a node with internal links.
    fn table_cols(&self, cells: &[Child], style: &[Decl]) -> Option<usize> {
        if cells.is_empty() || !cells.iter().all(|c| matches!(c, Child::Text(_))) {
            return None;
        }
        let start = cells[0].span().start;
        let end = cells.last().unwrap().span().end;
        if self.has_trivia_between(start, end) {
            return None;
        }
        count_columns(style)
    }

    /// Emit bare-text cells as aligned rows: each column padded to its widest
    /// cell, a single space between columns, `columns` cells per row. A cell that
    /// carries a `{ }` style block re-emits it, and its whole row leaves the grid
    /// (a block throws the widths off) — unstyled rows stay aligned [SPEC 19].
    fn emit_aligned_cells(&mut self, cells: &[Child], cols: usize, depth: usize) {
        // A cell that carries a `{ }` block **or** a worn class throws the
        // column widths off, so its whole row breaks out of the grid [SPEC 19].
        let styled = |c: &Child| matches!(c, Child::Text(t) if !is_plain_text(t));
        let rows: Vec<&[Child]> = cells.chunks(cols).collect();
        // Column widths come from the aligned (unstyled) rows only.
        let mut widths = vec![0usize; cols];
        for row in rows.iter().filter(|r| !r.iter().any(styled)) {
            for (col, c) in row.iter().enumerate() {
                if let Child::Text(t) = c {
                    widths[col] = widths[col].max(quoted(&t.text).len());
                }
            }
        }
        for row in &rows {
            let free = row.iter().any(styled);
            for (col, c) in row.iter().enumerate() {
                if col == 0 {
                    self.indent(depth);
                } else {
                    self.out.push(' ');
                }
                let Child::Text(t) = c else { continue };
                if free {
                    self.cursor = t.span.start;
                    self.emit_text_node(t, depth);
                } else {
                    let s = quoted(&t.text);
                    self.out.push_str(&s);
                    if col != row.len() - 1 {
                        pad(self.out, widths[col] - s.len());
                    }
                }
            }
            self.out.push('\n');
        }
        self.cursor = cells.last().unwrap().span().end;
    }

    /// Emit a run of declarations grouped onto as few lines as the source's
    /// trivia allows [SPEC 19]: consecutive decls with nothing between them
    /// share one line, and a comment or blank line starts a fresh one.
    fn emit_grouped_decls(&mut self, decls: &[&Decl], depth: usize) {
        let mut mid_line = false;
        for d in decls {
            if mid_line && (d.name == "draw" || self.has_trivia_between(self.cursor, d.span.start))
            {
                self.out.push('\n');
                mid_line = false;
            }
            self.emit_trivia_before(d.span.start, depth);
            if mid_line {
                self.out.push(' ');
            } else {
                self.indent(depth);
            }
            if d.name == "draw" {
                // The pen reads as a paragraph — never sharing a line with
                // another declaration, each subpath on its own [SPEC 15.3/19].
                self.emit_draw_decl(d, depth);
                self.cursor = d.span.end;
                self.out.push('\n');
                mid_line = false;
                continue;
            }
            self.emit_decl(d, false);
            self.cursor = d.span.end;
            mid_line = true;
        }
        if mid_line {
            self.out.push('\n');
        }
    }

    /// The canonical `draw:` layout [SPEC 19]: pen calls flow to the line
    /// budget, every `move()` after the first starts a new subpath line, and
    /// continuations indent to align under the first call.
    fn emit_draw_decl(&mut self, d: &Decl, depth: usize) {
        self.out.push_str("draw: ");
        let pad = " ".repeat(INDENT.len() * depth + "draw: ".len());
        let mut first = true;
        for v in d.groups.iter().flatten() {
            let start = self.out.len();
            let new_subpath = matches!(v, Value::Call(c) if c.name == "move");
            if !first {
                if new_subpath {
                    self.out.push('\n');
                    self.out.push_str(&pad);
                } else {
                    self.out.push(' ');
                }
            }
            self.emit_value(v);
            let line_start = self.out.rfind('\n').map_or(0, |i| i + 1);
            if !first && !new_subpath && self.out.len() - line_start >= MAX_LINE {
                self.out.truncate(start);
                self.out.push('\n');
                self.out.push_str(&pad);
                self.emit_value(v);
            }
            first = false;
        }
        self.out.push(';');
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
        // A multi-subpath pen always breaks — one line per `move()` [SPEC 19].
        let multi_subpath = |d: &Decl| {
            d.name == "draw"
                && d.groups
                    .iter()
                    .flatten()
                    .filter(|v| matches!(v, Value::Call(c) if c.name == "move"))
                    .count()
                    > 1
        };
        if decls.iter().any(multi_subpath) {
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

    // ───────── Links ─────────

    fn emit_link(&mut self, w: &Link, depth: usize) {
        self.indent(depth);
        for (i, group) in w.chain.iter().enumerate() {
            if i > 0 {
                self.out.push(' ');
                self.out.push_str(&w.ops[i - 1].spelling());
                self.out.push(' ');
            }
            for (j, ep) in group.endpoints.iter().enumerate() {
                if j > 0 {
                    self.out.push_str(" & ");
                }
                self.emit_endpoint(ep);
            }
        }
        // A one-ended statement — a leader or unary measure [SPEC 15.6] — carries
        // its op after the single endpoint group, before the tail.
        if w.chain.len() == 1 {
            self.out.push(' ');
            self.out.push_str(&w.op().spelling());
        }
        // The tail mirrors a node's order [SPEC 9]: head label, then classes,
        // then style, then the `[ ]` labels. A lone bare label trails the head
        // (`a -> b "x"`); two or more, a styled one, or a carried annotation
        // node [SPEC 15.9] ride the `[ ]`.
        let texts: Vec<&TextNode> = w.label.iter().chain(w.label_texts()).collect();
        let has_nodes = w.label_nodes().next().is_some();
        let styled = texts.iter().any(|t| !t.style.is_empty());
        let head_label =
            (self.terse && !has_nodes && texts.len() == 1 && !styled).then(|| texts[0]);
        if let Some(label) = head_label {
            self.out.push(' ');
            self.emit_text_node(label, depth);
        }
        if !w.classes.is_empty() {
            self.out.push(' ');
            self.out.push_str(&class_str(&w.classes));
        }
        if !w.style.is_empty() {
            let end = w.style_span.map_or(w.span.end, |s| s.end);
            self.emit_style_block(&w.style, end, depth, false);
        }
        if head_label.is_some() || (texts.is_empty() && !has_nodes) {
            return;
        }
        if has_nodes {
            // A `[ ]` holding a node goes multi-line, like a node body — the
            // head label folds in ahead of the items, source order kept.
            self.out.push_str(" [\n");
            if let Some(label) = &w.label {
                self.indent(depth + 1);
                self.emit_text_node(label, depth + 1);
                self.out.push('\n');
            }
            for item in &w.labels {
                match item {
                    LabelItem::Text(t) => {
                        self.indent(depth + 1);
                        self.emit_text_node(t, depth + 1);
                    }
                    LabelItem::Node(n) => self.emit_node(n, depth + 1),
                }
                self.out.push('\n');
            }
            self.indent(depth);
            self.out.push(']');
        } else {
            self.out.push_str(" [ ");
            for (i, label) in texts.iter().enumerate() {
                if i > 0 {
                    self.out.push(' ');
                }
                self.emit_text_node(label, depth);
            }
            self.out.push_str(" ]");
        }
    }

    /// A text leaf `"…"` with its optional worn classes and `{ }` style block
    /// [SPEC 3] — the node tail on a string head (`"Starter" .card-title { … }`).
    fn emit_text_node(&mut self, t: &TextNode, depth: usize) {
        self.emit_string(&t.text);
        if !t.classes.is_empty() {
            self.out.push(' ');
            self.out.push_str(&class_str(&t.classes));
        }
        if !t.style.is_empty() {
            let end = t.style_span.map_or(t.span.end, |s| s.end);
            self.emit_style_block(&t.style, end, depth, false);
        }
    }

    fn emit_endpoint(&mut self, ep: &Endpoint) {
        self.out.push_str(&ep.path.join("."));
        if let Some(copy) = ep.copy {
            self.out.push('.');
            self.out.push_str(&copy.to_string());
        }
        if let Some(point) = &ep.point {
            self.out.push(':');
            self.out.push_str(&point.name);
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
            Value::Percent(n) => {
                self.out.push_str(&format_number(*n));
                self.out.push('%');
            }
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
                    self.emit_arg(arg);
                }
                self.out.push(')');
            }
            // A free-standing math group [SPEC 10.7] — the parens are the fence, so a
            // direct value always wears them (a call argument sheds them, `emit_arg`).
            Value::Expr(s) => {
                self.out.push('(');
                self.out.push_str(s.trim());
                self.out.push(')');
            }
            // Pen items [SPEC 15.3]: the segment name glues to its call; a
            // freestanding point stands alone.
            Value::NamedCall(c, name) => {
                self.emit_value(&Value::Call(c.clone()));
                self.out.push(':');
                self.out.push_str(name);
            }
            // A space-group in one call-arg slot (`hatch(45 -45, 6)`).
            Value::Tuple(items) => {
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        self.out.push(' ');
                    }
                    self.emit_value(item);
                }
            }
        }
    }

    /// One call argument: an expression argument is already inside the call's own
    /// parens, so it sheds the group a direct value would wear [SPEC 10.7].
    fn emit_arg(&mut self, v: &Value) {
        match v {
            Value::Expr(s) => self.out.push_str(s.trim()),
            _ => self.emit_value(v),
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
        StyleItem::Binding(f) => f.span,
    }
}

/// The identity bars `|type#id|` (`|#id|` when the type defaults to box).
fn identity_bars(node: &Node) -> String {
    let mut s = String::from("|");
    if let Some(t) = &node.ty {
        s.push_str(t);
    }
    if let Some(id) = &node.id {
        s.push('#');
        s.push_str(id);
    }
    s.push('|');
    s
}

/// A bare text leaf — no worn class, no `{ }` block. Such a cell aligns in a
/// table grid and packs inline (`[ "a" "b" ]`); a classed or styled one breaks
/// out [SPEC 19].
fn is_plain_text(t: &TextNode) -> bool {
    t.classes.is_empty() && t.style.is_empty()
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

fn pad(out: &mut String, n: usize) {
    for _ in 0..n {
        out.push(' ');
    }
}

/// Whether a scope's instances and links are cleanly **phased** — every instance
/// before every link, the conventional case the formatter separates with a blank
/// line. A `layout: sequence` interleaves them (`false`), so they merge with no
/// split. One side empty is trivially phased.
fn phased(instances: &[Child], links: &[Link]) -> bool {
    match (
        instances.iter().map(|c| c.span().start).max(),
        links.iter().map(|w| w.span.start).min(),
    ) {
        (Some(last_instance), Some(first_link)) => last_instance < first_link,
        _ => true,
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

#[cfg(test)]
mod tests;
