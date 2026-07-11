//! Frame the finished layout into a [`LaidOut`]: union every node's true drawn
//! extent — rotations and overlays included — pad it into the viewbox, and carry
//! a pages-only scene's physical millimetres [SPEC 15.8].

use super::*;

/// Union every node's drawn extent into `bbox`, in world coords — so the
/// canvas includes absolute overlays that don't grow their parent's bbox.
/// `rot` is the accumulated ancestor rotation: each node renders as
/// `translate(cx, cy) rotate(deg)`, so a turned node's true extent is its
/// bbox corners swung about its origin — without this, a `rotate:`d part
/// (a mated bar stood on end, [SPEC 15.5]) clips at the canvas edge.
pub(super) fn accumulate_extent(n: &PlacedNode, ox: f64, oy: f64, rot: f64, bbox: &mut Bbox) {
    let turn = |x: f64, y: f64, deg: f64| -> (f64, f64) {
        if deg == 0.0 {
            return (x, y);
        }
        let (s, c) = deg.to_radians().sin_cos();
        (x * c - y * s, x * s + y * c)
    };
    let (dx, dy) = turn(n.cx, n.cy, rot);
    let (wx, wy) = (ox + dx, oy + dy);
    let total = rot + n.rotation;
    let b = &n.bbox;
    for (x, y) in [
        (b.min_x, b.min_y),
        (b.max_x, b.min_y),
        (b.min_x, b.max_y),
        (b.max_x, b.max_y),
    ] {
        let (px, py) = turn(x, y, total);
        *bbox = bbox.union(Bbox {
            min_x: wx + px,
            min_y: wy + py,
            max_x: wx + px,
            max_y: wy + py,
        });
    }
    // A clipped node bounds its children [SPEC 15.8] — a `|detail|`'s re-laid
    // clones extend past the region circle but are cropped to it, so the
    // node's own bbox (the circle) is the extent; don't descend into the crop.
    if n.attrs.get("clip").is_some() {
        return;
    }
    for c in &n.children {
        accumulate_extent(c, wx, wy, total, bbox);
    }
}

pub(super) fn finish(
    program: &Program,
    nodes: Vec<PlacedNode>,
    scene_bbox: Bbox,
    routing: routing::Routing,
) -> Result<LaidOut, Error> {
    // Viewbox = the whole drawn extent (scene bbox + link paths, labels, strays,
    // overlays) framed by the scene's `padding` on every side — the margin between
    // the diagram and the SVG edge.
    let pad = primitives::padding(&program.scene.attrs, Span::empty())?;
    // Absolute overlays don't grow their parent's bbox, so the scene bbox can
    // miss one that overflows; the canvas must still include every drawn node,
    // so take the true visual extent of the whole tree.
    let mut bbox = scene_bbox;
    for n in &nodes {
        accumulate_extent(n, 0.0, 0.0, 0.0, &mut bbox);
    }
    let link_points = routing.links.iter().flat_map(|w| &w.path);
    let air_points = routing.strays.iter().flat_map(|a| [&a.from, &a.to]);
    for &(x, y) in link_points.chain(air_points) {
        bbox.min_x = bbox.min_x.min(x);
        bbox.min_y = bbox.min_y.min(y);
        bbox.max_x = bbox.max_x.max(x);
        bbox.max_y = bbox.max_y.max(y);
    }
    for t in routing.links.iter().flat_map(|w| &w.texts) {
        let size = t.attrs.number("font-size").unwrap_or(0.0);
        let ls = t.attrs.number("letter-spacing").unwrap_or(0.0);
        let lsp = t.attrs.number("line-spacing").unwrap_or(0.0);
        let (hw, hh) = (
            text::approx_width(&t.content, size, ls) / 2.0,
            text::approx_height(&t.content, size, lsp) / 2.0,
        );
        bbox.min_x = bbox.min_x.min(t.position.0 - hw);
        bbox.min_y = bbox.min_y.min(t.position.1 - hh);
        bbox.max_x = bbox.max_x.max(t.position.0 + hw);
        bbox.max_y = bbox.max_y.max(t.position.1 + hh);
    }
    let vb = ViewBox {
        x: bbox.min_x - pad.left,
        y: bbox.min_y - pad.top,
        w: bbox.w() + pad.left + pad.right,
        h: bbox.h() + pad.top + pad.bottom,
    };

    // A root `fill:` overrides the canvas colour inline [SPEC 17]; the default
    // comes from the `.lini-canvas` rule (`--lini-bg`). `none` → transparent.
    let canvas_fill = program.scene.attrs.get("fill").cloned();

    // A pages-only scene prints true-scale [SPEC 15.8]: its `viewBox` is px, but
    // the SVG's `width` / `height` carry the paper's real millimetres — the
    // viewBox extent over the page's px-per-mm `scale:`. Same predicate as the
    // hug-the-canvas padding default, so a lone sheet fills the SVG exactly.
    let physical = pages_only(&nodes).map(|scale| (vb.w / scale, vb.h / scale));

    Ok(LaidOut {
        viewbox: vb,
        nodes,
        links: routing.links,
        link_report: routing.report,
        strays: routing.strays,
        vars: program.vars.clone(),
        sheet: program.sheet.clone(),
        canvas_fill,
        gradients: Vec::new(),
        hatches: Vec::new(),
        clips: Vec::new(),
        physical,
    })
}

/// The px-per-mm `scale:` of a **pages-only** scene [SPEC 15.8] — every drawn
/// top-level node a `|page|` — else `None`. The predicate the physical-size
/// emission and the hug-the-canvas padding default share.
fn pages_only(nodes: &[PlacedNode]) -> Option<f64> {
    if nodes.is_empty() || !nodes.iter().all(|n| page::is_page(&n.type_chain)) {
        return None;
    }
    Some(nodes[0].attrs.number("scale").unwrap_or(4.0))
}
