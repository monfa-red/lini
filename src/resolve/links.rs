//! Link resolution [SPEC 9]. A link resolves through the **node cascade**
//! [SPEC 13]: its type is `lini-link` (what `|-|` lowers to), its ancestors are its
//! scope chain, it has no id — so `stroke` is its wire and `color` / `font-*` its
//! labels, the ordinary vocabulary with no `link-*` family. Each statement layers
//! the baked base + scope `clearance`/`routing`, the `|-|` element rule, the
//! descendant / worn-class rules, then its own block; derives markers and line
//! style from the operator; resolves every endpoint by a scoped path-walk (with
//! did-you-mean errors); and cartesian-expands fan groups into one [`ResolvedLink`]
//! per pair.

use super::cascade::NodeFacts;
use super::ir::{
    Along, AttrMap, MarkerKind, ResolvedEndpoint, ResolvedLink, ResolvedText, ResolvedValue,
    Strategy,
};
use super::merge::{collapse, resolve_markers};
use super::scene::{PathIndex, SceneCtx};
use super::value::{resolve_groups, resolve_property};
use crate::ast::LineStyle;
use crate::error::Error;
use crate::syntax::ast::{Endpoint, EndpointGroup, Link};

/// The class every link wears [SPEC 9]: `|-|` lowers to it in desugar, so a link
/// resolves through the node cascade — its type tier, descendant/class rules, and
/// own block — with no `link-*` family.
pub const LINK_CLASS: &str = "lini-link";

/// Resolve one link statement into one resolved link per cartesian pair.
/// `path_prefix` scopes a lifted internal link to its host instance;
/// `scope_ancestors` is that scope's container chain (for descendant rules);
/// `base` is the baked link defaults plus the scope's `clearance`/`routing`.
pub fn resolve_link(
    w: &Link,
    ctx: &SceneCtx,
    paths: &PathIndex,
    path_prefix: &[String],
    scope_ancestors: &[NodeFacts],
    base: &[(String, ResolvedValue)],
) -> Result<Vec<ResolvedLink>, Error> {
    for class in &w.classes {
        if !ctx.sheet.defines_class(class) {
            return Err(Error::at(w.span, format!("unknown class '.{}'", class)));
        }
    }

    // A link is a node whose type is `lini-link`, whose ancestors are its scope
    // chain, with no id [SPEC 9, 4].
    let link_facts = NodeFacts {
        classes: std::iter::once(LINK_CLASS.to_string())
            .chain(w.classes.iter().cloned())
            .collect(),
        id: None,
    };

    // The cascade ladder, least-specific first [SPEC 4]: the baked base + scope
    // `clearance`/`routing`, the `|-|` element rule (the type tier), the descendant
    // / worn-class rules, then the link's own block. `stroke` is the wire, `font-*`
    // / `color` the labels — the same vocabulary a node uses.
    let mut ordered: Vec<(String, ResolvedValue)> = base.to_vec();
    ordered.extend(ctx.sheet.class_decls(LINK_CLASS));
    ordered.extend(ctx.sheet.node_layers(scope_ancestors, &link_facts));
    for d in &w.style {
        ordered.push((
            d.name.clone(),
            resolve_property(&d.name, &d.groups, d.span, ctx.vars, ctx.funcs)?,
        ));
    }

    let markers = resolve_markers(
        &ordered,
        MarkerKind::from_marker(w.op.start),
        MarkerKind::from_marker(w.op.end),
        w.span,
    )?;
    let mut attrs = collapse(&ordered);
    inject_line_style(&mut attrs, w.op.line);
    let routing = parse_routing(&attrs, w.span)?;
    attrs.map.remove("routing");

    // `along:` distributes the labels along the drawn route [SPEC 9]: one
    // fraction (0..1) per label, in order; an absent fraction is `Auto` (the
    // router spreads it). It is a placement directive, not a paint attr.
    let along: Vec<f64> = attrs
        .get("along")
        .map(collect_fractions)
        .unwrap_or_default();
    attrs.map.remove("along");

    // Labels ride `along:`, each a styleable text leaf [SPEC 9]: the link's text
    // baseline (font-size) overlaid with the label's own `{ }` (text-valid props).
    let mut texts: Vec<ResolvedText> = Vec::new();
    for (i, label) in w.labels.iter().enumerate() {
        let pos = along.get(i).copied().map_or(Along::Auto, Along::Fraction);
        let mut lattrs = link_text_attrs(&attrs);
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
            line: w.op.line,
            routing,
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

/// The resolved wiring strategy [SPEC 9]: `orthogonal` (the default) and
/// `straight` are built; `curved` is named but deferred.
fn parse_routing(attrs: &AttrMap, span: crate::span::Span) -> Result<Strategy, Error> {
    match attrs.get("routing") {
        None => Ok(Strategy::Orthogonal),
        Some(ResolvedValue::Ident(r)) if r == "orthogonal" => Ok(Strategy::Orthogonal),
        Some(ResolvedValue::Ident(r)) if r == "straight" => Ok(Strategy::Straight),
        Some(_) => Err(Error::at(
            span,
            "routing: 'orthogonal' and 'straight' are built; 'curved' is deferred (SPEC 22)",
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

/// A link's labels inherit its text context [SPEC 9]: every inheritable text prop
/// the link resolved — `font-*`, `color`, the spacings — seeds each label, which
/// its own `{ }` then overrides. This is how a `|-| { font-size: 14; color: red }`
/// restyles every label at once, exactly as a node's text inherits the node's.
fn link_text_attrs(link_attrs: &AttrMap) -> AttrMap {
    let mut map = AttrMap::new();
    for name in super::scene::INHERITED_TEXT {
        if let Some(v) = link_attrs.get(name) {
            map.insert(*name, v.clone());
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
