//! Table / entity / grid structure lowering [SPEC 8]: grid column count, the
//! auto-header and body-cell wrapping, and per-column align/justify distribution.

use super::*;
use crate::syntax::ast::TextNode;

/// The grid column count for a table / entity node [SPEC 8]: its own `columns:` decl,
/// else a bundle default in its chain (`entity` carries `columns: auto auto`). `None`
/// when undeterminable — the auto-header and title-span then no-op.
pub(super) fn column_count(style: &[Decl], chain: &[String]) -> Option<usize> {
    if let Some(d) = style.iter().find(|d| d.name == "columns") {
        let n = count_tracks(d);
        if n > 0 {
            return Some(n);
        }
    }
    chain.iter().rev().find_map(|name| {
        let n = count_tracks(
            crate::ledger::defaults::template_bundle(name)
                .iter()
                .find(|d| d.name == "columns")?,
        );
        (n > 0).then_some(n)
    })
}

/// Tracks a `columns:` value declares — each token is one track, `repeat(N)` is N.
fn count_tracks(d: &Decl) -> usize {
    d.groups
        .iter()
        .flatten()
        .map(|v| match v {
            Value::Call(c) if c.name == "repeat" => match c.args.first() {
                Some(Value::Number(n)) if *n >= 1.0 => *n as usize,
                _ => 1,
            },
            _ => 1,
        })
        .sum()
}

/// A `|header|` node carrying `text` [SPEC 8]. With `span`, it is an `|entity|`'s title
/// at the grid top-left; without, it wraps one bare-text table cell (the auto-header).
pub(super) fn header_node(text: &TextNode, span: Option<usize>) -> Node {
    let style = match span {
        Some(cols) => vec![
            decl("cell", vec![Value::Number(1.0), Value::Number(1.0)]),
            decl("span", vec![Value::Number(cols as f64)]),
        ],
        None => Vec::new(),
    };
    Node {
        id: None,
        ty: Some("header".into()),
        label: Some(text.clone()),
        classes: Vec::new(),
        style,
        style_span: None,
        children: Vec::new(),
        links: Vec::new(),
        span: text.span,
    }
}

/// A `|cell|` wrapping one bare-text table/entity body cell [SPEC 8]: the text
/// node survives inside it, and the `|cell|` type carries the padding inset and the
/// column's alignment class. Header/footer/box cells stay as they are.
fn block_cell(text: &TextNode) -> Node {
    Node {
        id: None,
        ty: Some("cell".into()),
        label: None,
        classes: Vec::new(),
        style: Vec::new(),
        style_span: None,
        children: vec![Child::Text(text.clone())],
        links: Vec::new(),
        span: text.span,
    }
}

/// Wrap each remaining bare-text body cell of a `|table|`/`|entity|` in a `|cell|`
/// [SPEC 8], the box that carries the cell padding. Header/footer/box cells are
/// already boxes and pass through; re-desugar is a fixed point (a wrapped cell is a
/// box, not text, so it is never re-wrapped).
pub(super) fn wrap_body_cells(
    children: &mut [Child],
    types: &Types,
    bodies: &Bodies,
) -> Result<(), Error> {
    for c in children.iter_mut() {
        if let Child::Text(t) = c {
            *c = Child::Box(lower_node(&block_cell(t), types, bodies, false)?);
        }
    }
    Ok(())
}

/// Carry a table's per-column `align`/`justify` down to its cells [SPEC 8]. Each is
/// one keyword per column (a scalar repeats), applied to the cell in that column by
/// auto-flow order (`i % cols`); `center`/`stretch` add nothing (the cell already
/// centres / fills). A `start`/`end` column wears a `.lini-align-*` / `.lini-justify-*`
/// class (defined in `classes`), so a whole column shares one class — not an inlined
/// copy per cell — and the grid honours it once it has stretched the cell.
pub(super) fn distribute_cell_alignment(
    children: &mut [Child],
    table_style: &[Decl],
    cols: usize,
    is_entity: bool,
) {
    let h = per_column(table_style, "align", cols)
        // An entity's field rows read left by default [SPEC 8]; the title header is
        // inserted *after* this pass, so it keeps its centred, full-span default.
        .or_else(|| is_entity.then(|| vec!["start".to_string(); cols]));
    let v = per_column(table_style, "justify", cols);
    if h.is_none() && v.is_none() {
        return;
    }
    for (i, child) in children.iter_mut().enumerate() {
        let Child::Box(cell) = child else { continue };
        let col = i % cols;
        for (list, axis) in [(&h, "align"), (&v, "justify")] {
            if let Some(vals) = list
                && matches!(vals[col].as_str(), "start" | "end")
            {
                let class = lini_class(&format!("{axis}-{}", vals[col]));
                if !cell.classes.contains(&class) {
                    cell.classes.push(class);
                }
            }
        }
    }
}

/// A table property's value as one keyword per column: a scalar repeats to every
/// column, a list maps by position (a short list repeats its first). `None` when
/// the property is absent or carries no keyword.
fn per_column(style: &[Decl], name: &str, cols: usize) -> Option<Vec<String>> {
    let d = style.iter().find(|d| d.name == name)?;
    let vals: Vec<String> = d
        .groups
        .iter()
        .flatten()
        .filter_map(|v| match v {
            Value::Ident(s) => Some(s.clone()),
            _ => None,
        })
        .collect();
    let first = vals.first()?.clone();
    Some(
        (0..cols)
            .map(|c| vals.get(c).cloned().unwrap_or_else(|| first.clone()))
            .collect(),
    )
}

/// Auto-header a `|table|`'s first row [SPEC 8]: wrap the first `cols` children as
/// `|header|` cells when they are all bare text. A first row holding a box or an
/// explicit `cell:` is left alone — that is a custom layout, not a header.
pub(super) fn wrap_header_row(
    children: &mut [Child],
    cols: usize,
    types: &Types,
    bodies: &Bodies,
) -> Result<(), Error> {
    let row_end = cols.min(children.len());
    if row_end == 0
        || !children[..row_end]
            .iter()
            .all(|c| matches!(c, Child::Text(_)))
    {
        return Ok(());
    }
    for c in &mut children[..row_end] {
        if let Child::Text(t) = c {
            *c = Child::Box(lower_node(&header_node(t, None), types, bodies, false)?);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lower(src: &str) -> File {
        let toks = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(src, &toks).expect("parse");
        desugar(&file).expect("desugar")
    }
    fn root_box<'a>(f: &'a File, id: &str) -> &'a Node {
        f.instances
            .iter()
            .find_map(|c| match c {
                Child::Box(n) if n.id.as_deref() == Some(id) => Some(n),
                _ => None,
            })
            .expect("node")
    }
    fn is_header(c: &Child) -> bool {
        matches!(c, Child::Box(n) if n.classes.iter().any(|x| x == "lini-header"))
    }
    /// A body cell is now a frameless `|block|` wrapping its bare text [SPEC 8].
    fn is_block_cell(c: &Child) -> bool {
        matches!(c, Child::Box(n)
            if n.classes.iter().any(|x| x == "lini-block")
            && matches!(n.children.as_slice(), [Child::Text(_)]))
    }

    #[test]
    fn table_first_row_becomes_header_cells() {
        let f = lower("|table#t| { columns: 30 30 } [\n\"a\"\n\"b\"\n\"c\"\n\"d\"\n]\n");
        let t = root_box(&f, "t");
        // Row 0 (the first `cols` cells) are header boxes; body cells are `|block|`s.
        assert!(
            is_header(&t.children[0]) && is_header(&t.children[1]),
            "first row is header"
        );
        assert!(
            is_block_cell(&t.children[2]) && is_block_cell(&t.children[3]),
            "body cells wrap in |block|"
        );
    }

    #[test]
    fn entity_label_is_a_spanning_header_fields_wrap_in_blocks() {
        let f = lower("|entity#e| \"Users\" [\n\"id\"\n\"int\"\n]\n");
        let e = root_box(&f, "e");
        let Child::Box(title) = &e.children[0] else {
            panic!("the entity title is a box");
        };
        assert!(title.classes.iter().any(|c| c == "lini-header"));
        assert!(
            title.style.iter().any(|d| d.name == "span"),
            "the title spans its columns"
        );
        // Field rows are not auto-headered — only the label is the title — but each
        // field cell now wraps in a `|block|`.
        assert!(is_block_cell(&e.children[1]) && is_block_cell(&e.children[2]));
    }

    #[test]
    fn table_distributes_per_column_align_to_cells() {
        // The table's own `align` is consumed (dropped, so the bundle's `stretch`
        // fills the cells) and carried to each cell by column [SPEC 8].
        let f = lower(
            "|table#t| { columns: 40 40; align: start end } [\n\"a\"\n\"b\"\n\"c\"\n\"d\"\n]\n",
        );
        let t = root_box(&f, "t");
        assert!(
            t.style.iter().all(|d| d.name != "align"),
            "the table's own align is consumed"
        );
        // Each start/end column's cells wear a shared alignment class (not inlined).
        let cell_class = |i: usize| match &t.children[i] {
            Child::Box(n) => n
                .classes
                .iter()
                .find(|c| c.starts_with("lini-align-"))
                .cloned(),
            _ => None,
        };
        // Columns 0/1 → start/end, for the header row (a, b) and the body row (c, d).
        assert_eq!(cell_class(0).as_deref(), Some("lini-align-start"));
        assert_eq!(cell_class(1).as_deref(), Some("lini-align-end"));
        assert_eq!(cell_class(2).as_deref(), Some("lini-align-start"));
        assert_eq!(cell_class(3).as_deref(), Some("lini-align-end"));
    }

    #[test]
    fn table_cells_get_lini_cell_but_the_caption_does_not() {
        // Cells are `|cell|`s (which carry the padding); a table's caption is a plain
        // `|block|`, not a `|cell|` [SPEC 8], so it must not wear `.lini-cell` — else
        // its title text would be inset like a cell.
        let f = lower("|table#t| \"Cap\" { columns: 30 30 } [\n\"a\"\n\"b\"\n\"c\"\n\"d\"\n]\n");
        let t = root_box(&f, "t");
        let Child::Box(cap) = &t.children[0] else {
            panic!("the caption is a box");
        };
        assert!(cap.classes.iter().any(|c| c == "lini-caption"));
        assert!(
            !cap.classes.iter().any(|c| c == "lini-cell"),
            "the caption is not a cell"
        );
        // Every actual cell carries `.lini-cell`.
        assert!(
            t.children[1..].iter().all(|c| matches!(
                c, Child::Box(n) if n.classes.iter().any(|x| x == "lini-cell"))),
            "every cell carries lini-cell"
        );
    }

    #[test]
    fn bare_grid_does_not_auto_header_or_wrap() {
        let f = lower("|grid#g| { columns: 30 30 } [\n\"a\"\n\"b\"\n]\n");
        let g = root_box(&f, "g");
        assert!(
            g.children.iter().all(|c| !is_header(c)),
            "a bare grid is not a table — no auto-header"
        );
        // A bare grid is not a table, so its bare-text cells stay bare text.
        assert!(
            g.children.iter().all(|c| matches!(c, Child::Text(_))),
            "bare grid cells stay bare text"
        );
    }
}
