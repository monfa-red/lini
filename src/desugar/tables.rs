//! Table / entity structure lowering [SPEC 8], the count-free half: an
//! `|entity|`'s title `|header|` and the `|cell|` every bare-text body cell
//! wraps in.
//!
//! Everything the grid's **column count** decides — the auto-header row, the
//! per-column alignment, an entity's full-width bands — lives in
//! [`crate::resolve::tables`], which reads the count the cascade settled. This
//! module never counts columns: a source read cannot see a class rule, a user
//! template, or a folded `repeat((…))`, so a count taken here would lay the
//! sugar out at one width while the grid laid the body out at another.

use super::*;
use crate::syntax::ast::TextNode;

/// An `|entity|`'s title [SPEC 8]: the `|header|` carrying its label, placed at
/// the grid's top-left. Its **span** is added at resolve, from the column count
/// the cascade settles.
pub(super) fn header_node(text: &TextNode) -> Node {
    let mut n = synth::labelled("header", text.clone());
    n.style = vec![decl("cell", vec![Value::Number(1.0), Value::Number(1.0)])];
    n
}

/// A `|cell|` wrapping one bare-text table/entity body cell [SPEC 8]: the text
/// node survives inside it, and the `|cell|` type carries the padding inset and the
/// column's alignment class. Header/footer/box cells stay as they are.
fn block_cell(text: &TextNode) -> Node {
    let mut n = synth::node("cell", text.span);
    n.children = vec![Child::Text(text.clone())];
    n
}

/// Wrap each remaining bare-text body cell of a `|table|`/`|entity|` in a `|cell|`
/// [SPEC 8], the box that carries the cell padding. Header/footer/box cells are
/// already boxes and pass through; re-desugar is a fixed point (a wrapped cell is a
/// box, not text, so it is never re-wrapped).
pub(super) fn wrap_body_cells(cx: &super::Lower, children: &mut [Child]) -> Result<(), Error> {
    for c in children.iter_mut() {
        if let Child::Text(t) = c {
            *c = Child::Box(lower_node(cx, &block_cell(t), Nest::NONE)?);
        }
    }
    Ok(())
}

/// The track count a node's **source** declares — its own `columns:`, else a
/// bundle default in its template chain.
///
/// This is a reading of the written text, not of the grid: the cascade's
/// `columns:` can come from a class, a descendant or id rule, or a folded
/// expression, none of which are visible here. So it answers only for callers
/// whose question *is* about the source — `fmt`, padding a table's cells into
/// aligned columns, and a `|title-block|`'s field grid, which writes the
/// `columns:` it reads. **The layout column count is
/// [`crate::resolve::tables`]'.**
pub(crate) fn declared_column_count(style: &[Decl], chain: &[String]) -> Option<usize> {
    // The **last** `columns:` wins, as it does in the cascade [SPEC 4].
    if let Some(d) = style.iter().rev().find(|d| d.name == "columns") {
        let n = d.track_count();
        if n > 0 {
            return Some(n);
        }
    }
    chain.iter().rev().find_map(|name| {
        let n = crate::ledger::defaults::template_bundle(name)
            .iter()
            .find(|d| d.name == "columns")?
            .track_count();
        (n > 0).then_some(n)
    })
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
    /// A body cell is a frameless `|block|` wrapping its bare text [SPEC 8].
    fn is_block_cell(c: &Child) -> bool {
        matches!(c, Child::Box(n)
            if n.classes.iter().any(|x| x == "lini-block")
            && matches!(n.children.as_slice(), [Child::Text(_)]))
    }

    #[test]
    fn table_cells_wrap_but_no_header_row_is_decided_here() {
        let f = lower("|table#t| { columns: 30, 30 } [\n\"a\"\n\"b\"\n\"c\"\n\"d\"\n]\n");
        let t = root_box(&f, "t");
        // Every cell wraps; which of them are the header row is the resolved
        // column count's answer, not this pass's.
        assert!(t.children.iter().all(is_block_cell), "every cell wraps");
        assert!(
            t.children.iter().all(
                |c| matches!(c, Child::Box(n) if !n.classes.iter().any(|x| x == "lini-header"))
            ),
            "no header decided at desugar"
        );
    }

    #[test]
    fn entity_label_lowers_to_a_top_left_header_without_a_span() {
        let f = lower("|entity#e| \"Users\" [\n\"id\"\n\"int\"\n]\n");
        let e = root_box(&f, "e");
        let Child::Box(title) = &e.children[0] else {
            panic!("the entity title is a box");
        };
        assert!(title.classes.iter().any(|c| c == "lini-header"));
        assert!(title.style.iter().any(|d| d.name == "cell"));
        assert!(
            title.style.iter().all(|d| d.name != "span"),
            "the span is the resolved column count's, added at resolve"
        );
        assert!(is_block_cell(&e.children[1]) && is_block_cell(&e.children[2]));
    }

    #[test]
    fn table_cells_get_lini_cell_but_the_caption_does_not() {
        // Cells are `|cell|`s (which carry the padding); a table's caption is a plain
        // `|block|`, not a `|cell|` [SPEC 8], so it must not wear `.lini-cell` — else
        // its title text would be inset like a cell.
        let f = lower("|table#t| \"Cap\" { columns: 30, 30 } [\n\"a\"\n\"b\"\n\"c\"\n\"d\"\n]\n");
        let t = root_box(&f, "t");
        let Child::Box(cap) = &t.children[0] else {
            panic!("the caption is a box");
        };
        assert!(cap.classes.iter().any(|c| c == "lini-caption"));
        assert!(
            !cap.classes.iter().any(|c| c == "lini-cell"),
            "the caption is not a cell"
        );
        assert!(
            t.children[1..].iter().all(|c| matches!(
                c, Child::Box(n) if n.classes.iter().any(|x| x == "lini-cell"))),
            "every cell carries lini-cell"
        );
    }

    #[test]
    fn bare_grid_does_not_wrap_its_cells() {
        let f = lower("|grid#g| { columns: 30, 30 } [\n\"a\"\n\"b\"\n]\n");
        let g = root_box(&f, "g");
        // A bare grid is not a table, so its bare-text cells stay bare text.
        assert!(
            g.children.iter().all(|c| matches!(c, Child::Text(_))),
            "bare grid cells stay bare text"
        );
    }
}
