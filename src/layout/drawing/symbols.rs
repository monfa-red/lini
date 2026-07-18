//! Drafting symbols [SPEC 15.9] — the annotation node types drawn from the
//! glyph registry (`crate::glyph`), sheet content in every sense: sized in
//! natural units off the annotation `font-size` and the statement's
//! `stroke-width`, never the view scale. `|surface-finish|` lowers here; the
//! frame types (`|feature-control|` / `|control|` / `|datum|`) land with
//! GDT-alpha4 Stage 2.

use super::super::ir::PlacedNode;
use super::super::{approx_height, approx_width, prim};
use crate::error::Error;
use crate::glyph::{FINISH_APEX_X, FINISH_TIP_X, GRID};
use crate::ledger::consts::DRAWING_LINK_FONT_SIZE;
use crate::resolve::{NodeKind, ResolvedInst, ResolvedValue};

/// The drafting-symbol type a node wears, if any — the [SPEC 20]
/// drawing-scope gate and the lowering dispatch key on one list.
pub(in crate::layout) fn drafting_type(chain: &[String]) -> Option<&'static str> {
    chain.iter().find_map(|t| match t.as_str() {
        "surface-finish" => Some("surface-finish"),
        _ => None,
    })
}

/// The finish vee's height as a multiple of the annotation font [SPEC 15.9]:
/// ISO 1302 draws the long leg at 3× the lettering height (h₂ = 3h).
const FINISH_HEIGHT_EM: f64 = 3.0;

/// Lower a drafting-symbol node — today always `|surface-finish|`. The vee's
/// **tip is the node's local origin** (the datum the node places by, and
/// Stage 3's seat anchor); the indication rides the long leg's apex.
pub(in crate::layout) fn layout_node(inst: &ResolvedInst) -> Result<PlacedNode, Error> {
    let fs = inst
        .attrs
        .number("font-size")
        .unwrap_or(DRAWING_LINK_FONT_SIZE);
    let sw = inst.attrs.number("stroke-width").unwrap_or(1.0);
    let stroke = inst
        .attrs
        .get("stroke")
        .cloned()
        .unwrap_or(ResolvedValue::LiveVar {
            name: "stroke-dark".into(),
            raw: false,
        });
    let variant = match inst.attrs.get("symbol") {
        Some(ResolvedValue::Ident(s)) => s.as_str(),
        _ => "basic",
    };
    let name = format!("finish-{variant}");
    let g = crate::glyph::lookup(&name).expect("a validated finish variant");

    // Natural units [SPEC 15.9]: height off the font, uniform scale off the
    // registry grid; the stroke width passes through untouched.
    let h = FINISH_HEIGHT_EM * fs;
    let s = h / GRID;
    let w = g.width * s;
    // The glyph child centres so its grid tip lands on the local origin.
    let glyph = prim::glyph(
        &name,
        (g.width / 2.0 - FINISH_TIP_X) * s,
        -GRID / 2.0 * s,
        w,
        h,
        stroke,
        sw,
    );
    let mut bbox = glyph.bbox.shifted(glyph.cx, glyph.cy);
    let mut children = vec![glyph];

    // The indication — the smart label, one line per authored text — rides
    // the long leg: stacked above the apex, reading rightward [SPEC 15.9].
    let apex = ((FINISH_APEX_X - FINISH_TIP_X) * s, -h);
    let lines: Vec<&ResolvedInst> = inst
        .children
        .iter()
        .filter(|c| c.kind == NodeKind::Text && c.label.is_some())
        .collect();
    let mut bottom = apex.1;
    for t in lines.iter().rev() {
        let content = t.label.as_deref().unwrap_or_default();
        let size = t.attrs.number("font-size").unwrap_or(fs);
        let tw = approx_width(content, t.font, size, 0.0);
        let th = approx_height(content, size, 0.0);
        let n = prim::dim_text(
            content,
            apex.0 + 3.0 + tw / 2.0,
            bottom - th / 2.0,
            size,
            t.font.kind,
        );
        bottom -= th + 2.0;
        bbox = bbox.union(n.bbox.shifted(n.cx, n.cy));
        children.push(n);
    }
    // Texts back into source order (they were laid bottom-up).
    children[1..].reverse();

    // The shell is invisible anatomy [SPEC 15.9] — a `Path` with no path
    // draws nothing, while the node keeps its identity, classes, and attrs
    // (the leader form raycasts its bbox; `translate:` places it).
    let mut shell = prim::container(inst, bbox, children);
    shell.kind = NodeKind::Path;
    Ok(shell)
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{by_id, laid, layout_err, texts};
    use crate::resolve::{NodeKind, ResolvedValue};

    fn symbol_of(n: &crate::layout::PlacedNode) -> Option<&str> {
        match n.attrs.get("symbol") {
            Some(ResolvedValue::Ident(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    fn compile_err(src: &str) -> String {
        let toks = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(src, &toks).expect("parse");
        match crate::desugar::desugar(&file)
            .and_then(|low| crate::resolve::resolve_with_theme(&low, &[]).map(|_| ()))
        {
            Ok(()) => panic!("expected a resolve error"),
            Err(e) => e.message,
        }
    }

    const PART: &str = "{ layout: drawing; density: 1 }\n|rect#a| { width: 80; height: 40 }\n";

    fn glyph_of(n: &crate::layout::PlacedNode) -> &crate::layout::PlacedNode {
        n.children
            .iter()
            .find(|c| c.type_chain.iter().any(|t| t == "drafting-glyph"))
            .expect("the vee glyph child")
    }

    #[test]
    fn each_variant_lowers_its_vee_at_natural_units() {
        for (variant, name) in [
            ("basic", "finish-basic"),
            ("machined", "finish-machined"),
            ("prohibited", "finish-prohibited"),
        ] {
            let src = format!(
                "{PART}|surface-finish#sf| \"Ra 1.6\" {{ symbol: {variant}; translate: 0 -60 }}\n"
            );
            let out = laid(&src);
            let sf = by_id(&out.nodes, "sf");
            let g = glyph_of(sf);
            assert_eq!(g.kind, NodeKind::Icon);
            assert_eq!(symbol_of(g), Some(name));
            // Natural units [SPEC 15.9]: height = 3 × the 12 px annotation
            // font; the stroke width stays the statement's 1, unscaled.
            assert!((g.bbox.h() - 36.0).abs() < 1e-9);
            assert_eq!(g.attrs.number("stroke-width"), Some(1.0));
        }
    }

    #[test]
    fn the_default_variant_is_basic_and_the_indication_rides_the_apex() {
        let out = laid(&format!(
            "{PART}|surface-finish#sf| \"Ra 1.6\" {{ translate: 0 -60 }}\n"
        ));
        let sf = by_id(&out.nodes, "sf");
        assert_eq!(symbol_of(glyph_of(sf)), Some("finish-basic"));
        let (tx, ty, _) = super::super::testutil::text_at(&out.nodes, "Ra 1.6");
        // The vee tip is the node origin; the label reads up-right of it.
        let tip = (sf.cx, sf.cy);
        assert!(tx > tip.0 && ty < tip.1);
    }

    #[test]
    fn a_view_scale_never_touches_the_symbol() {
        // Sheet content [SPEC 15.1/15.9]: at `scale: 2` the geometry doubles,
        // the vee and its stroke hold at the same sheet size as at 1:1.
        let at = |scale: &str| {
            let out = laid(&format!(
                "{{ layout: drawing; density: 1 }}\n|rect#a| {{ width: 80; height: 40; {scale} }}\n|surface-finish#sf| \"Ra 1.6\" {{ translate: 0 -60 }}\n"
            ));
            let sf = by_id(&out.nodes, "sf");
            let g = glyph_of(sf);
            (g.bbox.h(), g.attrs.number("stroke-width").unwrap())
        };
        assert_eq!(at(""), at("scale: 2"));
    }

    #[test]
    fn the_leader_form_wires_the_one_placed_node() {
        let out = laid(&format!(
            "{PART}|surface-finish#sf| \"Ra 3.2\" {{ symbol: machined; translate: 70 -70 }}\na:top <- sf\n"
        ));
        // One node, one glyph — the leader attaches, never re-renders it.
        fn count(nodes: &[crate::layout::PlacedNode]) -> usize {
            nodes
                .iter()
                .map(|n| {
                    usize::from(n.type_chain.iter().any(|t| t == "drafting-glyph"))
                        + count(&n.children)
                })
                .sum()
        }
        assert_eq!(count(&out.nodes), 1);
        assert_eq!(
            texts(&out.nodes)
                .iter()
                .filter(|(t, ..)| t == "Ra 3.2")
                .count(),
            1
        );
    }

    #[test]
    fn a_dim_row_packs_past_a_placed_finish_symbol() {
        // The symbol sits where the bottom row would seat [SPEC 15.6/15.9]:
        // the row must stand clear below it, never overlap.
        let out = laid(&format!(
            "{PART}|surface-finish#sf| \"Ra 1.6\" {{ translate: 0 32 }}\na:left (-) a:right {{ side: bottom }}\n"
        ));
        let sf = by_id(&out.nodes, "sf");
        let sf_box = sf.bbox.shifted(sf.cx, sf.cy);
        let (_, vy, _) = super::super::testutil::text_at(&out.nodes, "80");
        assert!(
            vy > sf_box.max_y,
            "dim value at y {vy} inside the symbol (bottom {})",
            sf_box.max_y
        );
    }

    #[test]
    fn outside_a_drawing_the_type_errors() {
        assert_eq!(
            layout_err("|surface-finish| \"Ra 1.6\"\n|box#a|\n"),
            "'|surface-finish|' annotates a drawing — it belongs in a 'layout: drawing'"
        );
    }

    #[test]
    fn an_unknown_variant_errors_at_the_node() {
        assert_eq!(
            compile_err(&format!(
                "{PART}|surface-finish#sf| \"Ra 1.6\" {{ symbol: polished }}\n"
            )),
            "'symbol' picks the vee — basic, machined, or prohibited"
        );
    }
}
