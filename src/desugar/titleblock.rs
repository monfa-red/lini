//! `|title-block|` field sugar [SPEC 15.8]. ISO 7200 fields — string-valued
//! properties on the block — desugar into a fixed grid: the title spans the
//! top, the rest flow three per row, each a small muted caption over its value.
//! **Absent fields collapse** (no cell), so the default block is minimal
//! (Title / Dwg No. / Rev / Sheet). A `|title-block|` with **no** field
//! property keeps the plain-table form — its cells fully authored.

use crate::span::Span;
use crate::syntax::ast::{Child, Decl, Node, TextNode, Value};

/// The ISO 7200 fields, in block order, with their captions. `title` leads and
/// spans the width; the rest flow into the grid.
const FIELDS: &[(&str, &str)] = &[
    ("title", "Title"),
    ("dwg", "Dwg No."),
    ("rev", "Rev"),
    ("sheet", "Sheet"),
    ("date", "Date"),
    ("author", "Drawn"),
    ("approved", "Approved"),
    ("dept", "Dept"),
    ("reference", "Reference"),
    ("doc-type", "Type"),
    ("status", "Status"),
];

/// The grid's column count — the title spans them all.
const COLUMNS: usize = 3;
/// The caption's size (the muted footer tone); the value keeps the block's font.
const CAPTION_SIZE: f64 = 7.0;

/// Whether a `|title-block|`'s style carries any ISO 7200 field.
pub(super) fn has_fields(style: &[Decl]) -> bool {
    FIELDS.iter().any(|(k, _)| field_value(style, k).is_some())
}

/// Expand a field-carrying `|title-block|` [SPEC 15.8]: return one `|cell|` per
/// **present** field (a caption over its value; the title spanning), and set
/// the grid's `columns:`. The field decls are consumed from `style`.
pub(super) fn expand_fields(style: &mut Vec<Decl>, span: Span) -> Vec<Node> {
    let cells: Vec<Node> = FIELDS
        .iter()
        .filter_map(|(key, cap)| {
            field_value(style, key).map(|v| {
                let cols = (*key == "title").then_some(COLUMNS);
                field_cell(cap, &v, cols, span)
            })
        })
        .collect();
    style.retain(|d| !FIELDS.iter().any(|(k, _)| *k == d.name));
    if !style.iter().any(|d| d.name == "columns") {
        style.push(Decl {
            name: "columns".into(),
            groups: vec![vec![Value::Ident("auto".into()); COLUMNS]],
            span,
        });
    }
    cells
}

/// A field's string value, when present.
fn field_value(style: &[Decl], key: &str) -> Option<String> {
    match style
        .iter()
        .find(|d| d.name == key)
        .and_then(|d| d.groups.first())
        .and_then(|g| g.first())
    {
        Some(Value::String(s)) => Some(s.clone()),
        _ => None,
    }
}

/// One field cell: a small, muted caption stacked over the value (a `|cell|` is
/// a column-flow block, so its two text leaves stack). The title cell spans.
fn field_cell(caption: &str, value: &str, span_cols: Option<usize>, span: Span) -> Node {
    let mut style = Vec::new();
    if let Some(cols) = span_cols {
        style.push(Decl {
            name: "span".into(),
            groups: vec![vec![Value::Number(cols as f64), Value::Number(1.0)]],
            span,
        });
    }
    Node {
        id: None,
        ty: Some("cell".into()),
        label: None,
        classes: Vec::new(),
        style,
        style_span: None,
        children: vec![
            Child::Text(caption_text(caption, span)),
            Child::Text(text(value, span)),
        ],
        links: Vec::new(),
        span,
    }
}

/// The muted field caption — small, in the footer tone.
fn caption_text(s: &str, span: Span) -> TextNode {
    TextNode {
        text: s.to_string(),
        style: vec![
            Decl {
                name: "font-size".into(),
                groups: vec![vec![Value::Number(CAPTION_SIZE)]],
                span,
            },
            Decl {
                name: "color".into(),
                groups: vec![vec![Value::Var("footer-color".into())]],
                span,
            },
        ],
        style_span: None,
        span,
    }
}

/// A bare value leaf — inherits the block's font (11).
fn text(s: &str, span: Span) -> TextNode {
    TextNode {
        text: s.to_string(),
        style: Vec::new(),
        style_span: None,
        span,
    }
}

#[cfg(test)]
mod tests {
    use crate::syntax::ast::{Child, Node};

    fn title_block(src: &str) -> Node {
        let toks = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(src, &toks).expect("parse");
        let lowered = crate::desugar::desugar(&file).expect("desugar");
        fn find(children: &[Child]) -> Option<Node> {
            for c in children {
                if let Child::Box(n) = c {
                    if n.classes.iter().any(|k| k == "lini-title-block") {
                        return Some(n.clone());
                    }
                    if let Some(hit) = find(&n.children) {
                        return Some(hit);
                    }
                }
            }
            None
        }
        find(&lowered.instances).expect("a title block")
    }

    fn cells(n: &Node) -> Vec<&Node> {
        n.children
            .iter()
            .filter_map(|c| match c {
                Child::Box(b) if b.classes.iter().any(|k| k == "lini-cell") => Some(b),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn present_fields_become_cells_and_absent_collapse() {
        let tb = title_block(
            "|page#p| [\n  |drawing#v| [ |rect#r| { width: 10; height: 10 } ]\n  |title-block| { title: \"T\"; dwg: \"D\"; rev: \"A\" }\n]\n",
        );
        // Three present fields → three cells (author, date, … absent: no cell).
        let cs = cells(&tb);
        assert_eq!(cs.len(), 3, "one cell per present field");
        // The title cell spans the columns.
        assert!(
            cs[0].style.iter().any(|d| d.name == "span"),
            "the title spans"
        );
        assert!(
            tb.style.iter().any(|d| d.name == "columns"),
            "the grid gets columns"
        );
    }

    #[test]
    fn no_field_keeps_the_plain_table_form() {
        let tb = title_block(
            "|page#p| [\n  |drawing#v| [ |rect#r| { width: 10; height: 10 } ]\n  |title-block| { columns: 40 auto } [ \"Scale\" \"1:1\" ]\n]\n",
        );
        // The authored cells stand; no field grid is synthesized.
        assert_eq!(cells(&tb).len(), 2, "the two authored cells");
        assert!(tb.style.iter().any(|d| d.name == "columns"));
    }
}
