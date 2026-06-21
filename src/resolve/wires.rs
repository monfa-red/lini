//! Wire resolution (SPEC §9). Each wire statement cascades `wire { }` defaults →
//! class rules → its own block, derives markers and line style from the
//! operator, resolves every endpoint by a scoped path-walk (with did-you-mean
//! errors), and cartesian-expands fan groups into one [`ResolvedWire`] per pair.

use super::cascade::NodeFacts;
use super::ir::{
    Along, AttrMap, MarkerKind, ResolvedEndpoint, ResolvedText, ResolvedValue, ResolvedWire,
    VarEntry, VarKind, VarTable,
};
use super::merge::{collapse, resolve_markers};
use super::scene::{PathIndex, SceneCtx};
use super::value::resolve_groups;
use crate::ast::LineStyle;
use crate::error::Error;
use crate::syntax::ast::{Endpoint, EndpointGroup, Wire};

/// Resolve one wire statement into one resolved wire per cartesian pair.
/// `path_prefix` scopes a lifted internal wire to its host instance;
/// `wire_defaults` is the `wire { }` element rule (lowest specificity).
pub fn resolve_wire(
    w: &Wire,
    ctx: &SceneCtx,
    paths: &PathIndex,
    path_prefix: &[String],
    wire_defaults: &[(String, ResolvedValue)],
) -> Result<Vec<ResolvedWire>, Error> {
    for class in &w.classes {
        if !ctx.sheet.defines_class(class) {
            return Err(Error::at(w.span, format!("unknown class '.{}'", class)));
        }
    }

    // Cascade: wire defaults → class rules → own block (SPEC §4).
    let wire_facts = NodeFacts {
        classes: w.classes.clone(),
    };
    let mut ordered: Vec<(String, ResolvedValue)> = wire_defaults.to_vec();
    ordered.extend(ctx.sheet.node_layers(&[], &wire_facts));
    for d in &w.style {
        ordered.push((d.name.clone(), resolve_groups(&d.groups, d.span, ctx.vars)?));
    }

    let markers = resolve_markers(
        &ordered,
        MarkerKind::from_marker(w.op.start),
        MarkerKind::from_marker(w.op.end),
        w.span,
    )?;
    let mut attrs = collapse(&ordered);
    inject_line_style(&mut attrs, w.op.line);

    // `along:` distributes the labels along the drawn route (SPEC §9): one
    // fraction (0..1) per label, in order; an absent fraction is `Auto` (the
    // router spreads it). It is a placement directive, not a paint attr.
    let along: Vec<f64> = attrs
        .get("along")
        .map(collect_fractions)
        .unwrap_or_default();
    attrs.map.remove("along");

    // Labels are bare strings, placed by `along:` and styled together (SPEC §9).
    let mut texts: Vec<ResolvedText> = Vec::new();
    for (i, label) in w.labels.iter().enumerate() {
        let pos = along.get(i).copied().map_or(Along::Auto, Along::Fraction);
        texts.push(ResolvedText {
            text: label.text.clone(),
            along: pos,
            attrs: wire_text_attrs(AttrMap::new(), ctx.vars),
        });
    }

    // Cartesian fan expansion: one resolved wire per endpoint sequence.
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
        out.push(ResolvedWire {
            endpoints,
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

/// Default a wire label's `font-size` to the baked `--wire-font-size` (12) when
/// unset, so labels read a touch smaller than body text.
fn wire_text_attrs(mut map: AttrMap, vars: &VarTable) -> AttrMap {
    if map.get("font-size").is_none() {
        map.insert("font-size", baked_layout_var(vars, "wire-font-size"));
    }
    map
}

/// A `--name` reference carrying a layout var's baked value — reads as a number
/// at layout time, prints `var(--lini-name)` in live mode.
fn baked_layout_var(vars: &VarTable, name: &str) -> ResolvedValue {
    let baked = match vars.get(name) {
        Some(VarEntry {
            kind: VarKind::Layout,
            value,
        }) => Some(Box::new(value.clone())),
        _ => None,
    };
    ResolvedValue::LiveVar {
        name: name.to_string(),
        raw: false,
        baked,
    }
}

/// Flatten a chain's endpoint groups into every cartesian sequence — one per
/// resolved wire (`a & b -> c` → `a→c`, `b→c`).
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
    let mut msg = format!("wire endpoint '{}' not found {}", ep.path.join("."), where_);
    let suggestions = paths.suggest(ep.path.last().expect("non-empty path"), scope);
    if !suggestions.is_empty() {
        let quoted: Vec<String> = suggestions.iter().map(|s| format!("'{}'", s)).collect();
        msg.push_str(&format!("; did you mean {}?", quoted.join(", ")));
    }
    Error::at(ep.span, msg)
}
