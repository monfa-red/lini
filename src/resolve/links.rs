//! Link resolution (SPEC §9). Each link statement cascades `link { }` defaults →
//! class rules → its own block, derives markers and line style from the
//! operator, resolves every endpoint by a scoped path-walk (with did-you-mean
//! errors), and cartesian-expands fan groups into one [`ResolvedLink`] per pair.

use super::cascade::NodeFacts;
use super::ir::{
    Along, AttrMap, MarkerKind, ResolvedEndpoint, ResolvedLink, ResolvedText, ResolvedValue,
};
use super::merge::{collapse, resolve_markers};
use super::scene::{PathIndex, SceneCtx};
use super::value::{resolve_groups, resolve_property};
use crate::ast::LineStyle;
use crate::error::Error;
use crate::syntax::ast::{Endpoint, EndpointGroup, Link};

/// Resolve one link statement into one resolved link per cartesian pair.
/// `path_prefix` scopes a lifted internal link to its host instance;
/// `link_defaults` is the `link { }` element rule (lowest specificity).
pub fn resolve_link(
    w: &Link,
    ctx: &SceneCtx,
    paths: &PathIndex,
    path_prefix: &[String],
    link_defaults: &[(String, ResolvedValue)],
) -> Result<Vec<ResolvedLink>, Error> {
    for class in &w.classes {
        if !ctx.sheet.defines_class(class) {
            return Err(Error::at(w.span, format!("unknown class '.{}'", class)));
        }
    }

    // Cascade: link defaults → class rules → own block (SPEC §4). A link has no
    // id, so id-tier rules never target it.
    let link_facts = NodeFacts {
        classes: w.classes.clone(),
        id: None,
    };
    // A link is painted by the `link` family, never `stroke*` (SPEC §9) — it is a
    // link, not a stroked shape — so a `stroke*` property a user puts on it (its
    // own block or a worn class) is an error, pointing at the `link*` equivalent.
    // The baked defaults carry `stroke-width` as the *internal* link-width, so
    // they are exempt — only what the user wrote is checked.
    let class_layers = ctx.sheet.node_layers(&[], &link_facts);
    reject_stroke_props(&w.style, &class_layers, w.span)?;

    let mut ordered: Vec<(String, ResolvedValue)> = link_defaults.to_vec();
    ordered.extend(class_layers);
    for d in &w.style {
        ordered.push((
            d.name.clone(),
            resolve_property(&d.name, &d.groups, d.span, ctx.vars, ctx.funcs)?,
        ));
    }
    // A link's paint family is `link` / `link-width` / `link-style` (SPEC §9);
    // map them onto the path's `stroke*` so the cascade and renderer are uniform.
    let ordered = map_link_props(ordered);

    let markers = resolve_markers(
        &ordered,
        MarkerKind::from_marker(w.op.start),
        MarkerKind::from_marker(w.op.end),
        w.span,
    )?;
    let mut attrs = collapse(&ordered);
    inject_line_style(&mut attrs, w.op.line);
    validate_routing(&attrs, w.span)?;
    attrs.map.remove("routing");

    // `along:` distributes the labels along the drawn route (SPEC §9): one
    // fraction (0..1) per label, in order; an absent fraction is `Auto` (the
    // router spreads it). It is a placement directive, not a paint attr.
    let along: Vec<f64> = attrs
        .get("along")
        .map(collect_fractions)
        .unwrap_or_default();
    attrs.map.remove("along");

    // Labels ride `along:`, each a styleable text leaf (SPEC §9): the link's text
    // baseline (font-size) overlaid with the label's own `{ }` (text-valid props).
    let mut texts: Vec<ResolvedText> = Vec::new();
    for (i, label) in w.labels.iter().enumerate() {
        let pos = along.get(i).copied().map_or(Along::Auto, Along::Fraction);
        let mut lattrs = link_text_attrs(AttrMap::new(), &attrs);
        for d in &label.style {
            if !super::scene::is_text_prop(&d.name) {
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
        });
    }

    // Cartesian fan expansion: one resolved link per endpoint sequence.
    let mut out = Vec::new();
    for (fan_index, chain) in expand_chain(&w.chain).into_iter().enumerate() {
        let mut endpoints = Vec::with_capacity(chain.len());
        for ep in chain {
            let qualified: Vec<String> = if path_prefix.is_empty() {
                ep.path.clone()
            } else {
                let mut p = path_prefix.to_vec();
                p.extend(ep.path.iter().cloned());
                p
            };
            let path = paths
                .resolve(&qualified)
                .ok_or_else(|| endpoint_error(&ep, paths, path_prefix))?;
            endpoints.push(ResolvedEndpoint {
                path,
                side: ep.side,
                span: ep.span,
            });
        }
        out.push(ResolvedLink {
            endpoints,
            scope: path_prefix.join("."),
            attrs: attrs.clone(),
            applied_styles: w.classes.clone(),
            markers: markers.clone(),
            // A fan's single written label rides one sibling, not each.
            texts: if fan_index == 0 {
                texts.clone()
            } else {
                Vec::new()
            },
            span: w.span,
        });
    }
    Ok(out)
}

/// The operator's line part sets `stroke-style` unless an explicit one already
/// won the cascade (SPEC §9).
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

/// The shape-outline paint that a link rejects (SPEC §9) — it owns the parallel
/// `link*` family instead, so the two never both apply to a line.
const STROKE_PROPS: [&str; 3] = ["stroke", "stroke-width", "stroke-style"];

/// The `link*` property that replaces a `stroke*` one on a link.
fn link_equiv(name: &str) -> &str {
    match name {
        "stroke" => "link",
        "stroke-width" => "link-width",
        "stroke-style" => "link-style",
        other => other,
    }
}

fn stroke_on_link(name: &str) -> String {
    format!(
        "'{}' paints a shape's outline, not a link — a link uses the 'link' family, so write '{}' (SPEC §9)",
        name,
        link_equiv(name)
    )
}

/// Reject a `stroke*` property on a link (SPEC §9): the link's own block reports
/// at the offending declaration; one a **worn class** contributes reports at the
/// link statement (the class is fine on a box — just not worn by a link).
fn reject_stroke_props(
    own: &[crate::syntax::ast::Decl],
    class_layers: &[(String, ResolvedValue)],
    link_span: crate::span::Span,
) -> Result<(), Error> {
    for d in own {
        if STROKE_PROPS.contains(&d.name.as_str()) {
            return Err(Error::at(d.span, stroke_on_link(&d.name)));
        }
    }
    if let Some((name, _)) = class_layers
        .iter()
        .find(|(k, _)| STROKE_PROPS.contains(&k.as_str()))
    {
        return Err(Error::at(link_span, stroke_on_link(name)));
    }
    Ok(())
}

/// Map a link's surface paint family — `link` / `link-width` / `link-style` /
/// `link-font-size` (SPEC §9) — onto the SVG path's `stroke*` / `font-size`, so
/// the cascade, the renderer, and the `.lini-link` rule all speak one vocabulary.
/// Every other property (clearance, along, marker*, …) passes through unchanged.
pub(super) fn map_link_props(
    ordered: Vec<(String, ResolvedValue)>,
) -> Vec<(String, ResolvedValue)> {
    ordered
        .into_iter()
        .map(|(k, v)| (map_link_name(&k).to_string(), v))
        .collect()
}

fn map_link_name(name: &str) -> &str {
    match name {
        "link" => "stroke",
        "link-width" => "stroke-width",
        "link-style" => "stroke-style",
        "link-font-size" => "font-size",
        other => other,
    }
}

/// Only `routing: orthogonal` is built; `straight` / `curved` are named but
/// deferred (SPEC §19).
fn validate_routing(attrs: &AttrMap, span: crate::span::Span) -> Result<(), Error> {
    match attrs.get("routing") {
        None => Ok(()),
        Some(ResolvedValue::Ident(r)) if r == "orthogonal" => Ok(()),
        Some(_) => Err(Error::at(
            span,
            "routing: only 'orthogonal' is built; 'straight' / 'curved' are deferred (SPEC §19)",
        )),
    }
}

/// The `along:` value as a list of route fractions — one number, or a group.
fn collect_fractions(v: &ResolvedValue) -> Vec<f64> {
    match v {
        ResolvedValue::Number(n) => vec![*n],
        ResolvedValue::Tuple(xs) | ResolvedValue::List(xs) => {
            xs.iter().filter_map(ResolvedValue::as_number).collect()
        }
        _ => Vec::new(),
    }
}

/// Default a link label's `font-size` to the baked `--link-font-size` (12) when
/// unset, so labels read a touch smaller than body text.
/// A link label inherits the link's `font-size` (the baked `11`
/// default, or an override) so its measured size and rendered size agree.
fn link_text_attrs(mut map: AttrMap, link_attrs: &AttrMap) -> AttrMap {
    if map.get("font-size").is_none()
        && let Some(fs) = link_attrs.get("font-size")
    {
        map.insert("font-size", fs.clone());
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

fn endpoint_error(ep: &Endpoint, paths: &PathIndex, scope: &[String]) -> Error {
    let where_ = if scope.is_empty() {
        "at scene root".to_string()
    } else {
        format!("in '{}'", scope.join("."))
    };
    let mut msg = format!("link endpoint '{}' not found {}", ep.path.join("."), where_);
    let suggestions = paths.suggest(ep.path.last().expect("non-empty path"), scope);
    if !suggestions.is_empty() {
        let quoted: Vec<String> = suggestions.iter().map(|s| format!("'{}'", s)).collect();
        msg.push_str(&format!("; did you mean {}?", quoted.join(", ")));
    }
    Error::at(ep.span, msg)
}
