//! Scene-tree resolution (SPEC §3–§8). Each instance node becomes a
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
use crate::span::Span;
use crate::syntax::ast::{Call, Child, Decl, Link, Node, TextNode, Value};
use std::collections::HashMap;

/// Text properties that cascade to descendant text (SPEC §10): nearest ancestor
/// wins, a node's own value beats inherited.
pub(super) const INHERITED_TEXT: &[&str] = &[
    "font-family",
    "font-size",
    "font-weight",
    "font-style",
    "text-transform",
    "text-decoration",
    "text-shadow",
    "letter-spacing",
    "line-spacing",
    "color",
];

/// An internal link lifted from a body or define, with its host's dot-path
/// prefix — resolved at program level once the whole tree (and its path index)
/// exists.
pub struct LiftedLink {
    pub link: Link,
    pub prefix: Vec<String>,
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
    let mut ancestors = Vec::new();
    let mut nodes = Vec::with_capacity(instances.len());
    for child in instances {
        nodes.push(resolve_child(
            child,
            ctx,
            &mut ancestors,
            &[],
            text_ctx,
            id_seen,
            lifted,
        )?);
    }
    drop_blank_text(&mut nodes, root_attrs);
    Ok(nodes)
}

/// Resolve a body child (SPEC §3): a box recurses; a bare string becomes a text
/// node carrying the inherited text properties.
#[allow(clippy::too_many_arguments)]
fn resolve_child(
    child: &Child,
    ctx: &SceneCtx,
    ancestors: &mut Vec<NodeFacts>,
    path_prefix: &[String],
    text_ctx: &AttrMap,
    id_seen: &mut HashMap<String, Span>,
    lifted: &mut Vec<LiftedLink>,
) -> Result<ResolvedInst, Error> {
    match child {
        Child::Box(n) => resolve_node(n, ctx, ancestors, path_prefix, text_ctx, id_seen, lifted),
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
    // by position now, not from the path (SPEC §18) — so the only id error here is
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

    // The cascade ladder, least-specific first (SPEC §12): the worn `.lini-*`
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
        // and are bound only at chart layout, once the x-domain is fixed ([CHARTS.md]
        // §4) — the same defer `points:` would need if its domain came from siblings.
        if d.name == "fn" {
            ordered.push(("fn".to_string(), defer_fn(d)?));
            continue;
        }
        // A `points:` parametric expression in `u` is sampled into a vertex list
        // here (SPEC §11.7); any other value folds normally.
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
    for name in INHERITED_TEXT {
        if let Some(v) = attrs.get(name) {
            child_text_ctx.insert(*name, v.clone());
        }
    }

    let mut child_prefix = path_prefix.to_vec();
    if let Some(id) = &node.id {
        child_prefix.push(id.clone());
    }

    // Internal links lift to program level (define bodies are inlined by desugar,
    // so the node's own `[ ]` holds them already).
    for w in &node.links {
        lifted.push(LiftedLink {
            link: w.clone(),
            prefix: child_prefix.clone(),
        });
    }

    // An `|icon|` is named by its `symbol` (SPEC §7), not its id, so it gains no
    // id-as-label (desugar skips it); a bare string it carries is an ordinary
    // centred-text child — the same leaf through the same renderer, so `translate`
    // and styling reach it exactly as on any node's text.
    let is_icon = kind == NodeKind::Icon;
    if is_icon {
        validate_icon(&attrs, node.span)?;
    } else if kind == NodeKind::Image {
        validate_fit(&attrs, node.span)?;
    }

    ancestors.push(facts);
    let mut children = Vec::new();
    for child in &node.children {
        children.push(resolve_child(
            child,
            ctx,
            ancestors,
            &child_prefix,
            &child_text_ctx,
            id_seen,
            lifted,
        )?);
    }
    ancestors.pop();
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
        attrs,
        own_style: AttrMap::new(),
        markers,
        children,
        span: node.span,
    })
}

/// Hold a `fn:` series formula unevaluated ([CHARTS.md] §4): each value in the
/// single space-group is a backtick expression — or a bare constant — parsed now
/// (so a syntax error surfaces here, with this span) but **not** evaluated, since
/// its `x` / `u` bind only at chart layout. A whole-domain `fn:` is one expression;
/// a per-band list ([CHARTS.md] §7) is several.
fn defer_fn(d: &Decl) -> Result<ResolvedValue, Error> {
    let [group] = d.groups.as_slice() else {
        return Err(Error::at(
            d.span,
            "'fn' takes a backtick expression or a space-separated per-band list, not a comma list",
        ));
    };
    let mut exprs = Vec::with_capacity(group.len());
    for v in group {
        let src = match v {
            Value::Expr(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => {
                return Err(Error::at(
                    d.span,
                    "'fn' takes backtick expressions (or bare constants)",
                ));
            }
        };
        exprs.push(Expr::parse(&src).map_err(|e| Error::at(d.span, e.0))?);
    }
    Ok(ResolvedValue::Deferred(exprs))
}

/// Sample a parametric `points:` into a vertex list (SPEC §11.7): a backtick
/// `` `(…u…)` `` or a named curve `wave(20, 3)` whose `u` sweeps 0→1 over
/// `samples:` steps, each step evaluating to a point. Returns `None` for a literal
/// points list or a constant expression — the normal fold handles those.
fn sample_points(
    d: &Decl,
    style: &[Decl],
    funcs: &FuncTable,
) -> Result<Option<ResolvedValue>, Error> {
    // The value must be a single scalar: a backtick body or a call.
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
    // `fn:` uses for `x` ([CHARTS.md] §4), shared via `expr::sample`.
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
/// same engine as a backtick body. Numeric / expr / call args only (geometry).
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

/// A resolved text node (SPEC §3/§10): content carrying the text properties
/// inherited from its container, overlaid with its own `{ }` style (text-valid
/// props only). `Text` is internal — never a user `|type|`, only the kind of a
/// string node (a label, a cell, canvas text). The own style renders as a
/// `style=` on the `<text>`; `attrs` is the effective context for measurement.
fn text_inst(t: &TextNode, ctx: &SceneCtx, text_ctx: &AttrMap) -> Result<ResolvedInst, Error> {
    let mut attrs = AttrMap::new();
    for name in INHERITED_TEXT {
        if let Some(v) = text_ctx.get(name) {
            attrs.insert(*name, v.clone());
        }
    }
    // The text's own `{ }`: text-valid props only — a box property errors and
    // points at `|block|` (SPEC §3, §15).
    let mut own_style = AttrMap::new();
    for d in &t.style {
        if !is_text_prop(&d.name) {
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
        type_chain: Vec::new(),
        applied_styles: Vec::new(),
        label: Some(t.text.clone()),
        attrs,
        own_style,
        markers: Markers::default(),
        children: Vec::new(),
        span: t.span,
    })
}

/// Properties valid on a bare text node (SPEC §3/§10): paint, every `font-*`, the
/// baked spacings, the live-CSS text props, and the two transforms. Anything else
/// (`pin`, `padding`, `width`, a border, `layout`, …) needs a box. Shared with the
/// link-label path ([`super::links`]).
pub(super) fn is_text_prop(name: &str) -> bool {
    matches!(
        name,
        "color"
            | "fill"
            | "opacity"
            | "font-family"
            | "font-size"
            | "font-weight"
            | "font-style"
            | "text-transform"
            | "text-decoration"
            | "text-shadow"
            | "letter-spacing"
            | "line-spacing"
            | "translate"
            | "rotate"
            | "layer"
    )
}

/// An `|icon|` must name a known `symbol` (SPEC §7). Errors point at the node,
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
    let msg = match crate::icon::suggest(symbol).as_slice() {
        [] => format!("unknown icon '{symbol}'"),
        names => {
            let quoted: Vec<String> = names.iter().map(|n| format!("'{n}'")).collect();
            format!(
                "unknown icon '{symbol}'; did you mean {}?",
                quoted.join(", ")
            )
        }
    };
    Err(Error::at(span, msg))
}

/// `fit` (SPEC §10) accepts only the four object-fit keywords — used by `|icon|`
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
/// real cell that holds its track (SPEC §3/§5). A grid is positional, so its
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

fn join_path(prefix: &[String], id: &str) -> String {
    if prefix.is_empty() {
        id.to_string()
    } else {
        format!("{}.{}", prefix.join("."), id)
    }
}

// ─────────────────────────── Path index ───────────────────────────

/// Maps every node's fully-qualified dot-path, for endpoint resolution and
/// auto-create (SPEC §9).
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
