//! Every built-in default, expressed as parser-shaped [`Decl`]s. This is the one
//! place Lini's look is tuned; desugar lowers these into `.lini-*` class defs and
//! the global block, and `resolve` reads [`link_defaults`] as the baked link base.
//! Visual `--lini-*` colours stay live `--var` references (render emits their
//! defaults as `@layer` CSS).

use crate::resolve::NodeKind;
use crate::span::Span;
use crate::syntax::ast::{Decl, Value};

/// A sequence's default `gap: row col` [SPEC 13] — the message pitch (rows) and the
/// participant spacing (columns), larger than the generic `20` so the time axis breathes.
/// Shared by the `|sequence|` template and the root `{ layout: sequence }` form.
pub(crate) const SEQ_GAP_ROW: f64 = 32.0;
pub(crate) const SEQ_GAP_COL: f64 = 32.0;

fn decl(name: &str, values: Vec<Value>) -> Decl {
    Decl {
        name: name.into(),
        groups: vec![values],
        span: Span::empty(),
    }
}
fn n(name: &str, v: f64) -> Decl {
    decl(name, vec![Value::Number(v)])
}
fn id(name: &str, v: &str) -> Decl {
    decl(name, vec![Value::Ident(v.into())])
}
fn var(name: &str, v: &str) -> Decl {
    decl(name, vec![Value::Var(v.into())])
}
fn pair(name: &str, a: f64, b: f64) -> Decl {
    decl(name, vec![Value::Number(a), Value::Number(b)])
}

/// Root defaults specific to a root `{ layout: X }` engine, layered over [`root_defaults`]
/// (the user's own decls still win). A root sequence breathes like a `|sequence|` node — it
/// gets the same `gap`. The default lives here, so the layout core stays dumb.
pub(crate) fn root_layout_defaults(layout: Option<&str>) -> Vec<Decl> {
    match layout {
        Some("sequence") => vec![pair("gap", SEQ_GAP_ROW, SEQ_GAP_COL)],
        // A drawing's units are millimetres at screen resolution: ~4 px/mm,
        // so the view defaults to 4 [SPEC 15.1]; any authored scale wins.
        Some("drawing") => vec![n("scale", 4.0)],
        _ => Vec::new(),
    }
}

/// A primitive's complete default set (paint + geometry).
pub fn primitive_bundle(kind: NodeKind) -> Vec<Decl> {
    use NodeKind::*;
    // Closed, content-sized primitives share paint + box-model defaults.
    let sized = || {
        vec![
            var("fill", "fill"),
            var("stroke", "stroke"),
            n("stroke-width", 2.0),
            n("padding", 20.0),
            n("gap", 20.0),
        ]
    };
    match kind {
        // The bare rectangle [SPEC 7]: frameless, no padding — like a `div`.
        // It keeps the default `stroke-width` (invisible while `stroke: none`, so
        // bbox geometry is unchanged from the old `|plain|`, and a styled `|block|`
        // gets a sensible 2px border); the `|box|` template lifts paint/radius/
        // padding back on top.
        Block => vec![
            id("fill", "none"),
            id("stroke", "none"),
            n("stroke-width", 2.0),
            n("padding", 0.0),
            n("gap", 20.0),
        ],
        Oval | Hex | Cyl | Diamond => sized(),
        Slant => {
            let mut b = sized();
            b.push(n("skew", 15.0));
            b
        }
        // Geometry-sized closed shapes: paint only, no box model. The sketch pen
        // [SPEC 15.3] paints like them — object lines at the core weight 2.
        Poly | Path | Sketch => vec![
            var("fill", "fill"),
            var("stroke", "stroke"),
            n("stroke-width", 2.0),
        ],
        Line => vec![
            id("fill", "none"),
            var("stroke", "stroke"),
            n("stroke-width", 2.0),
        ],
        // Phosphor icon: painted like a box (fill body, stroke line) at a
        // counter-scaled stroke-width; defaults to a soft-grey duotone at 32px.
        Icon => vec![
            var("fill", "icon-fill"),
            var("stroke", "stroke"),
            n("stroke-width", 2.0),
            n("width", 32.0),
            n("height", 32.0),
        ],
        // Text is structural (render's `lini-text` rule); Image requires src/dims.
        Text | Image => Vec::new(),
    }
}

/// A built-in template's delta over its base [SPEC 8]. Empty for a non-template.
pub fn template_bundle(name: &str) -> Vec<Decl> {
    match name {
        // The default node: a rounded, framed card over the bare `|block|` base.
        "box" => vec![
            var("fill", "fill"),
            var("stroke", "stroke"),
            n("stroke-width", 2.0),
            n("padding", 20.0),
            n("radius", 8.0),
        ],
        "rect" => vec![n("radius", 0.0)],
        "group" => vec![
            var("stroke", "group-stroke"),
            id("stroke-style", "dashed"),
            n("stroke-width", 1.0),
            var("fill", "group-fill"),
            n("radius", 8.0),
            n("padding", 20.0),
        ],
        "caption" => vec![
            decl(
                "pin",
                vec![Value::Ident("top".into()), Value::Ident("left".into())],
            ),
            pair("translate", 0.0, -18.0),
            var("color", "caption-color"),
            n("font-size", 12.0),
            var("font-weight", "caption-font-weight"),
        ],
        "footnote" => vec![
            id("pin", "bottom"),
            pair("translate", 0.0, 17.0),
            n("font-size", 11.0),
            var("color", "footer-color"),
        ],
        "badge" => vec![
            decl(
                "pin",
                vec![Value::Ident("top".into()), Value::Ident("right".into())],
            ),
            pair("translate", 6.0, -6.0),
            n("radius", 8.0),
            pair("padding", 2.0, 6.0),
            decl(
                "shadow",
                vec![Value::Number(2.0), Value::Number(3.0), Value::Number(3.0)],
            ),
            var("fill", "accent"),
            var("color", "accent-text"),
            n("font-size", 11.0),
            id("font-weight", "normal"),
        ],
        // Frameless flow wrappers over |block| [SPEC 8]: the engine is flow by
        // default, so these only set the orientation. |grid| is the grid sibling.
        "row" => vec![id("direction", "row")],
        "column" => vec![id("direction", "column")],
        "grid" => vec![id("layout", "grid")],
        // Chart containers [SPEC 14.1]: the layout preset is the whole bundle,
        // exactly as `table` is `grid + gap-fill`. The chart layout reads everything
        // else (sizes, scales, paint) from the node and its children at layout time.
        // `gap` is the clear space between the plot and the title / legend that sit
        // outside it [SPEC 14.6], overriding the `|block|` base `gap: 20`; the
        // user tunes it (`gap: 0` ≈ touching).
        "chart" => vec![id("layout", "chart"), n("gap", 10.0)],
        "pie" => vec![id("layout", "pie"), n("gap", 10.0)],
        // Sequences [SPEC 13]: the layout preset + the message pitch / participant spacing
        // (`gap`, larger than the generic 20 to breathe), plus the note / frame / separator
        // looks, all reusing scene role variables (no new ones). Participants are ordinary
        // boxes and keep their own type's paint. (A root `{ layout: sequence }` picks up the
        // same `gap` default in `desugar`.)
        "sequence" => vec![
            id("layout", "sequence"),
            pair("gap", SEQ_GAP_ROW, SEQ_GAP_COL),
        ],
        // The note card [SPEC 8] — one type in every layout: shape-like padding
        // in flow / grid, compacted inside a sequence / drawing by the built-in
        // scoped rules ([`scoped_rules`]), sheet-space in a scaled view.
        "note" => vec![
            var("fill", "fill"),
            var("stroke", "stroke"),
            n("padding", 20.0),
            n("scale", 1.0),
        ],
        // The assembly balloon [SPEC 8, 15.8]: a numbered circle a leader points
        // from; sheet-space like all annotation chrome.
        "balloon" => vec![
            n("width", 16.0),
            var("fill", "fill"),
            var("stroke", "stroke"),
            n("font-size", 11.0),
            n("scale", 1.0),
        ],
        // Drawings [SPEC 15]: the container (frameless — the geometry and its
        // annotations are the content)…
        "drawing" => vec![id("layout", "drawing"), n("padding", 0.0), n("scale", 4.0)],
        // …the round hole — `width:` (required) is its diameter; it punches by
        // paint order and draws its own centre marks [SPEC 15.4]…
        "hole" => vec![var("fill", "bg"), var("stroke", "stroke")],
        // …and the dash-dot chrome types [SPEC 15.7]. `|breakline|` is the break
        // cut's generated zigzag / S edge — solid, annotation-weight.
        "centerline" => vec![
            id("stroke-style", "center"),
            var("stroke", "stroke-light"),
            n("stroke-width", 1.0),
            id("fill", "none"),
        ],
        "pitch-circle" => vec![
            id("stroke-style", "center"),
            var("stroke", "stroke-light"),
            n("stroke-width", 1.0),
            id("fill", "none"),
        ],
        "breakline" => vec![
            var("stroke", "stroke-light"),
            n("stroke-width", 1.0),
            id("fill", "none"),
        ],
        // Hidden interior geometry [SPEC 15.7]: a dashed, unfilled pen profile
        // on its own child — the one-node-one-stroke-style law. Thin, like the
        // hidden-edge convention. (`|shoulder|`, the revolve’s edge line, is a
        // real visible edge and keeps the |line| base's geometry weight.)
        "hidden" => vec![
            id("stroke-style", "dashed"),
            n("stroke-width", 1.0),
            id("fill", "none"),
        ],
        // A thread's ISO 6410 thin lines [SPEC 15.3/15.4]: the minor line
        // beside a dressed run, the ¾ arc on a round view — continuous, the
        // support tone, like extension lines.
        "threadline" => vec![
            var("stroke", "stroke-light"),
            n("stroke-width", 1.0),
            id("fill", "none"),
        ],
        // A frame: a dashed, rounded rectangle around a span of messages. `padding` insets
        // the border from the messages it spans (vertical) and the lifelines (horizontal).
        "loop" | "opt" | "alt" => vec![
            id("fill", "none"),
            var("stroke", "group-stroke"),
            id("stroke-style", "dashed"),
            n("stroke-width", 1.0),
            n("radius", 4.0),
            n("padding", 24.0),
            n("font-size", 12.0),
        ],
        // An |alt| compartment separator: the same dashed line, no body radius.
        "else" => vec![
            id("fill", "none"),
            var("stroke", "group-stroke"),
            id("stroke-style", "dashed"),
            n("stroke-width", 1.0),
            n("font-size", 12.0),
        ],
        // A bar's corners are softly rounded by default [SPEC 14.2]; `stroke: auto`
        // is the outlined-look sentinel ([SPEC 10]) — the chart draws a deep edge of the soft
        // fill, while an explicit `stroke: none` (overriding `auto`) removes it. Both ride
        // the class so the user overrides them (`radius: 0` square, `stroke: none` flat).
        "bars" => vec![n("radius", 2.0), id("stroke", "auto")],
        // A pie slice shares the outlined look [SPEC 14.6]: `stroke: auto` gives
        // it a deep edge of its soft fill unless `stroke: none` opts out.
        "slice" => vec![id("stroke", "auto")],
        // A `|mark|` annotation point shows a dot by default [SPEC 14.5]; the
        // marker cascade then distinguishes that default (and `marker: dot`) from an
        // explicit `marker: none`, which resolve would otherwise collapse together.
        "mark" => vec![id("marker", "dot")],
        // A larger icon meant to stand alone as a node, with room for a short
        // label: the icon primitive at 64px with a little padding. Defaults to
        // `fit: contain` so the glyph fills that box rather than floating small
        // inside Phosphor's margin like a bare `|icon|` [SPEC 8]. A `|sign|` sits
        // among ordinary nodes and takes their `stroke-width: 2`, matching the
        // diagram's line weight (the same weight a bare `|icon|` keeps).
        "sign" => vec![
            n("width", 64.0),
            n("height", 64.0),
            n("padding", 4.0),
            n("stroke-width", 2.0),
            id("fit", "contain"),
        ],
        // A ruled grid [SPEC 8]: hairline `gap-fill` gutters fill the 1px gaps
        // between cells, the group border frames the whole. `padding: 0` on the
        // table itself — each cell's inset comes from the shipped `|table| |block|`
        // rule (desugar::classes), since body cells are now `|block|`s. Cells
        // `stretch` on both axes so every cell fills its track (backgrounds fill,
        // text has room); the user's own `align`/`justify` are distributed to the
        // cells to place their text (desugar::mod).
        "table" => vec![
            id("layout", "grid"),
            id("align", "stretch"),
            id("justify", "stretch"),
            n("gap", 1.0),
            var("gap-fill", "stroke"),
            n("padding", 0.0),
            id("fill", "none"),
            var("stroke", "stroke"),
            // A touch heavier than the group base (1) so the frame and its gutters —
            // and an |entity|, which builds on this — read crisply [SPEC 8].
            n("stroke-width", 2.0),
            id("stroke-style", "solid"),
            n("font-size", 14.0),
            id("font-weight", "normal"),
            // A table is sheet furniture: in a scaled drawing view it keeps its
            // size, like notes and balloons [SPEC 15.1].
            n("scale", 1.0),
        ],
        // A table cell [SPEC 8]: a frameless `|block|` carrying the text-to-gutter
        // inset. Body cells wrap in it; `|header|` / `|footer|` build on it. Only the
        // caption (a plain `|block|`) is left uninset. Override with `|cell| { … }`
        // or, per table, `|table| |cell| { … }`.
        "cell" => vec![pair("padding", 4.0, 8.0)],
        // A table header cell [SPEC 8]: a `|cell|` with the fill band and `bold`
        // weight. It fills its track and takes its inset / text alignment from the
        // `|cell|` + `|table|` defaults. The cascade overrides via `|table| |header| { … }`.
        "header" => vec![var("fill", "header-fill"), id("font-weight", "bold")],
        // A table footer cell [SPEC 8]: a `|cell|`, muted text, no fill.
        "footer" => vec![var("color", "footer-color")],
        // An ER / database entity [SPEC 8]: a two-column table; its label lowers to a
        // spanning header (desugar) and its body cells default to left-aligned (in
        // `distribute_cell_alignment`, so the title header stays centred and filled).
        // Everything else is the |table| base.
        "entity" => vec![decl(
            "columns",
            vec![Value::Ident("auto".into()), Value::Ident("auto".into())],
        )],
        _ => Vec::new(),
    }
}

/// Scene/root config defaults — prepended to the global block (user decls override).
pub fn root_defaults() -> Vec<Decl> {
    vec![
        id("layout", "flow"),
        n("padding", 20.0),
        n("gap", 20.0),
        n("font-size", 15.0),
    ]
}

/// The baked link base [SPEC 10.5]: a link's lowest-specificity layer, resolved
/// per link below the scope's `link*` / `clearance` / `routing` cascade, the
/// class rules, and the link's own block.
pub fn link_defaults() -> Vec<Decl> {
    vec![
        n("stroke-width", 2.0),
        n("clearance", 16.0),
        n("font-size", 11.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::NodeKind;
    use crate::syntax::ast::Value;

    fn has(decls: &[Decl], name: &str) -> bool {
        decls.iter().any(|d| d.name == name)
    }
    fn num(decls: &[Decl], name: &str) -> Option<f64> {
        decls
            .iter()
            .find(|d| d.name == name)
            .and_then(|d| match d.groups.first()?.first()? {
                Value::Number(n) => Some(*n),
                _ => None,
            })
    }
    fn ident(decls: &[Decl], name: &str) -> Option<String> {
        decls
            .iter()
            .find(|d| d.name == name)
            .and_then(|d| match d.groups.first()?.first()? {
                Value::Ident(s) => Some(s.clone()),
                _ => None,
            })
    }
    fn var(decls: &[Decl], name: &str) -> Option<String> {
        decls
            .iter()
            .find(|d| d.name == name)
            .and_then(|d| match d.groups.first()?.first()? {
                Value::Var(s) => Some(s.clone()),
                _ => None,
            })
    }

    #[test]
    fn block_is_bare_and_box_template_carries_the_paint() {
        // The bare primitive: frameless, no padding, just the container gap.
        let block = primitive_bundle(NodeKind::Block);
        assert_eq!(num(&block, "padding"), Some(0.0));
        assert_eq!(num(&block, "gap"), Some(20.0));
        assert!(!has(&block, "radius"));
        // The |box| template lifts the framed-card paint back on top.
        let boxt = template_bundle("box");
        assert_eq!(num(&boxt, "radius"), Some(8.0));
        assert_eq!(num(&boxt, "padding"), Some(20.0));
        assert_eq!(num(&boxt, "stroke-width"), Some(2.0));
        assert!(has(&boxt, "fill") && has(&boxt, "stroke"));
    }

    #[test]
    fn slant_carries_skew_icon_carries_size() {
        assert_eq!(num(&primitive_bundle(NodeKind::Slant), "skew"), Some(15.0));
        let icon = primitive_bundle(NodeKind::Icon);
        assert_eq!(num(&icon, "width"), Some(32.0));
        assert_eq!(num(&icon, "height"), Some(32.0));
    }

    #[test]
    fn group_template_is_a_dashed_frame() {
        let g = template_bundle("group");
        assert!(g.iter().any(|d| d.name == "stroke-style"));
        assert_eq!(num(&g, "stroke-width"), Some(1.0));
        assert!(template_bundle("oval").is_empty());
    }

    #[test]
    fn chart_templates_set_the_gap_and_bars_round() {
        // The chart/pie templates override the |block| base gap with the title/legend
        // gutter default; |bars| carries the default corner radius on its class.
        assert_eq!(num(&template_bundle("chart"), "gap"), Some(10.0));
        assert_eq!(num(&template_bundle("pie"), "gap"), Some(10.0));
        assert_eq!(num(&template_bundle("bars"), "radius"), Some(2.0));
    }

    #[test]
    fn flow_sugars_set_direction_and_grid_sets_layout() {
        let dir = |t: &str| match template_bundle(t).iter().find(|d| d.name == "direction") {
            Some(d) => match d.groups.first().and_then(|g| g.first()) {
                Some(Value::Ident(s)) => Some(s.clone()),
                _ => None,
            },
            None => None,
        };
        assert_eq!(dir("row").as_deref(), Some("row"));
        assert_eq!(dir("column").as_deref(), Some("column"));
        assert!(!has(&template_bundle("row"), "layout"));
        assert_eq!(num(&template_bundle("grid"), "radius"), None);
        assert!(template_bundle("grid")
            .iter()
            .any(|d| d.name == "layout"
                && matches!(d.groups.first().and_then(|g| g.first()), Some(Value::Ident(s)) if s == "grid")));
    }

    #[test]
    fn root_default_layout_is_flow() {
        assert!(root_defaults()
            .iter()
            .any(|d| d.name == "layout"
                && matches!(d.groups.first().and_then(|g| g.first()), Some(Value::Ident(s)) if s == "flow")));
    }

    #[test]
    fn root_and_link_defaults_are_present() {
        assert_eq!(num(&root_defaults(), "padding"), Some(20.0));
        assert_eq!(num(&root_defaults(), "font-size"), Some(15.0));
        assert_eq!(num(&link_defaults(), "clearance"), Some(16.0));
        assert_eq!(num(&link_defaults(), "font-size"), Some(11.0));
    }

    #[test]
    fn header_footer_entity_footnote_bundles() {
        // The header cell: a filled, bold band [SPEC 8] — it fills its track and
        // takes its alignment from the |table| cell defaults, so it carries no
        // align/justify of its own.
        let h = template_bundle("header");
        assert!(!has(&h, "justify") && !has(&h, "align"));
        assert_eq!(var(&h, "fill").as_deref(), Some("header-fill"));
        assert_eq!(ident(&h, "font-weight").as_deref(), Some("bold"));
        // The footer cell: muted text, no fill, no align of its own.
        let f = template_bundle("footer");
        assert!(!has(&f, "justify") && !has(&f, "align"));
        assert_eq!(var(&f, "color").as_deref(), Some("footer-color"));
        assert!(!has(&f, "fill"));
        // The entity: two auto columns over the |table| base (body cells are
        // left-aligned by the entity path in `distribute_cell_alignment`, not the bundle).
        let e = template_bundle("entity");
        assert!(has(&e, "columns"));
        assert!(!has(&e, "align"));
        // The footnote (the renamed old footer): still the pinned bottom caption.
        let foot = template_bundle("footnote");
        assert_eq!(ident(&foot, "pin").as_deref(), Some("bottom"));
        assert_eq!(num(&foot, "font-size"), Some(11.0));
    }
}
