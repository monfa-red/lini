//! Scene-tree resolution ([SPEC 3–8]). Each instance node becomes a
//! [`ResolvedInst`]: its type cascade resolved, descendant/class/block layers
//! applied, caption/label sugar expanded into `|caption|`/`|text|` children,
//! text properties inherited, define bodies materialised (with scoped ids), and
//! internal links lifted out for the link pass.

use super::cascade::{NodeFacts, Stylesheet};
use super::ir::{AttrMap, MarkerKind, Markers, NodeKind, ResolvedInst, ResolvedValue, VarTable};
use super::merge::{collapse, resolve_markers};
use super::value::{resolve_groups, resolve_property};
use crate::error::Error;
use crate::expr::{self, Expr, FuncTable, Value as ExprValue};
use crate::ledger::properties;
use crate::span::Span;
use crate::syntax::ast::{Call, Child, Decl, Link, Node, TextNode, Value};
use std::collections::HashMap;

/// An internal link lifted from a body or define, with its host's dot-path
/// prefix — resolved at program level once the whole tree (and its path index)
/// exists.
pub struct LiftedLink {
    pub link: Link,
    pub prefix: Vec<String>,
    /// The real container chain enclosing the link's written position, root →
    /// innermost — **every** container, anonymous ones included (sequence
    /// frames excepted: scope-transparent [SPEC 13]). The id-segment `prefix`
    /// cannot name an anonymous wrapper, so scope config (`clearance` /
    /// `routing`) and descendant-rule facts read this chain instead.
    pub chain: Vec<ScopeStep>,
}

/// One container on a link's written chain: its selector identity, resolved
/// attrs (the scope-config source), and display type (drawing diagnostics).
#[derive(Clone)]
pub struct ScopeStep {
    pub facts: NodeFacts,
    pub attrs: AttrMap,
    pub display_type: String,
}

/// Everything node resolution reads but does not mutate.
pub struct SceneCtx<'a> {
    pub sheet: &'a Stylesheet,
    pub vars: &'a VarTable,
    pub funcs: &'a crate::expr::FuncTable,
}

/// Resolve the top-level instances into scene nodes, collecting lifted internal
/// links. `text_ctx` seeds the inheritable text properties from the root config.
pub fn resolve_instances(
    instances: &[Child],
    ctx: &SceneCtx,
    root_attrs: &AttrMap,
    text_ctx: &AttrMap,
    id_seen: &mut HashMap<String, Span>,
    lifted: &mut Vec<LiftedLink>,
) -> Result<Vec<ResolvedInst>, Error> {
    // The file is the root container [SPEC 1]: a root that picks a scope-owning
    // engine wears its class for the cascade, so a scoped rule (`|sequence|
    // |note|`, `|drawing| |note|` — [SPEC 8]) reaches a root-scoped child exactly
    // as it reaches one inside a `|sequence|` / `|drawing|` node.
    let mut ancestors: Vec<NodeFacts> = root_facts(root_attrs).into_iter().collect();
    let mut steps: Vec<ScopeStep> = Vec::new();
    let mut nodes = Vec::with_capacity(instances.len());
    for child in instances {
        nodes.push(resolve_child(
            child,
            ctx,
            &mut ancestors,
            &mut steps,
            &[],
            text_ctx,
            id_seen,
            lifted,
        )?);
    }
    drop_blank_text(&mut nodes, root_attrs);
    Ok(nodes)
}

/// The synthetic cascade identity of a `{ layout: sequence }` / `{ layout:
/// drawing }` root — the engines whose scoped rules select by container type.
/// Links share it: a root drawing's `|-|`s match `|drawing| |-|` exactly as a
/// `|drawing#x|`'s do.
pub(super) fn root_facts(root_attrs: &AttrMap) -> Option<NodeFacts> {
    match root_attrs.get("layout") {
        Some(ResolvedValue::Ident(l)) if l == "sequence" || l == "drawing" => Some(NodeFacts {
            classes: vec![format!("lini-{l}")],
            id: None,
        }),
        _ => None,
    }
}

/// Resolve a body child [SPEC 3]: a box recurses; a bare string becomes a text
/// node carrying the inherited text properties.
#[allow(clippy::too_many_arguments)]
fn resolve_child(
    child: &Child,
    ctx: &SceneCtx,
    ancestors: &mut Vec<NodeFacts>,
    steps: &mut Vec<ScopeStep>,
    path_prefix: &[String],
    text_ctx: &AttrMap,
    id_seen: &mut HashMap<String, Span>,
    lifted: &mut Vec<LiftedLink>,
) -> Result<ResolvedInst, Error> {
    match child {
        Child::Box(n) => resolve_node(
            n,
            ctx,
            ancestors,
            steps,
            path_prefix,
            text_ctx,
            id_seen,
            lifted,
        ),
        Child::Text(t) => text_inst(t, ctx, text_ctx),
    }
}

/// Resolve one node into a [`ResolvedInst`], recursing into its children.
/// `ancestors` is the matcher chain (root → parent); `path_prefix` is the
/// dot-path to this node's parent; `text_ctx` carries inherited text props.
#[allow(clippy::too_many_arguments)]
pub fn resolve_node(
    node: &Node,
    ctx: &SceneCtx,
    ancestors: &mut Vec<NodeFacts>,
    steps: &mut Vec<ScopeStep>,
    path_prefix: &[String],
    text_ctx: &AttrMap,
    id_seen: &mut HashMap<String, Span>,
    lifted: &mut Vec<LiftedLink>,
) -> Result<ResolvedInst, Error> {
    let type_name = node.ty.as_deref().unwrap_or("box");
    // Post-desugar the type is always a primitive; anything else means the input
    // bypassed desugar (the "dumb core" guard).
    let kind = NodeKind::parse(type_name)
        .ok_or_else(|| Error::at(node.span, format!("unknown type '{}'", type_name)))?;

    // Split the worn classes: `.lini-*` are the type tier (the primitive plus the
    // render type_chain), user classes are tier 3 (and must be defined).
    let primitive_class = format!("lini-{}", kind.as_str());
    let mut type_chain = Vec::new();
    let mut applied_styles = Vec::new();
    for c in &node.classes {
        if let Some(name) = c.strip_prefix("lini-") {
            if *c != primitive_class {
                type_chain.push(name.to_string());
            }
        } else {
            if !ctx.sheet.defines_class(c) {
                return Err(Error::at(node.span, format!("unknown class '.{}'", c)));
            }
            applied_styles.push(c.clone());
        }
    }

    // Sides (`top`/`bottom`/`left`/`right`) are free as ids — a `:side` is peeled
    // by position now, not from the path [SPEC 18] — so the only id error here is
    // a duplicate.
    if let Some(id) = &node.id {
        let full = join_path(path_prefix, id);
        if let Some(prev) = id_seen.get(&full) {
            return Err(Error::at(node.span, format!("duplicate id '{}'", id)).with_related(*prev));
        }
        id_seen.insert(full, node.span);
    }

    let facts = NodeFacts {
        classes: node.classes.clone(),
        id: node.id.clone(),
    };

    // The cascade ladder, least-specific first [SPEC 4]: the worn `.lini-*`
    // classes as the type tier (folded base→derived — worn order is
    // derived→base→primitive, so iterate reversed), then descendant + user-class
    // layers, then the instance's own block.
    let mut ordered: Vec<(String, ResolvedValue)> = Vec::new();
    for c in node.classes.iter().rev() {
        if c.starts_with("lini-") {
            ordered.extend(ctx.sheet.class_decls(c));
        }
    }
    ordered.extend(ctx.sheet.node_layers(ancestors, &facts));
    for d in &node.style {
        // A `fn:` series formula is held unevaluated: its `x` / `u` are unbound here
        // and are bound only at chart layout, once the x-domain is fixed
        // [SPEC 14.3] — the same defer `points:` would need if its domain came from siblings.
        if d.name == "fn" {
            ordered.push(("fn".to_string(), defer_fn(d)?));
            continue;
        }
        // A `points:` parametric expression in `u` is sampled into a vertex list
        // here [SPEC 10.7]; any other value folds normally.
        if d.name == "points"
            && let Some(sampled) = sample_points(d, &node.style, ctx.funcs)?
        {
            ordered.push(("points".to_string(), sampled));
            continue;
        }
        ordered.push((
            d.name.clone(),
            resolve_property(&d.name, &d.groups, d.span, ctx.vars, ctx.funcs)?,
        ));
    }

    let markers = resolve_markers(&ordered, MarkerKind::None, MarkerKind::None, node.span)?;
    let attrs = collapse(&ordered);

    if kind == NodeKind::Slant
        && let Some(skew) = attrs.number("skew")
        && (skew <= -89.0 || skew >= 89.0)
    {
        return Err(Error::at(
            node.span,
            format!("skew: {} must be in (-89, 89)", skew),
        ));
    }

    // Inherited text context for children: overlay this node's own text props.
    let mut child_text_ctx = text_ctx.clone();
    for name in properties::inherited_text() {
        if let Some(v) = attrs.get(name) {
            child_text_ctx.insert(name, v.clone());
        }
    }

    // A sequence frame (`loop`/`opt`/`alt`/`else`) is **scope-transparent** [SPEC 13]: it
    // opens no scope, so it contributes no path segment — its body links lift with the
    // enclosing sequence's prefix and resolve against the sequence's participants, never
    // frame-local ids. (Frames are usually unnamed; this also covers a named one.)
    let mut child_prefix = path_prefix.to_vec();
    if let Some(id) = &node.id
        && !is_frame_type(&type_chain)
    {
        child_prefix.push(id.clone());
    }

    // This node's chain step: pushed before its body links lift, so the chain
    // holds the link's real written position — anonymous containers included.
    // Sequence frames stay scope-transparent [SPEC 13], mirroring the prefix.
    let own_step = (!is_frame_type(&type_chain)).then(|| ScopeStep {
        facts: facts.clone(),
        attrs: attrs.clone(),
        display_type: type_chain
            .first()
            .cloned()
            .unwrap_or_else(|| kind.as_str().to_string()),
    });
    if let Some(step) = own_step.clone() {
        steps.push(step);
    }

    // Internal links lift to program level (define bodies are inlined by desugar,
    // so the node's own `[ ]` holds them already).
    for w in &node.links {
        lifted.push(LiftedLink {
            link: w.clone(),
            prefix: child_prefix.clone(),
            chain: steps.clone(),
        });
    }

    // An `|icon|` is named by its `symbol` [SPEC 7], not its id, so it gains no
    // id-as-label (desugar skips it); a bare string it carries is an ordinary
    // centred-text child — the same leaf through the same renderer, so `translate`
    // and styling reach it exactly as on any node's text.
    let is_icon = kind == NodeKind::Icon;
    if is_icon {
        validate_icon(&attrs, node.span)?;
    } else if kind == NodeKind::Image {
        validate_fit(&attrs, node.span)?;
    } else if type_chain.iter().any(|t| t == "surface-finish") {
        validate_finish(&attrs, node.span)?;
    }

    ancestors.push(facts);
    let mut children = Vec::new();
    for child in &node.children {
        children.push(resolve_child(
            child,
            ctx,
            ancestors,
            steps,
            &child_prefix,
            &child_text_ctx,
            id_seen,
            lifted,
        )?);
    }
    ancestors.pop();
    if own_step.is_some() {
        steps.pop();
    }
    drop_blank_text(&mut children, &attrs);

    Ok(ResolvedInst {
        id: node.id.clone(),
        kind,
        type_chain,
        applied_styles,
        // Desugar lowered box / container / icon labels into children / `symbol`, so
        // this is `None` for them; a geometry primitive keeps its label (a chart
        // reads a `|line|` series' legend name from it). Render ignores it on a shape.
        label: node.label.as_ref().map(|t| t.text.clone()),
        font: crate::font::Font::of(&child_text_ctx),
        attrs,
        own_style: AttrMap::new(),
        markers,
        children,
        span: node.span,
    })
}

/// Hold a `fn:` series formula unevaluated [SPEC 14.3]: each value in the
/// comma-group is a `(…)` expression — or a bare constant — parsed now (so a
/// syntax error surfaces here, with this span) but **not** evaluated, since its
/// `x` / `u` bind only at chart layout. A whole-domain `fn:` is one expression;
/// a per-band list is comma-separated [SPEC 2/14.5]: `fn: (u*10), 5, (2*u)`.
fn defer_fn(d: &Decl) -> Result<ResolvedValue, Error> {
    let mut exprs = Vec::with_capacity(d.groups.len());
    for group in &d.groups {
        let src = match group.as_slice() {
            [Value::Expr(s)] => s.clone(),
            [Value::Number(n)] => n.to_string(),
            [_, _, ..] => {
                return Err(Error::at(
                    d.span,
                    "'fn' segments are comma-separated — 'fn: (u*10), 5, (2*u)'",
                ));
            }
            _ => {
                return Err(Error::at(
                    d.span,
                    "'fn' takes expressions (or bare constants)",
                ));
            }
        };
        exprs.push(Expr::parse(&src).map_err(|e| Error::at(d.span, e.0))?);
    }
    Ok(ResolvedValue::Deferred(exprs))
}

/// Sample a parametric `points:` into a vertex list [SPEC 10.7]: a `(…u…)` group or
/// a named curve `wave(20, 3)` whose `u` sweeps 0→1 over `samples:` steps, each step
/// evaluating to a point. Returns `None` for a literal points list or a constant
/// expression — the normal fold handles those.
fn sample_points(
    d: &Decl,
    style: &[Decl],
    funcs: &FuncTable,
) -> Result<Option<ResolvedValue>, Error> {
    // The value must be a single scalar: a `(…)` group or a call.
    let src = match d.groups.as_slice() {
        [group] => match group.as_slice() {
            [Value::Expr(s)] => s.clone(),
            [Value::Call(c)] => call_src(c),
            _ => return Ok(None),
        },
        _ => return Ok(None),
    };
    let expr = Expr::parse(&src).map_err(|e| Error::at(d.span, e.0))?;
    // Parametric iff it sweeps `u` or calls a (possibly `u`-bearing) user function;
    // a constant expression folds via the normal path instead.
    let parametric = expr
        .referenced_names()
        .iter()
        .any(|n| n == "u" || funcs.contains(n));
    if !parametric {
        return Ok(None);
    }
    let n = sample_count(style).max(2);
    // `u` sweeps 0 → 1 across the samples — the same ambient-sampling seam a chart's
    // `fn:` uses for `x` [SPEC 14.3], shared via `expr::sample`.
    let us: Vec<f64> = (0..n).map(|i| i as f64 / (n - 1) as f64).collect();
    let mut pts = Vec::with_capacity(n);
    for v in expr::sample(&expr, "u", &us, funcs).map_err(|e| Error::at(d.span, e.0))? {
        match v {
            ExprValue::Point(x, y) => pts.push(ResolvedValue::Tuple(vec![
                ResolvedValue::Number(x),
                ResolvedValue::Number(y),
            ])),
            ExprValue::Number(_) => {
                return Err(Error::at(
                    d.span,
                    "a parametric 'points:' expression must return a point '(x, y)'",
                ));
            }
        }
    }
    Ok(Some(ResolvedValue::List(pts)))
}

/// A call as expression source (`wave(20, 3)`), so a named curve folds through the
/// same engine as any expression. Numeric / expr / call args only (geometry).
fn call_src(c: &Call) -> String {
    let args: Vec<String> = c.args.iter().map(value_src).collect();
    format!("{}({})", c.name, args.join(", "))
}

fn value_src(v: &Value) -> String {
    match v {
        Value::Number(n) => n.to_string(),
        Value::Expr(s) => format!("({s})"),
        Value::Call(c) => call_src(c),
        // A non-numeric argument is invalid in geometry; emit it so the parse /
        // eval reports a clear error rather than silently dropping it.
        Value::Percent(n) => format!("{n}%"),
        Value::String(s) => format!("\"{s}\""),
        Value::Hex(h) => format!("#{h}"),
        Value::Ident(s) => s.clone(),
        Value::Var(s) => format!("--{s}"),
        Value::Tuple(items) => items.iter().map(value_src).collect::<Vec<_>>().join(" "),
        Value::NamedCall(c, name) => format!("{}:{name}", call_src(c)),
    }
}

/// The `samples:` count from a node's own block (default 2 — a straight segment).
fn sample_count(style: &[Decl]) -> usize {
    style
        .iter()
        .find(|d| d.name == "samples")
        .and_then(|d| match d.groups.as_slice() {
            [group] => match group.as_slice() {
                [Value::Number(n)] if *n >= 2.0 => Some(*n as usize),
                _ => None,
            },
            _ => None,
        })
        .unwrap_or(2)
}

/// A resolved text node [SPEC 3/6]: content carrying the text properties
/// inherited from its container, its worn classes' text-valid declarations
/// (tier 3), then its own `{ }` style (text-valid props only, highest). `Text`
/// is internal — never a user `|type|`, only the kind of a string node (a label,
/// a cell, canvas text). The own style renders as a `style=` on the `<text>`;
/// `attrs` is the effective context for measurement.
fn text_inst(t: &TextNode, ctx: &SceneCtx, text_ctx: &AttrMap) -> Result<ResolvedInst, Error> {
    let mut attrs = AttrMap::new();
    for name in properties::inherited_text() {
        if let Some(v) = text_ctx.get(name) {
            attrs.insert(name, v.clone());
        }
    }
    // Tier 3 [SPEC 4]: worn classes, below the leaf's own block.
    let (type_chain, applied_styles) = apply_text_classes(&t.classes, &mut attrs, ctx, t.span)?;
    // The text's own `{ }`: text-valid props only — a box property errors and
    // points at `|block|` [SPEC 3, 19].
    let mut own_style = AttrMap::new();
    for d in &t.style {
        if !properties::is_text_valid(&d.name) {
            return Err(Error::at(
                d.span,
                format!("'{}' needs a box — wrap the text in '|block|'", d.name),
            ));
        }
        let v = resolve_groups(&d.groups, d.span, ctx.vars, ctx.funcs)?;
        attrs.insert(d.name.as_str(), v.clone());
        own_style.insert(d.name.as_str(), v);
    }
    Ok(ResolvedInst {
        id: None,
        kind: NodeKind::Text,
        type_chain,
        applied_styles,
        label: Some(t.text.clone()),
        font: crate::font::Font::of(&attrs),
        attrs,
        own_style,
        markers: Markers::default(),
        children: Vec::new(),
        span: t.span,
    })
}

/// Apply a text leaf's worn classes [SPEC 3/4] — the tier-3 cascade layer shared
/// by a node's text and a link's label, so the two never drift. Splits the worn
/// chain (`.lini-*` → the render type chain, the rest → user classes, each of
/// which must be defined), then overlays the **user** classes' **text-valid**
/// declarations onto `attrs` in definition order; a non-text-valid class
/// declaration is inert — the class-polymorphism law — never an error. Returns
/// `(type_chain, applied_styles)` for the `<text>` element's classes.
pub(super) fn apply_text_classes(
    classes: &[String],
    attrs: &mut AttrMap,
    ctx: &SceneCtx,
    span: Span,
) -> Result<(Vec<String>, Vec<String>), Error> {
    let mut type_chain = Vec::new();
    let mut applied_styles = Vec::new();
    for c in classes {
        if let Some(name) = c.strip_prefix("lini-") {
            if c != "lini-text" {
                type_chain.push(name.to_string());
            }
        } else if ctx.sheet.defines_class(c) {
            applied_styles.push(c.clone());
        } else {
            return Err(Error::at(span, format!("unknown class '.{}'", c)));
        }
    }
    for (name, v) in ctx.sheet.user_class_decls(classes) {
        if properties::is_text_valid(&name) {
            attrs.insert(name.as_str(), v);
        }
    }
    Ok((type_chain, applied_styles))
}

/// An `|icon|` must name a known `symbol` [SPEC 7]. Errors point at the node,
/// suggest the nearest name, or — when the set was not compiled in — hint at the
/// `icons` feature.
fn validate_icon(attrs: &AttrMap, span: Span) -> Result<(), Error> {
    validate_fit(attrs, span)?;
    let symbol = match attrs.get("symbol") {
        Some(ResolvedValue::Ident(s) | ResolvedValue::String(s)) => s.as_str(),
        Some(_) => {
            return Err(Error::at(
                span,
                "'symbol' must be an icon name, e.g. { symbol: heart }",
            ));
        }
        None => {
            return Err(Error::at(
                span,
                "'|icon|' needs a 'symbol' (e.g. { symbol: heart })",
            ));
        }
    };
    if crate::icon::lookup(symbol).is_some() {
        return Ok(());
    }
    if !crate::icon::ENABLED {
        return Err(Error::at(
            span,
            "icon support is not built in — rebuild with the `icons` feature",
        ));
    }
    let msg = format!(
        "unknown icon '{symbol}'{}",
        crate::suggest::did_you_mean(&crate::icon::suggest(symbol))
    );
    Err(Error::at(span, msg))
}

/// On `|surface-finish|` the `symbol` homonym picks the ISO 1302 vee variant
/// [SPEC 15.9/16] — anything else errors here, at the node.
fn validate_finish(attrs: &AttrMap, span: Span) -> Result<(), Error> {
    match attrs.get("symbol") {
        None => Ok(()),
        Some(ResolvedValue::Ident(s))
            if matches!(s.as_str(), "basic" | "machined" | "prohibited") =>
        {
            Ok(())
        }
        Some(_) => Err(Error::at(
            span,
            "'symbol' picks the vee — basic, machined, or prohibited",
        )),
    }
}

/// `fit` [SPEC 7] accepts only the four object-fit keywords — used by `|icon|`
/// and `|image|` to map content into the box.
fn validate_fit(attrs: &AttrMap, span: Span) -> Result<(), Error> {
    match attrs.get("fit") {
        None => Ok(()),
        Some(ResolvedValue::Ident(s))
            if matches!(s.as_str(), "auto" | "contain" | "cover" | "stretch") =>
        {
            Ok(())
        }
        Some(_) => Err(Error::at(
            span,
            "'fit' must be auto, contain, cover, or stretch",
        )),
    }
}

/// Drop empty (`""`) text children — they suppress the label and would emit an
/// empty `<text>` — **unless** the container is a grid, where an empty `""` is a
/// real cell that holds its track [SPEC 3/12]. A grid is positional, so its
/// cells keep their slots; flow has no slot for an empty to hold.
fn drop_blank_text(children: &mut Vec<ResolvedInst>, container: &AttrMap) {
    let is_grid = matches!(container.get("layout"), Some(ResolvedValue::Ident(s)) if s == "grid");
    if !is_grid {
        children.retain(|c| !is_blank_anon_text(c));
    }
}

/// A text node with no visible content and no id — from a `""` label.
fn is_blank_anon_text(r: &ResolvedInst) -> bool {
    r.id.is_none() && r.kind == NodeKind::Text && r.label.as_deref().is_none_or(str::is_empty)
}

/// A sequence frame type [SPEC 13] — scope-transparent, so it adds no path segment.
fn is_frame_type(type_chain: &[String]) -> bool {
    type_chain
        .iter()
        .any(|t| crate::desugar::FRAME_TYPES.contains(&t.as_str()))
}

fn join_path(prefix: &[String], id: &str) -> String {
    if prefix.is_empty() {
        id.to_string()
    } else {
        format!("{}.{}", prefix.join("."), id)
    }
}

// ─────────────────────────── Path index ───────────────────────────

/// Maps every node's fully-qualified dot-path, for endpoint resolution and
/// auto-create [SPEC 9].
pub struct PathIndex {
    paths: Vec<String>,
}

impl PathIndex {
    pub fn build(nodes: &[ResolvedInst]) -> Self {
        let mut paths = Vec::new();
        for n in nodes {
            walk_paths(n, &mut Vec::new(), &mut paths);
        }
        Self { paths }
    }

    pub fn contains(&self, path: &str) -> bool {
        self.paths.iter().any(|p| p == path)
    }

    /// An endpoint is an exact path from the link's scope (the caller prepends
    /// the scope prefix). There is no search.
    pub fn resolve(&self, query: &[String]) -> Option<String> {
        let joined = query.join(".");
        self.contains(&joined).then_some(joined)
    }

    /// Same-named paths to propose in a did-you-mean error, stripped to the form
    /// typed in `scope`. Sorted, deduped, capped at 3.
    pub fn suggest(&self, seg: &str, scope: &[String]) -> Vec<String> {
        let prefix = if scope.is_empty() {
            String::new()
        } else {
            format!("{}.", scope.join("."))
        };
        let mut hits: Vec<String> = self
            .paths
            .iter()
            .filter(|p| final_segment(p) == seg)
            .filter_map(|p| {
                if prefix.is_empty() {
                    Some(p.clone())
                } else {
                    p.strip_prefix(&prefix).map(str::to_string)
                }
            })
            .collect();
        hits.sort();
        hits.dedup();
        hits.truncate(3);
        hits
    }
}

fn final_segment(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or(path)
}

/// Find the node a path segment names at this level — or inside an
/// **anonymous** container here, which is scope-transparent [SPEC 9]: its
/// children address as the parent's, exactly as this module's path prefixes
/// and the routing index already skip it. Traversed wrappers append to `via`
/// (the container chain still wants their facts and config). Deterministic:
/// ids are unique within a transparent scope — the duplicate-id check keys
/// full transparent paths.
pub(crate) fn find_in_scope<'a>(
    nodes: &'a [ResolvedInst],
    seg: &str,
    via: &mut Vec<&'a ResolvedInst>,
) -> Option<&'a ResolvedInst> {
    if let Some(n) = nodes.iter().find(|n| n.id.as_deref() == Some(seg)) {
        return Some(n);
    }
    for anon in nodes.iter().filter(|n| n.id.is_none()) {
        let mark = via.len();
        via.push(anon);
        if let Some(hit) = find_in_scope(&anon.children, seg, via) {
            return Some(hit);
        }
        via.truncate(mark);
    }
    None
}

fn walk_paths(n: &ResolvedInst, stack: &mut Vec<String>, out: &mut Vec<String>) {
    if let Some(id) = &n.id {
        stack.push(id.clone());
        out.push(stack.join("."));
    }
    for c in &n.children {
        walk_paths(c, stack, out);
    }
    if n.id.is_some() {
        stack.pop();
    }
}
