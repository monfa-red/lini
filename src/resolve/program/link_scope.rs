//! The link-scope cascade [SPEC 9, 15]: a link's base layer, the container
//! chain it matches descendant rules against, and the drawing-scope predicates.

use super::*;

// ─────────────────────────── Render inputs ───────────────────────────

/// The baked link base [SPEC 10.5] — a link's lowest-specificity layer, below
/// the scope cascade, class rules, and its own block. The values live in the one
/// tuning home (`ledger::defaults`).
pub(super) fn baked_link_defaults(
    vars: &VarTable,
    funcs: &FuncTable,
) -> Result<Vec<(String, ResolvedValue)>, Error> {
    let mut out = Vec::new();
    for d in crate::ledger::defaults::link_defaults() {
        out.push((
            d.name.clone(),
            resolve_groups(&d.groups, d.span, vars, funcs)?,
        ));
    }
    Ok(out)
}

/// The container chain from the scene root down to `scope` (each segment an id),
/// stopping at the first missing segment. **Anonymous containers are
/// scope-transparent** [SPEC 9] — a segment may sit inside an id-less wrapper
/// (an unnamed `|page|`, a `|group|`): the walk descends through it and keeps
/// the wrapper in the chain, so its facts still match descendant rules and its
/// config still cascades. The root is not a node, so it is absent —
/// [`link_scope`] folds it in for the config cascade, and a bare `|-|` matches
/// every link with no ancestor needed.
fn scope_chain<'a>(nodes: &'a [ResolvedInst], scope: &[String]) -> Vec<&'a ResolvedInst> {
    let mut out = Vec::new();
    let mut cur = nodes;
    for seg in scope {
        match scene::find_in_scope(cur, seg, &mut out) {
            Some(n) => {
                out.push(n);
                cur = &n.children;
            }
            None => break,
        }
    }
    out
}

/// The selector identity of a resolved container [SPEC 4]: its worn `.lini-*` type
/// classes (the type chain plus its primitive) and user classes, and its id — what a
/// descendant `|table| |-|` matches against.
fn inst_facts(inst: &ResolvedInst) -> NodeFacts {
    let mut classes: Vec<String> = inst
        .type_chain
        .iter()
        .map(|t| format!("lini-{t}"))
        .collect();
    classes.push(format!("lini-{}", inst.kind.as_str()));
    classes.extend(inst.applied_styles.iter().cloned());
    NodeFacts {
        classes,
        id: inst.id.clone(),
    }
}

/// The built-in scoped rules [SPEC 8]: `|sequence| |note|` and `|drawing| |note|`
/// compact the core note card where drafting convention expects. Ordinary
/// descendant rules at the lowest source position, so any user rule of equal
/// specificity wins — the `|table| |cell|` mechanism, engine-supplied.
pub(super) fn scoped_rules() -> Vec<Rule> {
    use crate::span::Span;
    let number = |name: &str, ns: &[f64]| Decl {
        name: name.to_string(),
        groups: vec![ns.iter().map(|n| Value::Number(*n)).collect()],
        span: Span::empty(),
    };
    let compact = |scope: &str| Rule {
        selector: Selector {
            units: vec![
                SelUnit::Class(format!("lini-{scope}")),
                SelUnit::Class("lini-note".to_string()),
            ],
        },
        decls: vec![
            number("padding", &[6.0, 10.0]),
            number("font-size", &[13.0]),
        ],
        span: Span::empty(),
    };
    vec![compact("sequence"), compact("drawing")]
}

/// Whether a link's scope is a drawing [SPEC 15] — its immediate container (or
/// the root, for top-level links) resolved `layout: drawing`. Gates the drawing
/// statements: the measuring ops, `||`, `tol:`, and the wider anchor set.
fn scope_is_drawing(nodes: &[ResolvedInst], root_attrs: &AttrMap, scope: &[String]) -> bool {
    let attrs = scope_chain(nodes, scope)
        .last()
        .map_or(root_attrs, |c| &c.attrs);
    is_drawing(attrs)
}

/// A link scope's drawing classification [SPEC 15/20]: whether the immediate
/// container is a drawing, and — when it is not but a drawing encloses it —
/// that container's display type, so a mate written inside a `|row|` in a
/// drawing errors "a '|row|' places its own children".
pub(super) fn link_scope_kind(
    nodes: &[ResolvedInst],
    root_attrs: &AttrMap,
    scope: &[String],
) -> links::LinkScope {
    let drawing = scope_is_drawing(nodes, root_attrs, scope);
    let flow_in_drawing = if drawing {
        None
    } else {
        let chain = scope_chain(nodes, scope);
        let enclosed = is_drawing(root_attrs)
            || chain
                .iter()
                .take(chain.len().saturating_sub(1))
                .any(|c| is_drawing(&c.attrs));
        match (enclosed, chain.last()) {
            (true, Some(container)) => Some(
                container
                    .type_chain
                    .first()
                    .cloned()
                    .unwrap_or_else(|| container.kind.as_str().to_string()),
            ),
            _ => None,
        }
    };
    // A detail scope [SPEC 15.8] — a `|drawing| { of: <magnifier> }` — re-lays
    // its geometry from the source at layout, so its endpoints defer. A section
    // (`of:` a `|plane|`) authors its geometry, so it resolves normally.
    let detail = scope_chain(nodes, scope).last().is_some_and(
        |c| matches!(c.attrs.get("of"), Some(ResolvedValue::Ident(id)) if is_magnifier(nodes, id)),
    );
    links::LinkScope {
        drawing,
        flow_in_drawing,
        detail,
    }
}

/// Whether a node with id `id` anywhere in the scene is a `|magnifier|`
/// [SPEC 15.8] — the `of:` reference a detail scope's deferral keys on.
fn is_magnifier(nodes: &[ResolvedInst], id: &str) -> bool {
    nodes.iter().any(|n| {
        (n.id.as_deref() == Some(id) && n.type_chain.iter().any(|t| t == "magnifier"))
            || is_magnifier(&n.children, id)
    })
}

/// A link's scope inputs: its `base` layer — the baked defaults plus the nearest
/// scope's config props (`clearance` / `routing` [SPEC 9], root → container
/// chain, nearest winning; geometry, not paint, so they live on a container's
/// own block — unlike the wire and label look, which come from `|-|` rules) — and the
/// `ancestors` its descendant `|…| |-|` rules match against. A root-scope link
/// passes `scope: &[]`.
pub(super) fn link_scope(
    baked: &[(String, ResolvedValue)],
    nodes: &[ResolvedInst],
    root_attrs: &AttrMap,
    scope: &[String],
) -> (Vec<(String, ResolvedValue)>, Vec<NodeFacts>) {
    let chain = scope_chain(nodes, scope);
    let mut base = baked.to_vec();
    // The drafting line-weight contrast [SPEC 15.1]: geometry keeps stroke 2,
    // a drawing's links thin to 1. A **scope default**, not a rule — it rides
    // the base layer below every user rule, so a plain `|-| { stroke-width: … }`
    // overrides it. The same immediate-scope predicate as the mate gate: a
    // `|row|` nested in a drawing owns ordinary routed links, weight 2.
    if scope_is_drawing(nodes, root_attrs, scope) {
        base.push((
            "stroke-width".to_string(),
            ResolvedValue::Number(consts::DRAWING_LINK_STROKE_WIDTH),
        ));
        // …and its annotation text reads at the caption size, 12 — the same
        // base-layer seat, so a plain `|-| { font-size: … }` still wins.
        base.push((
            "font-size".to_string(),
            ResolvedValue::Number(consts::DRAWING_LINK_FONT_SIZE),
        ));
    }
    for prop in properties::scope_link_props() {
        let nearest = chain
            .iter()
            .rev()
            .find_map(|n| n.attrs.get(prop))
            .or_else(|| root_attrs.get(prop));
        if let Some(v) = nearest {
            base.push((prop.to_string(), v.clone()));
        }
    }
    // The file is the root container [SPEC 1]: a root engine's synthetic fact
    // heads the chain, so `|drawing| |-|` reaches a root drawing's links.
    let mut ancestors: Vec<NodeFacts> = scene::root_facts(root_attrs).into_iter().collect();
    ancestors.extend(chain.iter().map(|n| inst_facts(n)));
    (base, ancestors)
}
