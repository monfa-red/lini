//! Canonical source formatter. Parses to AST, emits a normalized form.
//!
//! Rules (SPEC section 15 `lini fmt`):
//! - 2-space indent.
//! - One declaration per line; sibling declarations inside the same block get
//!   their id / type / label / attr columns aligned.
//! - Comments and blank-line groupings between siblings are preserved (at most
//!   one blank line collapsed from any longer run).
//! - Canonical value formatting (`(1, 2)` not `( 1 ,2 )`, etc.).
//! - Idempotent: `fmt(fmt(x)) == fmt(x)`.

use crate::ast::{
    AttrItem, BodyItem, DefsBlock, DefsEntry, File, SceneConfig, ShapeDef, ShapeInst, Stmt,
    StyleDef, TypeDefaults, Value, VarOverride, WireConfig, WireDecl, WireOp,
};
use crate::error::Error;
use crate::lexer;
use crate::parser;
use crate::span::Span;

const INDENT: &str = "  ";

pub fn format(src: &str) -> Result<String, Error> {
    let tokens = lexer::lex(src)?;
    let file = parser::parse(&tokens)?;
    let trivia = scan_trivia(src);
    let mut out = String::new();
    let mut emitter = Emitter {
        src,
        trivia: &trivia,
        cursor: 0,
        out: &mut out,
    };
    emitter.emit_file(&file);
    Ok(out)
}

/// Print an AST directly in canonical form, with no source to draw comments
/// from — for a synthesized `File` (e.g. the desugar pass), whose nodes carry
/// no real spans. Same emitter, empty trivia: clean output, comments dropped.
pub(crate) fn print_file(file: &File) -> String {
    let mut out = String::new();
    let mut emitter = Emitter {
        src: "",
        trivia: &[],
        cursor: 0,
        out: &mut out,
    };
    emitter.emit_file(file);
    out
}

// ─────────────────────────── Trivia (comments + blank lines) ───────────────────────────

#[derive(Debug, Clone)]
enum Trivia {
    Comment(String),
    BlankLine,
}

#[derive(Debug, Clone)]
struct TriviaToken {
    pos: usize,
    kind: Trivia,
}

fn scan_trivia(src: &str) -> Vec<TriviaToken> {
    let mut out = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    let mut at_line_start = true;
    let mut blank_run = 0usize;

    while i < bytes.len() {
        let c = bytes[i];
        match c {
            b' ' | b'\t' | b'\r' => i += 1,
            b'\n' => {
                if at_line_start {
                    blank_run += 1;
                    if blank_run == 2 {
                        out.push(TriviaToken {
                            pos: i,
                            kind: Trivia::BlankLine,
                        });
                    }
                } else {
                    blank_run = 1;
                }
                at_line_start = true;
                i += 1;
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                let start = i;
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                let text = src[start..i].trim_end().to_string();
                out.push(TriviaToken {
                    pos: start,
                    kind: Trivia::Comment(text),
                });
                at_line_start = false;
                blank_run = 0;
            }
            _ => {
                at_line_start = false;
                blank_run = 0;
                if c == b'"' {
                    i += 1;
                    while i < bytes.len() {
                        let cc = bytes[i];
                        if cc == b'\\' && i + 1 < bytes.len() {
                            i += 2;
                            continue;
                        }
                        i += 1;
                        if cc == b'"' {
                            break;
                        }
                    }
                } else {
                    i += 1;
                }
            }
        }
    }
    out
}

// ─────────────────────────── Emitter ───────────────────────────

struct Emitter<'a> {
    src: &'a str,
    trivia: &'a [TriviaToken],
    cursor: usize,
    out: &'a mut String,
}

impl<'a> Emitter<'a> {
    fn emit_file(&mut self, file: &File) {
        if let Some(defs) = &file.defs {
            self.emit_trivia_before(defs.span.start, 0);
            self.emit_defs(defs);
            self.cursor = defs.span.end;
        }
        if !file.stmts.is_empty() {
            // Blank line between defs and first stmt.
            if file.defs.is_some() && !self.out.ends_with("\n\n") {
                if self.out.ends_with('\n') {
                    self.out.push('\n');
                } else {
                    self.out.push_str("\n\n");
                }
            }
            let widths = compute_root_widths(&file.stmts, self.trivia);
            for (i, stmt) in file.stmts.iter().enumerate() {
                self.emit_trivia_before(stmt_span(stmt).start, 0);
                self.emit_stmt(stmt, 0, widths[i]);
                self.cursor = stmt_span(stmt).end;
            }
        }
        self.emit_trivia_before(self.src.len(), 0);
        if !self.out.ends_with('\n') {
            self.out.push('\n');
        }
    }

    // ───────── Defs block ─────────

    fn emit_defs(&mut self, defs: &DefsBlock) {
        self.out.push_str("{\n");
        for entry in &defs.entries {
            self.emit_trivia_before(entry.span().start, 1);
            self.indent(1);
            self.emit_defs_entry(entry);
            self.out.push('\n');
            self.cursor = entry.span().end;
        }
        self.emit_trivia_before(defs.span.end, 1);
        self.out.push_str("}\n");
    }

    fn emit_defs_entry(&mut self, entry: &DefsEntry) {
        match entry {
            DefsEntry::SceneConfig(s) => self.emit_scene_config(s),
            DefsEntry::WireConfig(w) => self.emit_wire_config(w),
            DefsEntry::TypeDefaults(t) => self.emit_type_defaults(t),
            DefsEntry::VarOverride(v) => self.emit_var_override(v),
            DefsEntry::StyleDef(s) => self.emit_style_def(s),
            DefsEntry::ShapeDef(s) => self.emit_shape_def(s),
        }
    }

    fn emit_scene_config(&mut self, s: &SceneConfig) {
        self.out.push_str("|scene|");
        self.emit_attr_items(&s.items);
    }

    fn emit_wire_config(&mut self, w: &WireConfig) {
        self.out.push_str("|wire|");
        self.emit_attr_items(&w.items);
    }

    fn emit_type_defaults(&mut self, t: &TypeDefaults) {
        self.out.push('|');
        self.out.push_str(&t.name);
        self.out.push('|');
        self.emit_attr_items(&t.items);
    }

    fn emit_var_override(&mut self, v: &VarOverride) {
        self.out.push_str("--");
        self.out.push_str(&v.name);
        self.out.push(':');
        self.emit_value(&v.value);
    }

    fn emit_style_def(&mut self, s: &StyleDef) {
        self.out.push('.');
        self.out.push_str(&s.name);
        self.emit_attr_items(&s.items);
    }

    fn emit_shape_def(&mut self, s: &ShapeDef) {
        self.out.push('|');
        self.out.push_str(&s.name);
        self.out.push(':');
        self.out.push_str(&s.base.name);
        self.out.push('|');
        self.emit_attr_items(&s.items);
        self.emit_body(&s.body, s.span.end, 1);
    }

    // ───────── Top-level stmts ─────────

    fn emit_stmt(&mut self, stmt: &Stmt, depth: usize, w: NodeWidths) {
        match stmt {
            Stmt::Node(n) => {
                self.emit_shape_inst(n, depth, w);
                self.out.push('\n');
            }
            Stmt::Wire(wire) => {
                self.emit_wire(wire, depth);
                self.out.push('\n');
            }
        }
    }

    // ───────── Node (shape inst) ─────────

    fn emit_shape_inst(&mut self, inst: &ShapeInst, depth: usize, w: NodeWidths) {
        self.indent(depth);
        let has_label = !inst.labels.is_empty();
        let has_attrs = !inst.items.is_empty();
        let has_body = inst.body.is_some();

        // ID column
        if w.id > 0 {
            match &inst.id {
                Some(id) => {
                    self.out.push_str(id);
                    if w.ty > 0 {
                        pad(self.out, w.id.saturating_sub(id.len()));
                    }
                }
                None => {
                    if w.ty > 0 {
                        pad(self.out, w.id);
                    }
                }
            }
            if w.ty > 0 || inst.id.is_some() {
                self.out.push(' ');
            }
        } else if let Some(id) = &inst.id {
            self.out.push_str(id);
            self.out.push(' ');
        }

        // Type column
        let ty_text = format!("|{}|", inst.ty.name);
        self.out.push_str(&ty_text);
        if w.ty > 0 && (has_label || has_attrs || has_body) {
            pad(self.out, w.ty.saturating_sub(ty_text.len()));
        }

        for label in &inst.labels {
            self.out.push(' ');
            self.emit_string(label);
        }
        self.emit_attr_items(&inst.items);
        self.emit_body(&inst.body, inst.span.end, depth);
    }

    fn emit_body(&mut self, body: &Option<Vec<BodyItem>>, end: usize, depth: usize) {
        let body = match body {
            Some(b) => b,
            None => return,
        };
        if body.is_empty() && !self.has_comment_in(self.cursor, end) {
            self.out.push_str(" {}");
            self.cursor = end;
            return;
        }
        self.out.push_str(" {\n");
        let widths = compute_body_widths(body, self.trivia);
        for (i, c) in body.iter().enumerate() {
            self.emit_trivia_before(body_item_span(c).start, depth + 1);
            self.emit_body_item(c, depth + 1, widths[i]);
            self.cursor = body_item_span(c).end;
        }
        self.emit_trivia_before(end, depth + 1);
        self.indent(depth);
        self.out.push('}');
    }

    fn emit_body_item(&mut self, item: &BodyItem, depth: usize, w: NodeWidths) {
        match item {
            BodyItem::Inst(i) => {
                self.emit_shape_inst(i, depth, w);
                self.out.push('\n');
            }
            BodyItem::Wire(w) => {
                self.emit_wire(w, depth);
                self.out.push('\n');
            }
        }
    }

    // ───────── Wire decl ─────────

    fn emit_wire(&mut self, w: &WireDecl, depth: usize) {
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
                self.out.push_str(&ep.path.join("."));
                if let Some(side) = ep.side {
                    self.out.push('.');
                    self.out.push_str(side_str(side));
                }
            }
        }
        for label in &w.labels {
            self.out.push(' ');
            self.emit_string(label);
        }
        self.emit_attr_items(&w.items);
        if let Some(body) = &w.body {
            self.out.push_str(" {\n");
            for t in body {
                self.emit_trivia_before(t.span.start, depth + 1);
                self.indent(depth + 1);
                self.out.push_str("|text| ");
                self.emit_string(&t.text);
                self.emit_attr_items(&t.items);
                self.out.push('\n');
                self.cursor = t.span.end;
            }
            self.emit_trivia_before(w.span.end, depth + 1);
            self.indent(depth);
            self.out.push('}');
        }
    }

    fn emit_attr_items(&mut self, items: &[AttrItem]) {
        for item in items {
            self.out.push(' ');
            match item {
                AttrItem::Style(s) => {
                    self.out.push('.');
                    self.out.push_str(&s.name);
                }
                AttrItem::Attr(a) => {
                    self.out.push_str(&a.name);
                    self.out.push(':');
                    self.emit_value(&a.value);
                }
            }
        }
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
            Value::Tuple(items) => {
                self.out.push('(');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        self.out.push_str(", ");
                    }
                    self.emit_value(item);
                }
                self.out.push(')');
            }
            Value::List(items) => {
                self.out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        self.out.push_str(", ");
                    }
                    self.emit_value(item);
                }
                self.out.push(']');
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
            Value::RawCssVar(n) => {
                self.out.push_str("--");
                self.out.push_str(n);
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
                    if !last_was_blank && !self.out.ends_with("\n\n") {
                        self.out.push('\n');
                        last_was_blank = true;
                    }
                }
            }
        }
        self.cursor = until;
    }
}

// ─────────────────────────── Helpers ───────────────────────────

fn stmt_span(stmt: &Stmt) -> Span {
    match stmt {
        Stmt::Node(n) => n.span,
        Stmt::Wire(w) => w.span,
    }
}

fn body_item_span(item: &BodyItem) -> Span {
    match item {
        BodyItem::Inst(i) => i.span,
        BodyItem::Wire(w) => w.span,
    }
}

fn wire_op_str(op: WireOp) -> String {
    format!(
        "{}{}{}",
        op.start.start_str(),
        op.line.as_str(),
        op.end.end_str(),
    )
}

fn side_str(s: crate::ast::Side) -> &'static str {
    use crate::ast::Side::*;
    match s {
        Top => "top",
        Bottom => "bottom",
        Left => "left",
        Right => "right",
    }
}

// ─────────────────────────── Column alignment ───────────────────────────

#[derive(Default, Clone, Copy)]
struct NodeWidths {
    id: usize, // 0 if no ids in the group
    ty: usize, // includes leading & trailing '|'
}

fn compute_root_widths(stmts: &[Stmt], trivia: &[TriviaToken]) -> Vec<NodeWidths> {
    let groups = split_groups(stmts, trivia, stmt_span);
    let mut out = vec![NodeWidths::default(); stmts.len()];
    for g in groups {
        let mut w = NodeWidths::default();
        for &i in &g {
            if let Stmt::Node(inst) = &stmts[i] {
                if let Some(id) = &inst.id {
                    w.id = w.id.max(id.len());
                }
                w.ty = w.ty.max(inst.ty.name.len() + 2); // |name|
            }
        }
        for i in g {
            out[i] = w;
        }
    }
    out
}

fn compute_body_widths(items: &[BodyItem], trivia: &[TriviaToken]) -> Vec<NodeWidths> {
    let groups = split_groups(items, trivia, body_item_span);
    let mut out = vec![NodeWidths::default(); items.len()];
    for g in groups {
        let mut w = NodeWidths::default();
        for &i in &g {
            if let BodyItem::Inst(inst) = &items[i] {
                if let Some(id) = &inst.id {
                    w.id = w.id.max(id.len());
                }
                w.ty = w.ty.max(inst.ty.name.len() + 2);
            }
        }
        for i in g {
            out[i] = w;
        }
    }
    out
}

fn split_groups<T>(
    items: &[T],
    trivia: &[TriviaToken],
    span_of: impl Fn(&T) -> Span,
) -> Vec<Vec<usize>> {
    if items.is_empty() {
        return Vec::new();
    }
    let mut groups: Vec<Vec<usize>> = vec![vec![0]];
    for i in 1..items.len() {
        let prev_end = span_of(&items[i - 1]).end;
        let curr_start = span_of(&items[i]).start;
        let has_blank = trivia.iter().any(|t| {
            matches!(t.kind, Trivia::BlankLine) && t.pos >= prev_end && t.pos < curr_start
        });
        if has_blank {
            groups.push(vec![i]);
        } else {
            groups.last_mut().unwrap().push(i);
        }
    }
    groups
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
