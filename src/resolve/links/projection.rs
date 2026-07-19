//! The projection construction link [SPEC 15.8/20] — the one legalized
//! cross-view anchor form, split out of the link resolver: classification
//! (`try_projection`) and the `|projection|` node cascade its line resolves
//! through (`projection_attrs`).

use super::*;
use crate::error::Code;

/// Classify a **projection construction link** [SPEC 15.8] — the one legalized
/// cross-view anchor form. Returns `None` for any ordinary link (drawing-scope,
/// or a sheet link whose ends stay outside every view), which the caller then
/// resolves the normal way; `Some` for a projection line built here, or an error
/// for a misused cross-view statement ([SPEC 20]). A link **touches a view** when
/// an endpoint dot-paths *inside* a drawing (a strict descendant) — the sealed
/// body's one crack; anchoring a view's own bbox side (`side:top`) does not.
pub(super) fn try_projection(
    w: &Link,
    ctx: &SceneCtx,
    paths: &PathIndex,
    path_prefix: &[String],
    scope_ancestors: &[NodeFacts],
    scope_kind: &LinkScope,
    enclosing_view: &dyn Fn(&str) -> Option<String>,
) -> Result<Option<Vec<ResolvedLink>>, Error> {
    // Projection links live in the sheet's scope — outside *every* drawing
    // [SPEC 15.8]. A drawing scope, a flow container nested in one (its links
    // are the drawing's, weight 2), or a detail scope (endpoints deferred to
    // the anchor walk) is never one.
    if scope_kind.drawing || scope_kind.detail || scope_kind.flow_in_drawing.is_some() {
        return Ok(None);
    }
    // Resolve every endpoint's path and its enclosing view. An unresolvable path
    // is not ours — fall through so the normal flow raises the not-found error.
    let eps: Vec<&Endpoint> = w.chain.iter().flat_map(|g| &g.endpoints).collect();
    let mut resolved: Vec<(&Endpoint, String, Option<String>)> = Vec::with_capacity(eps.len());
    for ep in &eps {
        let qualified: Vec<String> = if path_prefix.is_empty() {
            ep.path.clone()
        } else {
            path_prefix.iter().chain(&ep.path).cloned().collect()
        };
        let Some(path) = paths.resolve(&qualified) else {
            return Ok(None);
        };
        let view = enclosing_view(&path);
        resolved.push((ep, path, view));
    }
    if resolved.iter().all(|(_, _, v)| v.is_none()) {
        // No endpoint reaches inside a view — an ordinary sheet link.
        return Ok(None);
    }

    // A view is touched: this is a cross-view statement, legal only as the
    // unmarked `-` projection line [SPEC 15.8, 20].
    match w.op() {
        ChainOp::Measure(_) | ChainOp::Mate => {
            return Err(Error::at(
                w.span,
                "a dimension reads one view — a cross-view correspondence is a construction link ('a - b')",
            ));
        }
        ChainOp::Wire(op) => {
            let unmarked = op.line == LineStyle::Solid
                && op.start == LinkMarker::None
                && op.end == LinkMarker::None;
            if !unmarked {
                return Err(Error::at(
                    w.span,
                    "a projection line is unmarked — write 'side.screw:head - end.od:top'",
                )
                .code(Code::PROJECTION));
            }
        }
    }

    // The line ties exactly two anchors, in two different views.
    let off_view = |ep: &Endpoint| {
        Error::at(
            w.span,
            format!(
                "a projection link ties drawing anchors — '{}' is not in a drawing view",
                ep.path.join(".")
            ),
        )
        .code(Code::PROJECTION)
    };
    let [(ea, pa, va), (eb, pb, vb)] = resolved.as_slice() else {
        // A chain of view anchors is still not a dimension: name the first end
        // that misses a view, else that they share one.
        if let Some((ep, _, _)) = resolved.iter().find(|(_, _, v)| v.is_none()) {
            return Err(off_view(ep));
        }
        let first = resolved[0].2.as_deref().unwrap_or_default();
        return Err(Error::at(
            w.span,
            format!("a projection link ties two views — both ends read '{first}'"),
        )
        .code(Code::PROJECTION));
    };
    match (va, vb) {
        (Some(a), Some(b)) if a == b => {
            return Err(Error::at(
                w.span,
                format!("a projection link ties two views — both ends read '{a}'"),
            )
            .code(Code::PROJECTION));
        }
        (None, _) => return Err(off_view(ea)),
        (_, None) => return Err(off_view(eb)),
        _ => {}
    }

    // Both ends carry the full drawing anchor vocabulary — the sealed-body
    // exception [SPEC 15.2]. Resolve each spot as in a drawing.
    let endpoint = |ep: &Endpoint, path: &str| -> Result<ResolvedEndpoint, Error> {
        let (side, point) = resolve_point(ep, true)?;
        Ok(ResolvedEndpoint {
            path: path.to_string(),
            copy: ep.copy,
            side,
            point,
            span: ep.span,
        })
    };
    let endpoints = vec![endpoint(ea, pa)?, endpoint(eb, pb)?];
    let attrs = projection_attrs(w, ctx, scope_ancestors)?;
    Ok(Some(vec![ResolvedLink {
        endpoints,
        kind: LinkKind::Wire,
        scope: path_prefix.join("."),
        line: LineStyle::Solid,
        routing: Strategy::Straight,
        attrs,
        applied_styles: w.classes.clone(),
        markers: Markers::default(),
        texts: Vec::new(),
        carried: Vec::new(),
        one_ended: false,
        projection: true,
        span: w.span,
    }]))
}

/// The `|projection|` node cascade [SPEC 8/15.8]: the `|line|` base then the
/// projection template default, the user `|projection| { }` rules (scope-wide
/// restyle / removal), the scope's descendant / id layers, and the link's own
/// `{ }` block — the same tiers a `|projection|` node resolves through, so the
/// generated line and an authored one look identical and one CSS rule dresses
/// both. Bundles seed the tiers when no instance made them present at desugar.
fn projection_attrs(
    w: &Link,
    ctx: &SceneCtx,
    scope_ancestors: &[NodeFacts],
) -> Result<AttrMap, Error> {
    use crate::ledger::defaults::{primitive_bundle, template_bundle};
    use crate::resolve::NodeKind;
    use crate::resolve::value::resolve_bundle;
    let facts = NodeFacts {
        classes: ["lini-projection", "lini-line"]
            .into_iter()
            .map(str::to_string)
            .chain(w.classes.iter().cloned())
            .collect(),
        id: None,
    };
    let mut ordered: Vec<(String, ResolvedValue)> = Vec::new();
    // Tier 1, worn base→derived: the `|line|` primitive then `|projection|`.
    let line = ctx.sheet.class_decls("lini-line");
    if line.is_empty() {
        ordered.extend(resolve_bundle(
            &primitive_bundle(NodeKind::Line),
            ctx.vars,
            ctx.funcs,
        )?);
    } else {
        ordered.extend(line);
    }
    let proj = ctx.sheet.class_decls("lini-projection");
    if proj.is_empty() {
        ordered.extend(resolve_bundle(
            &template_bundle("projection"),
            ctx.vars,
            ctx.funcs,
        )?);
    } else {
        ordered.extend(proj);
    }
    // Tiers 2–4: descendant / user-class / id rules matching the line.
    ordered.extend(ctx.sheet.node_layers(scope_ancestors, &facts));
    // Tier 5: the link's own `{ }` block.
    for d in &w.style {
        ordered.push((
            d.name.clone(),
            resolve_property(&d.name, &d.groups, d.span, ctx.vars, ctx.funcs)?,
        ));
    }
    Ok(collapse(&ordered))
}
