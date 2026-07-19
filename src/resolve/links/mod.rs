//! Link resolution [SPEC 9]. A link resolves through the **node cascade**
//! [SPEC 13]: its type is `lini-link` (what `|-|` lowers to), its ancestors are its
//! scope chain, it has no id — so `stroke` is its wire and `color` / `font-*` its
//! labels, the ordinary vocabulary with no `link-*` family. Each statement layers
//! the baked base + scope `clearance`/`routing`, the `|-|` element rule, the
//! descendant / worn-class rules, then its own block; derives markers and line
//! style from the operator; resolves every endpoint by a scoped path-walk (with
//! did-you-mean errors); and cartesian-expands fan groups into one [`ResolvedLink`]
//! per pair.

mod projection;

use super::cascade::NodeFacts;
use super::ir::{
    Along, AttrMap, LinkKind, MarkerKind, Markers, MeasureOp, ResolvedEndpoint, ResolvedLink,
    ResolvedText, ResolvedValue, Strategy,
};
use super::merge::{collapse, resolve_markers};
use super::scene::{PathIndex, SceneCtx};
use super::value::{resolve_groups, resolve_property};
use crate::ast::{ChainOp, DrawOp, LineStyle, LinkMarker, Side};
use crate::error::Error;
use crate::ledger::properties;
use crate::span::Span;
use crate::syntax::ast::{Endpoint, EndpointGroup, Link};

/// The class every link wears [SPEC 9]: `|-|` lowers to it in desugar, so a link
/// resolves through the node cascade — its type tier, descendant/class rules, and
/// own block — with no `link-*` family.
pub const LINK_CLASS: &str = "lini-link";

/// The class every **dimension** additionally wears [SPEC 4, 15.6]: `(-)` lowers to
/// it, and its layer sits just above `LINK_CLASS`, so a `(-) { }` rule beats a
/// `|-| { }` rule for dimensions — the `|-|` → `(-)` type cascade.
pub const DIMENSION_CLASS: &str = "lini-dimension";

/// A link scope's drawing classification [SPEC 15/20]: `drawing` gates the
/// drawing statements; `flow_in_drawing` names the layout-owning container
/// when a drawing encloses the scope without being it — the mate gate's
/// "a '|row|' places its own children" refinement.
pub struct LinkScope {
    pub drawing: bool,
    pub flow_in_drawing: Option<String>,
    /// The scope is a `|detail|` view [SPEC 15.8]: its geometry is re-laid from
    /// the source at layout, so its annotation endpoints are **deferred** —
    /// kept as qualified paths and landed against the re-laid clones by the
    /// anchor walk, not resolved in the scene index here.
    pub detail: bool,
}

/// Resolve one link statement into one resolved link per cartesian pair.
/// `path_prefix` scopes a lifted internal link to its host instance;
/// `scope_ancestors` is that scope's container chain (for descendant rules);
/// `base` is the baked link defaults plus the scope's `clearance`/`routing`;
/// `ancestors_for` gives the container chain down to an arbitrary resolved
/// path — the containment-link cascade below reads the **outer endpoint's**
/// chain instead of the written scope's.
#[allow(clippy::too_many_arguments)]
pub fn resolve_link(
    w: &Link,
    ctx: &SceneCtx,
    paths: &PathIndex,
    path_prefix: &[String],
    scope_ancestors: &[NodeFacts],
    base: &[(String, ResolvedValue)],
    scope_kind: &LinkScope,
    ancestors_for: &dyn Fn(&[String]) -> Vec<NodeFacts>,
    enclosing_view: &dyn Fn(&str) -> Option<String>,
    carried: Vec<crate::resolve::ResolvedInst>,
) -> Result<Vec<ResolvedLink>, Error> {
    for class in &w.classes {
        if !ctx.sheet.defines_class(class) {
            return Err(Error::at(w.span, format!("unknown class '.{}'", class)));
        }
    }
    let drawing_scope = scope_kind.drawing;
    // A sheet-scope link whose ends dot-path into views is the one legalized
    // cross-view form [SPEC 15.8] — classified and lowered here, ahead of the
    // ordinary statement gates (a cross-view measure / mate wants its own
    // message, not the generic "belongs in a 'layout: drawing'").
    if let Some(links) = projection::try_projection(
        w,
        ctx,
        paths,
        path_prefix,
        scope_ancestors,
        scope_kind,
        enclosing_view,
    )? {
        return Ok(links);
    }
    validate_statement(w, scope_kind)?;

    // The link's kind [SPEC 9, 15]: a plain wire, a measuring dimension, or a
    // mate — a pure function of the operator (an explicit `marker:` restyles a
    // wire but never re-types it), so it is the same for every fan pair. A
    // **dimension** is any `Measure(_)`.
    let kind = match w.op() {
        ChainOp::Wire(_) => LinkKind::Wire,
        ChainOp::Measure(DrawOp::Linear) => LinkKind::Measure(MeasureOp::Linear),
        ChainOp::Measure(DrawOp::Round) => LinkKind::Measure(MeasureOp::Round),
        ChainOp::Measure(DrawOp::Angle) => LinkKind::Measure(MeasureOp::Angle),
        ChainOp::Mate => LinkKind::Mate,
    };
    let is_dim = matches!(kind, LinkKind::Measure(_));

    // A link is a node whose type is `lini-link` — plus `lini-dimension` for a
    // dimension (the `|-|` subtype) — whose ancestors are its scope chain, with no
    // id [SPEC 9, 4, 15.6].
    let link_facts = NodeFacts {
        classes: std::iter::once(LINK_CLASS.to_string())
            .chain(is_dim.then(|| DIMENSION_CLASS.to_string()))
            .chain(w.classes.iter().cloned())
            .collect(),
        id: None,
    };

    // The cascade ladder, least-specific first [SPEC 4]: the baked base + scope
    // `clearance`/`routing`, the `|-|` element rule (the type tier) then the more
    // specific `(-)` dimension rule, the descendant / worn-class rules, then the
    // link's own block. `stroke` is the wire, `font-*` / `color` the labels — the
    // same vocabulary a node uses. One ladder per ancestor chain: the written
    // scope's by default, the outer endpoint's for a containment-shaped pair.
    let resolve_ladder = |ancestors: &[NodeFacts]| -> Result<Ladder, Error> {
        let mut ordered: Vec<(String, ResolvedValue)> = base.to_vec();
        ordered.extend(ctx.sheet.class_decls(LINK_CLASS));
        if is_dim {
            ordered.extend(ctx.sheet.class_decls(DIMENSION_CLASS));
        }
        ordered.extend(ctx.sheet.node_layers(ancestors, &link_facts));
        for d in &w.style {
            ordered.push((
                d.name.clone(),
                resolve_property(&d.name, &d.groups, d.span, ctx.vars, ctx.funcs)?,
            ));
        }

        // A measure / mate has no wire: no markers to derive, no line style to inject.
        let markers = match w.op().wire() {
            Some(op) => resolve_markers(
                &ordered,
                MarkerKind::from_marker(op.start),
                MarkerKind::from_marker(op.end),
                w.span,
            )?,
            None => Markers::default(),
        };
        let mut attrs = collapse(&ordered);
        if let Some(op) = w.op().wire() {
            inject_line_style(&mut attrs, op.line);
        }
        if !drawing_scope && attrs.get("tol").is_some() {
            return Err(Error::at(
                w.span,
                "'tol' composes a dimension's text — it belongs in a 'layout: drawing'",
            ));
        }
        if !drawing_scope && attrs.get("project").is_some() {
            return Err(Error::at(
                w.span,
                "'project' picks a dimension's axis — it belongs in a 'layout: drawing'",
            ));
        }
        // The drafting dash conventions are shape / |line| values [SPEC 7]; a
        // link's set stays the core four.
        if matches!(attrs.get("stroke-style"), Some(ResolvedValue::Ident(s)) if s == "center" || s == "phantom")
        {
            return Err(Error::at(
                w.span,
                "a link's stroke-style is solid, dashed, dotted, or wavy",
            ));
        }
        let routing = parse_routing(&attrs, w.span)?;
        attrs.map.remove("routing");

        // `along:` distributes the labels along the drawn route [SPEC 9]: one
        // fraction (0..1) per label, in order; an absent fraction is `Auto` (the
        // router spreads it). It is a placement directive, not a paint attr.
        let along: Vec<f64> = match attrs.get("along") {
            Some(v) => collect_fractions(v, w.span)?,
            None => Vec::new(),
        };
        attrs.map.remove("along");

        // Labels ride `along:`, each a styleable text leaf [SPEC 9]: the link's text
        // baseline (font-size) overlaid with the label's own `{ }` (text-valid props).
        // Carried annotation nodes are not labels [SPEC 15.9] — they resolved
        // through the node path already and ride `ResolvedLink::carried`.
        let mut texts: Vec<ResolvedText> = Vec::new();
        for (i, label) in w.label_texts().enumerate() {
            let pos = along.get(i).copied().map_or(Along::Auto, Along::Fraction);
            let mut lattrs = link_text_attrs(&attrs);
            // Tier 3 [SPEC 4]: the label's worn classes, below its own block — the
            // same leaf resolution a node's text runs.
            let (_, applied_styles) = crate::resolve::scene::apply_text_classes(
                &label.classes,
                &mut lattrs,
                ctx,
                label.span,
            )?;
            for d in &label.style {
                if !properties::is_text_valid(&d.name) {
                    return Err(Error::at(
                        d.span,
                        format!("'{}' needs a box — a link label is text", d.name),
                    ));
                }
                lattrs.insert(
                    d.name.as_str(),
                    resolve_groups(&d.groups, d.span, ctx.vars, ctx.funcs)?,
                );
            }
            texts.push(ResolvedText {
                text: label.text.clone(),
                along: pos,
                attrs: lattrs,
                applied_styles,
            });
        }
        Ok(Ladder {
            attrs,
            markers,
            routing,
            texts,
        })
    };
    let scoped = resolve_ladder(scope_ancestors)?;
    // Containment ladders by outer path — a fan's siblings share the outer, so
    // each chain resolves once.
    let mut inner_ladders: Vec<(String, Ladder)> = Vec::new();

    // Cartesian fan expansion: one resolved link per endpoint sequence — except
    // a **one-ended leader's** `&` fan, which stays one link carrying every
    // endpoint: one text and landing, an independent leg per feature
    // [SPEC 15.7] (the misuse gate above already bounced measures and mates).
    let one_ended = w.chain.len() == 1;
    let chains = if one_ended {
        vec![w.chain[0].endpoints.clone()]
    } else {
        expand_chain(&w.chain)
    };
    let mut out = Vec::new();
    let mut carried = carried;
    for (fan_index, chain) in chains.into_iter().enumerate() {
        let mut endpoints = Vec::with_capacity(chain.len());
        for ep in chain {
            let qualified: Vec<String> = if path_prefix.is_empty() {
                ep.path.clone()
            } else {
                let mut p = path_prefix.to_vec();
                p.extend(ep.path.iter().cloned());
                p
            };
            let path = if scope_kind.detail {
                // Deferred: the clones exist only at layout [SPEC 15.8]; keep the
                // qualified path for the anchor walk to land.
                qualified.join(".")
            } else {
                paths
                    .resolve(&qualified)
                    .ok_or_else(|| endpoint_error(&ep, paths, path_prefix, w.op(), drawing_scope))?
            };
            // The numeric copy segment is drawing grammar [SPEC 15.4/21] —
            // like the wider point set, it exists only in a drawing scope.
            if ep.copy.is_some() && !drawing_scope {
                return Err(Error::at(
                    ep.span,
                    "a numeric path segment picks a pattern copy — it belongs in a 'layout: drawing'",
                ));
            }
            let (side, point) = resolve_point(&ep, drawing_scope)?;
            endpoints.push(ResolvedEndpoint {
                path,
                copy: ep.copy,
                side,
                point,
                span: ep.span,
            });
        }
        // A containment-shaped pair — one endpoint's resolved path a strict
        // prefix of the other's — **cascades as if written in the outer
        // endpoint X** [SPEC 9/12]: a link from a node into its own descendant
        // is that node's internal affair (ROUTING.md routes it inside the
        // parent), so `#x |-| { }` reaches it wherever the statement was
        // textually written — a tree's generated branch fans included. Only the
        // descendant-rule chain switches; the inherited config (`clearance` /
        // `routing`, in `base`) keeps the written scope's.
        let ladder = match (!one_ended)
            .then(|| containment_outer(&endpoints))
            .flatten()
        {
            Some(outer) => {
                let at = match inner_ladders.iter().position(|(p, _)| p == outer) {
                    Some(i) => i,
                    None => {
                        let segs: Vec<String> = outer.split('.').map(str::to_string).collect();
                        let ladder = resolve_ladder(&ancestors_for(&segs))?;
                        inner_ladders.push((outer.to_string(), ladder));
                        inner_ladders.len() - 1
                    }
                };
                &inner_ladders[at].1
            }
            None => &scoped,
        };
        out.push(ResolvedLink {
            endpoints,
            kind,
            scope: path_prefix.join("."),
            line: w.op().wire().map_or(LineStyle::Solid, |op| op.line),
            routing: ladder.routing,
            attrs: ladder.attrs.clone(),
            applied_styles: w.classes.clone(),
            markers: ladder.markers.clone(),
            // A fan's single written label rides one sibling, not each.
            texts: if fan_index == 0 {
                ladder.texts.clone()
            } else {
                Vec::new()
            },
            carried: std::mem::take(&mut carried),
            one_ended,
            projection: false,
            span: w.span,
        });
    }
    Ok(out)
}

/// One resolved cascade ladder's outputs — the pieces that depend on the
/// ancestor chain the descendant rules match against.
struct Ladder {
    attrs: AttrMap,
    markers: Markers,
    routing: Strategy,
    texts: Vec<ResolvedText>,
}

/// The outer endpoint of a containment-shaped pair: exactly two endpoints, one
/// resolved path a strict (dot-bounded) prefix of the other. `None` otherwise.
fn containment_outer(endpoints: &[ResolvedEndpoint]) -> Option<&str> {
    let [a, b] = endpoints else { return None };
    let strict = |outer: &str, inner: &str| {
        inner.len() > outer.len()
            && inner.starts_with(outer)
            && inner.as_bytes()[outer.len()] == b'.'
    };
    if strict(&a.path, &b.path) {
        Some(&a.path)
    } else if strict(&b.path, &a.path) {
        Some(&b.path)
    } else {
        None
    }
}

/// The statement-shape gates [SPEC 15, 20]: the drawing ops need a drawing
/// scope; a mate takes no label; and a one-ended statement is legal only for
/// the leader-shaped and measuring ops, in a drawing.
fn validate_statement(w: &Link, scope: &LinkScope) -> Result<(), Error> {
    let drawing = scope.drawing;
    if !drawing {
        match w.op() {
            ChainOp::Measure(d) => {
                return Err(Error::at(
                    w.span,
                    format!(
                        "'{}' draws a dimension — it belongs in a 'layout: drawing'",
                        d.as_str()
                    ),
                ));
            }
            ChainOp::Mate => {
                // Inside a layout-owning child of a drawing, the flow already
                // decided every position [SPEC 15.5] — name the container.
                if let Some(ty) = &scope.flow_in_drawing {
                    return Err(Error::at(
                        w.span,
                        format!("a '|{ty}|' places its own children — mates seat a drawing's"),
                    ));
                }
                return Err(Error::at(
                    w.span,
                    "a mate seats a drawing's parts — '||' belongs in a 'layout: drawing'",
                ));
            }
            ChainOp::Wire(_) => {}
        }
    }
    // `&` fans one-ended leaders (one note, a leg per feature) and the core
    // two-ended wire ops [SPEC 9, 15.7]; a measure or mate never fans — a
    // span chain is the drafting form [SPEC 20].
    if matches!(w.op(), ChainOp::Measure(_) | ChainOp::Mate)
        && w.chain.iter().any(|g| g.endpoints.len() > 1)
    {
        return Err(Error::at(
            w.span,
            "'&' fans one-ended leaders — chain dimensions instead ('a (-) b (-) c')",
        ));
    }
    // A leader's text is its **text** labels; a carried node is never a label
    // [SPEC 15.9] — but a mate rejects either kind of `[ ]` content.
    let labelled = w.label.is_some() || w.label_texts().next().is_some();
    if matches!(w.op(), ChainOp::Mate) && (labelled || !w.labels.is_empty()) {
        return Err(Error::at(w.span, "a mate takes no label"));
    }
    // `(o)` is unary-only [SPEC 15.6] — the circle pictures one round feature.
    if matches!(w.op(), ChainOp::Measure(crate::ast::DrawOp::Round)) && w.chain.len() > 1 {
        return Err(Error::at(
            w.span,
            "'(o)' measures one round feature — write 'a:top (o)' for a span",
        ));
    }
    if w.chain.len() > 1 {
        return Ok(());
    }
    // One-ended [SPEC 15.6/21]: a unary round / angle measure, or a leader toward
    // its text. The binary `(-)` (linear) needs two ends.
    match w.op() {
        ChainOp::Measure(crate::ast::DrawOp::Linear) => {
            Err(Error::at(w.span, "a linear dimension measures two anchors"))
        }
        ChainOp::Measure(_) => Ok(()),
        ChainOp::Mate => Err(Error::at(w.span, "a mate seats two parts")),
        ChainOp::Wire(op) => {
            if !drawing {
                return Err(Error::at(w.span, "link requires at least two endpoints"));
            }
            let leader_tip = matches!(
                op.start,
                LinkMarker::Arrow | LinkMarker::Dot | LinkMarker::Crow
            ) && op.end == LinkMarker::None;
            if leader_tip {
                // A bare `<-` may compose its text from a threaded segment
                // ([SPEC 15.7]) — that is layout knowledge, so the empty-text
                // gate for the arrow leader moves there; `*-` / `>-` always
                // need their word here.
                if !labelled && op.start != LinkMarker::Arrow {
                    return Err(Error::at(
                        w.span,
                        "a leader needs its text — 'bolt <- \"THRU\"'",
                    ));
                }
                return Ok(());
            }
            if op.start == LinkMarker::None && op.end != LinkMarker::None {
                return Err(Error::at(
                    w.span,
                    "a leader points back at its feature — write 'a <- \"…\"'",
                ));
            }
            // A two-marker op (`<->`, `*-*`, …) is a plain annotation arrow here
            // [SPEC 15], not a dimension — it needs two ends like any link.
            Err(Error::at(w.span, "link requires at least two endpoints"))
        }
    }
}

/// An endpoint's `:point` [SPEC 9, 15.2]: a side everywhere; corners, `center`,
/// and authored names only in a drawing scope. A reversed corner gets its
/// did-you-mean; outside a drawing the message matches the scope's vocabulary.
fn resolve_point(ep: &Endpoint, drawing: bool) -> Result<(Option<Side>, Option<String>), Error> {
    let Some(p) = &ep.point else {
        return Ok((None, None));
    };
    if let Some(side) = Side::parse(&p.name) {
        return Ok((Some(side), None));
    }
    if let Some(fix) = corner_reorder(&p.name) {
        return Err(Error::at(
            p.span,
            format!("':{}' is not an anchor — did you mean ':{}'?", p.name, fix),
        ));
    }
    if drawing {
        return Ok((None, Some(p.name.clone())));
    }
    if matches!(
        p.name.as_str(),
        "center" | "top-left" | "top-right" | "bottom-left" | "bottom-right"
    ) {
        Err(Error::at(
            p.span,
            format!(
                "':{}' is a drawing anchor — it belongs in a 'layout: drawing'",
                p.name
            ),
        ))
    } else {
        Err(Error::at(
            p.span,
            format!(
                "':{}' is not a side — use top, bottom, left, or right",
                p.name
            ),
        ))
    }
}

/// `right-top` → `top-right`: the corner glues vertical word first [SPEC 15.2].
fn corner_reorder(name: &str) -> Option<String> {
    let (a, b) = name.split_once('-')?;
    (matches!(a, "left" | "right") && matches!(b, "top" | "bottom")).then(|| format!("{b}-{a}"))
}

/// The operator's line part sets `stroke-style` unless an explicit one already
/// won the cascade [SPEC 9].
fn inject_line_style(attrs: &mut AttrMap, line: LineStyle) {
    let style = match line {
        LineStyle::Solid => return,
        LineStyle::Dashed => "dashed",
        LineStyle::Dotted => "dotted",
        LineStyle::Wavy => "wavy",
    };
    if attrs.get("stroke-style").is_none() {
        attrs.insert("stroke-style", ResolvedValue::Ident(style.into()));
    }
}

/// The resolved wiring strategy [SPEC 9]: `orthogonal` (the default),
/// `natural`, and `straight`; `curved` was replaced by `natural` [SPEC 20].
fn parse_routing(attrs: &AttrMap, span: crate::span::Span) -> Result<Strategy, Error> {
    match attrs.get("routing") {
        None => Ok(Strategy::Orthogonal),
        Some(ResolvedValue::Ident(r)) if r == "orthogonal" => Ok(Strategy::Orthogonal),
        Some(ResolvedValue::Ident(r)) if r == "natural" => Ok(Strategy::Natural),
        Some(ResolvedValue::Ident(r)) if r == "straight" => Ok(Strategy::Straight),
        Some(_) => Err(Error::at(
            span,
            "routing takes orthogonal, natural, or straight — 'curved' was replaced by 'natural'",
        )),
    }
}

/// The `along:` value as a list of route fractions — comma-separated [SPEC 2/9].
fn collect_fractions(v: &ResolvedValue, span: Span) -> Result<Vec<f64>, Error> {
    let items = match v {
        ResolvedValue::List(xs) => xs.as_slice(),
        one => std::slice::from_ref(one),
    };
    items
        .iter()
        .map(|x| {
            x.as_number().ok_or_else(|| {
                Error::at(
                    span,
                    "'along' takes comma-separated fractions — 'along: 0.2, 0.5, 0.8'",
                )
            })
        })
        .collect()
}

/// A link's labels inherit its text context [SPEC 9]: every inheritable text prop
/// the link resolved — `font-*`, `color`, the spacings — seeds each label, which
/// its own `{ }` then overrides. This is how a `|-| { font-size: 14; color: red }`
/// restyles every label at once, exactly as a node's text inherits the node's.
fn link_text_attrs(link_attrs: &AttrMap) -> AttrMap {
    let mut map = AttrMap::new();
    for name in properties::inherited_text() {
        if let Some(v) = link_attrs.get(name) {
            map.insert(name, v.clone());
        }
    }
    map
}

/// Flatten a chain's endpoint groups into every cartesian sequence — one per
/// resolved link (`a & b -> c` → `a→c`, `b→c`).
fn expand_chain(chain: &[EndpointGroup]) -> Vec<Vec<Endpoint>> {
    let mut acc: Vec<Vec<Endpoint>> = vec![Vec::new()];
    for group in chain {
        let mut next = Vec::with_capacity(acc.len() * group.endpoints.len());
        for trail in &acc {
            for ep in &group.endpoints {
                let mut t = trail.clone();
                t.push(ep.clone());
                next.push(t);
            }
        }
        acc = next;
    }
    acc
}

fn endpoint_error(
    ep: &Endpoint,
    paths: &PathIndex,
    scope: &[String],
    op: ChainOp,
    drawing: bool,
) -> Error {
    let where_ = if scope.is_empty() {
        "at scene root".to_string()
    } else {
        format!("in '{}'", scope.join("."))
    };
    // A drawing statement's endpoint is never auto-created [SPEC 15], so the
    // noun names what actually failed there — a `<->` in a drawing *is* a
    // dimension; elsewhere every statement is a link.
    let noun = match (op, drawing) {
        (_, false) => "link",
        (ChainOp::Mate, true) => "mate",
        (_, true) => "dimension",
    };
    // A copy index rides the spelling (`bolt.2`) — copies leak no ids, so the
    // carrierless form is exactly this unknown-endpoint error [SPEC 15.4].
    let mut spelled = ep.path.join(".");
    if let Some(k) = ep.copy {
        spelled.push('.');
        spelled.push_str(&k.to_string());
    }
    let mut msg = format!("{noun} endpoint '{spelled}' not found {where_}");
    let suggestions = paths.suggest(ep.path.last().expect("non-empty path"), scope);
    msg.push_str(&crate::suggest::did_you_mean(&suggestions));
    Error::at(ep.span, msg)
}
