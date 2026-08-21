//! Table / entity structure [SPEC 8] — everything the grid's **column count**
//! decides.
//!
//! A `|table|`'s auto-header row, its per-column alignment, and an `|entity|`'s
//! full-width `|header|` / `|footer|` all need to know how many columns the
//! grid will lay out. That number is the cascade's: it can arrive from a class,
//! a descendant or id rule, a user template, or a folded `repeat((…))`, none of
//! which the source text shows. So the structure is decided **here**, against
//! the resolved `columns:`, and desugar keeps only the count-free half (wrapping
//! each bare-text cell in a `|cell|`, and lowering an entity's label to its
//! title `|header|`). One reader of `columns`, so the sugar and the grid can
//! never lay out at different widths.

use super::cascade::{NodeFacts, Stylesheet};
use super::ir::{AttrMap, ResolvedValue};
use super::tracks;
use crate::error::Error;
use crate::span::Span;
use crate::syntax::ast::{Child, Decl, Node, Value};

/// Decorate a `|table|` / `|entity|`'s children with what its resolved column
/// count decides, consuming the table's own `align` / `justify` into its cells.
/// `None` when the node is not a table (or declares no readable `columns:` —
/// the grid raises that). The returned children replace the node's own.
pub(super) fn decorate(
    sheet: &Stylesheet,
    ancestors: &[NodeFacts],
    facts: &NodeFacts,
    type_chain: &[String],
    children: &[Child],
    attrs: &mut AttrMap,
    span: Span,
) -> Result<Option<Vec<Child>>, Error> {
    let is_entity = type_chain.iter().any(|t| t == "entity");
    if !is_entity && !type_chain.iter().any(|t| t == "table") {
        return Ok(None);
    }
    let Some(columns) = attrs.get("columns") else {
        return Ok(None);
    };
    let cols = tracks::parse(columns, span)?.len();
    if cols == 0 {
        return Ok(None);
    }
    // The children's own matcher chain — this table appended to its ancestors.
    // Built here, past every early return, so a plain node never pays for it.
    let mut chain = ancestors.to_vec();
    chain.push(facts.clone());
    let chain = chain.as_slice();
    let mut kids = children.to_vec();
    // An entity's `|header|` / `|footer|` cells span the full width [SPEC 8] —
    // its title (lowered from the label by desugar) and any hand-written band
    // alike, one rule for both.
    if is_entity {
        for child in &mut kids {
            if let Child::Box(n) = child
                && (wears(n, "lini-header") || wears(n, "lini-footer"))
                && !declares(sheet, chain, n, "span")
            {
                n.style.push(number_decl("span", cols, n.span));
            }
        }
    // A table's **first row becomes its header** [SPEC 8]: its leading `cols`
    // in-flow children, when every one is a plain body cell. A row holding a
    // box of its own, an authored `|header|`, or a `cell:`-placed child is a
    // custom layout, not a header, and is left alone.
    } else {
        let row: Vec<usize> = flow_children(sheet, chain, &kids).take(cols).collect();
        if row.len() == cols && row.iter().all(|i| is_plain_cell(sheet, chain, &kids[*i])) {
            for i in row {
                if let Child::Box(n) = &mut kids[i] {
                    n.classes.insert(0, "lini-header".to_string());
                }
            }
        }
    }
    distribute_alignment(sheet, chain, &mut kids, attrs, cols, is_entity);
    Ok(Some(kids))
}

/// Carry the table's own `align` / `justify` onto its cells [SPEC 8]. Every
/// cell fills its track (the `|table|` bundle's `stretch`), so the author's
/// value cannot also pack the boxes — it places each cell's *text* instead,
/// applied to the cells of its own column as a shared `.lini-align-*` /
/// `.lini-justify-*` class rather than an inlined copy per cell. The table's
/// own value is then consumed: the bundle's packing stands, so the cells fill.
///
/// Only auto-flow cells are covered — a `cell:`-placed child (an entity's
/// spanning title among them) keeps the column default.
fn distribute_alignment(
    sheet: &Stylesheet,
    chain: &[NodeFacts],
    kids: &mut [Child],
    attrs: &mut AttrMap,
    cols: usize,
    is_entity: bool,
) {
    let h = authored(attrs, "align", cols)
        // An entity's field rows read left by default [SPEC 8]; its title is
        // `cell:`-placed, so it keeps the centred band.
        .or_else(|| is_entity.then(|| vec!["start".to_string(); cols]));
    let v = authored(attrs, "justify", cols);
    for name in ["align", "justify"] {
        match bundle_packing(name) {
            Some(v) => attrs.insert(name, ResolvedValue::Ident(v)),
            None => attrs.remove(name),
        };
    }
    if h.is_none() && v.is_none() {
        return;
    }
    let flow: Vec<usize> = flow_children(sheet, chain, kids).collect();
    for (n, i) in flow.into_iter().enumerate() {
        let col = n % cols;
        let Child::Box(cell) = &mut kids[i] else {
            continue;
        };
        for (list, axis) in [(&h, "align"), (&v, "justify")] {
            if let Some(vals) = list
                && matches!(vals[col].as_str(), "start" | "end")
            {
                let class = format!("lini-{axis}-{}", vals[col]);
                if !cell.classes.contains(&class) {
                    cell.classes.push(class);
                }
            }
        }
    }
}

/// The table's **own** `align:` / `justify:` — the author's, per column. The
/// resolved value always carries the `|table|` bundle's packing when nobody
/// overrode it, so the bundle's own value reads as "unset": it is the base
/// every cell fills by, never a column alignment the author asked for.
fn authored(attrs: &AttrMap, name: &str, cols: usize) -> Option<Vec<String>> {
    let vals = per_column(attrs.get(name), cols)?;
    let base = bundle_packing(name);
    let untouched = vals.iter().all(|v| Some(v.as_str()) == base.as_deref());
    (!untouched).then_some(vals)
}

/// A resolved `align:` / `justify:` as one keyword per column — the comma law
/// [SPEC 2]: a scalar repeats to every column, a list maps by position (a short
/// list repeats its first). `None` when unset or carrying no keyword.
fn per_column(value: Option<&ResolvedValue>, cols: usize) -> Option<Vec<String>> {
    let mut vals: Vec<String> = Vec::new();
    match value? {
        ResolvedValue::Ident(s) => vals.push(s.clone()),
        ResolvedValue::List(items) => {
            for item in items {
                if let ResolvedValue::Ident(s) = item {
                    vals.push(s.clone());
                }
            }
        }
        _ => {}
    }
    let first = vals.first().cloned()?;
    Some(
        (0..cols)
            .map(|c| vals.get(c).cloned().unwrap_or_else(|| first.clone()))
            .collect(),
    )
}

/// The `|table|` bundle's own `align` / `justify` — the `stretch` that makes
/// every cell fill its track [SPEC 8]. Read from the bundle, so the value keeps
/// its one home in the ledger.
fn bundle_packing(name: &str) -> Option<String> {
    crate::ledger::defaults::template_bundle("table")
        .iter()
        .rev()
        .find(|d| d.name == name)?
        .ident()
        .map(str::to_string)
}

/// The children that take a **track** — the grid's auto-flow, in source order.
/// A `cell:`-placed child holds its own slot (an entity's spanning title among
/// them) and a `pin`ned one is out of the flow entirely (the caption a table's
/// label lowers to), so neither joins a row. Every remaining child is a box:
/// a table's bare text is already wrapped in its `|cell|` by desugar.
fn flow_children<'a>(
    sheet: &'a Stylesheet,
    chain: &'a [NodeFacts],
    kids: &'a [Child],
) -> impl Iterator<Item = usize> + 'a {
    kids.iter().enumerate().filter_map(move |(i, child)| {
        let Child::Box(n) = child else { return None };
        (!declares(sheet, chain, n, "cell") && !is_pinned(sheet, chain, n)).then_some(i)
    })
}

/// Whether a child is a `pin`ned overlay [SPEC 5] — out of the flow, so it
/// holds no track. The caption a table's label lowers to is the common one, and
/// its `pin` is its template's, never its own block: the reading has to be the
/// cascade's or the row shifts under a rule the child never spells out.
fn is_pinned(sheet: &Stylesheet, chain: &[NodeFacts], n: &Node) -> bool {
    if let Some(d) = n.style.iter().rev().find(|d| d.name == "pin") {
        // `pin: top left` is two idents, so only an explicit `none` opts out.
        return !matches!(d.ident(), Some("none"));
    }
    super::pins_out_of_flow(from_rules(sheet, chain, n, "pin").as_ref())
}

/// Whether the child's cascade sets `name` at all — its own block, else any
/// rule that reaches it.
fn declares(sheet: &Stylesheet, chain: &[NodeFacts], n: &Node, name: &str) -> bool {
    n.style.iter().any(|d| d.name == name) || from_rules(sheet, chain, n, name).is_some()
}

/// The value the **rules** give a child for `name`, walked as the child's own
/// cascade will walk them [SPEC 4]: the id / class / descendant layers
/// (most-specific last), else the type tier its `.lini-*` classes carry
/// (derived → base). The child's own `{ }` block is its caller's to check.
fn from_rules(
    sheet: &Stylesheet,
    chain: &[NodeFacts],
    n: &Node,
    name: &str,
) -> Option<ResolvedValue> {
    let facts = NodeFacts {
        classes: n.classes.clone(),
        id: n.id.clone(),
    };
    let layered = sheet
        .node_layers(chain, &facts)
        .into_iter()
        .rev()
        .find_map(|(k, v)| (k == name).then_some(v));
    layered.or_else(|| {
        n.classes
            .iter()
            .filter(|c| c.starts_with("lini-"))
            .find_map(|c| {
                sheet
                    .class_decls(c)
                    .into_iter()
                    .rev()
                    .find_map(|(k, v)| (k == name).then_some(v))
            })
    })
}

/// A plain body cell — a `|cell|` wrapping one text leaf, placed by auto-flow.
/// The shape desugar's cell wrapping produces, and the shape an authored
/// `|cell| "…"` produces: the header row reads them alike.
fn is_plain_cell(sheet: &Stylesheet, chain: &[NodeFacts], child: &Child) -> bool {
    let Child::Box(n) = child else { return false };
    wears(n, "lini-cell")
        && !wears(n, "lini-header")
        && !wears(n, "lini-footer")
        && !declares(sheet, chain, n, "cell")
        && !declares(sheet, chain, n, "span")
        && matches!(n.children.as_slice(), [Child::Text(_)])
}

fn wears(n: &Node, class: &str) -> bool {
    n.classes.iter().any(|c| c == class)
}

fn number_decl(name: &str, n: usize, span: Span) -> Decl {
    Decl {
        name: name.to_string(),
        groups: vec![vec![Value::Number(n as f64)]],
        span,
    }
}

#[cfg(test)]
mod tests {
    use crate::resolve::{NodeKind, ResolvedInst};
    use crate::testutil::program;

    fn root<'a>(p: &'a crate::resolve::Program, id: &str) -> &'a ResolvedInst {
        p.scene
            .nodes
            .iter()
            .find(|n| n.id.as_deref() == Some(id))
            .expect("node")
    }
    fn wears(n: &ResolvedInst, class: &str) -> bool {
        n.type_chain.iter().any(|c| c == class)
    }
    /// The cells that hold a track, in source order — the caption a table's
    /// label lowers to is pinned and skipped, as the pass skips it.
    fn cells(n: &ResolvedInst) -> Vec<&ResolvedInst> {
        n.children
            .iter()
            .filter(|c| wears(c, "cell") && c.attrs.get("cell").is_none())
            .collect()
    }
    fn title(n: &ResolvedInst) -> &ResolvedInst {
        n.children
            .iter()
            .find(|c| wears(c, "header") && c.attrs.get("cell").is_some())
            .expect("the entity title")
    }

    const ROWS: &str = r#" [ "PK" "id" "int"  "" "name" "varchar" ] "#;

    /// The title spans **every** column however `columns:` reaches the node —
    /// its own block, a worn class, or a user template. The count is the
    /// cascade's, so all three must agree [SPEC 8].
    #[test]
    fn an_entity_title_spans_the_resolved_column_count() {
        let span_of = |src: &str| {
            let p = program(src);
            title(root(&p, "x")).attrs.number("span").expect("a span")
        };
        assert_eq!(
            span_of(&format!(
                "|entity#x| \"T\" {{ columns: auto, auto, auto }}{ROWS}"
            )),
            3.0
        );
        assert_eq!(
            span_of(&format!(
                "{{ .t {{ columns: auto, auto, auto; }} }}\n|entity#x| \"T\" .t{ROWS}"
            )),
            3.0,
            "a class-borne columns reaches the title"
        );
        assert_eq!(
            span_of(&format!(
                "{{ |t::entity| {{ columns: repeat(3); }} }}\n|t#x| \"T\"{ROWS}"
            )),
            3.0,
            "a user template's columns reaches the title"
        );
        assert_eq!(
            span_of(&format!(
                "{{ #x {{ columns: repeat(3); }} }}\n|entity#x| \"T\"{ROWS}"
            )),
            3.0,
            "an id rule's columns reaches the title"
        );
        // The bundle's own two columns still stand when nobody overrides.
        assert_eq!(span_of("|entity#x| \"T\" [ \"id\" \"int\" ]"), 2.0);
    }

    /// A `|table|`'s first row is its header band and its `align:` is carried
    /// onto the cells — both read the same resolved count, so a class-borne
    /// `columns:` gets them exactly as an inline one does [SPEC 8].
    #[test]
    fn a_class_borne_columns_still_headers_and_aligns_the_table() {
        let src = "{ .t { columns: 80, 140, 80; align: start, center, end; } }\n\
                   |table#b| .t [ \"Fruit\" \"Qty\" \"Notes\"  \"Apple\" \"12\" \"fresh\" ]";
        let p = program(src);
        let b = root(&p, "b");
        let cells = cells(b);
        assert_eq!(cells.len(), 6);
        assert!(
            cells[..3].iter().all(|c| wears(c, "header")),
            "the first row is the header band"
        );
        assert!(
            cells[3..].iter().all(|c| !wears(c, "header")),
            "the body is not"
        );
        // Column 0 reads start, column 2 end — header row and body row alike.
        for row in [0, 3] {
            assert!(wears(cells[row], "align-start"));
            assert!(!wears(cells[row + 1], "align-start") && !wears(cells[row + 1], "align-end"));
            assert!(wears(cells[row + 2], "align-end"));
        }
        // The table's own packing is consumed: the bundle's `stretch` stands,
        // so every cell still fills its track.
        assert!(matches!(
            b.attrs.get("align"),
            Some(crate::resolve::ResolvedValue::Ident(s)) if s == "stretch"
        ));
    }

    /// `pin:` and `cell:` decide which children hold a track, and both are far
    /// more often a rule's than a child's own block (a table's caption is
    /// pinned by its template). Reading only the block would shift the row
    /// under a rule the cell never spells out.
    #[test]
    fn a_rule_borne_pin_or_cell_still_leaves_the_flow() {
        let p = program(
            "{ .float { pin: top right; } }\n\
             |table#t| { columns: 40, 40 } [ |cell| \"x\" .float; \"a\" \"b\" \"c\" \"d\" ]",
        );
        let t = root(&p, "t");
        let headers: Vec<&str> = t
            .children
            .iter()
            .filter(|c| wears(c, "header"))
            .filter_map(|c| c.children.first()?.label.as_deref())
            .collect();
        assert_eq!(headers, ["a", "b"], "the pinned cell holds no track");

        let p = program(
            "{ #odd { cell: 2 2; } }\n\
             |table#t| { columns: 40, 40 } [ |cell#odd| \"x\"; \"a\" \"b\" \"c\" \"d\" ]",
        );
        let t = root(&p, "t");
        let headers: Vec<&str> = t
            .children
            .iter()
            .filter(|c| wears(c, "header"))
            .filter_map(|c| c.children.first()?.label.as_deref())
            .collect();
        assert_eq!(headers, ["a", "b"], "the placed cell holds its own slot");
    }

    /// A first row that is not plain cells is a custom layout, not a header.
    #[test]
    fn an_authored_first_row_is_left_alone() {
        let p = program("|table#t| { columns: 40, 40 } [ |box| \"a\"; \"b\" \"c\" \"d\" ]");
        let t = root(&p, "t");
        assert!(
            t.children.iter().all(|c| !wears(c, "header")),
            "a box in the first row means no auto-header"
        );
        assert!(t.children.iter().any(|c| c.kind == NodeKind::Block));
    }
}
