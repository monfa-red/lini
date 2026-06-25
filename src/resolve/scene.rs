//! Scene-tree resolution (SPEC §3–§8). Each instance node becomes a
//! [`ResolvedInst`]: its type cascade resolved, descendant/class/block layers
//! applied, caption/label sugar expanded into `|caption|`/`|text|` children,
//! text properties inherited, define bodies materialised (with scoped ids), and
//! internal links lifted out for the link pass.

use super::cascade::{NodeFacts, Stylesheet};
use super::ir::{AttrMap, MarkerKind, Markers, NodeKind, ResolvedInst, ResolvedValue, VarTable};
use super::merge::{collapse, resolve_markers};
use super::value::resolve_groups;
use crate::error::Error;
use crate::span::Span;
use crate::syntax::ast::{Child, Link, Node, TextNode};
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

    if let Some(id) = &node.id {
        if is_reserved_id(id) {
            return Err(reserved_error(node.span, id));
        }
        let full = join_path(path_prefix, id);
        if let Some(prev) = id_seen.get(&full) {
            return Err(Error::at(node.span, format!("duplicate id '{}'", id)).with_related(*prev));
        }
        id_seen.insert(full, node.span);
    }

    let facts = NodeFacts {
        classes: node.classes.clone(),
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
        ordered.push((d.name.clone(), resolve_groups(&d.groups, d.span, ctx.vars)?));
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

    // An `|icon|` is named by its `symbol` (SPEC §7), not its label; any bare
    // string it carries rides along as centred text (`own_label`, drawn by the
    // renderer). It stays a leaf, so its children are not resolved as a subtree.
    let is_icon = kind == NodeKind::Icon;
    if is_icon {
        validate_icon(&attrs, node.span)?;
    } else if kind == NodeKind::Image {
        validate_fit(&attrs, node.span)?;
    }
    let own_label = if is_icon {
        first_text(&node.children).map(str::to_string)
    } else {
        None
    };

    ancestors.push(facts);
    let mut children = Vec::new();
    if !is_icon {
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
    }
    ancestors.pop();
    drop_blank_text(&mut children, &attrs);

    Ok(ResolvedInst {
        id: node.id.clone(),
        kind,
        type_chain,
        applied_styles,
        label: own_label,
        attrs,
        own_style: AttrMap::new(),
        markers,
        children,
        span: node.span,
    })
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
        let v = resolve_groups(&d.groups, d.span, ctx.vars)?;
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

/// The first bare string among a node's children — an `|icon|`'s optional
/// centred text.
fn first_text(children: &[Child]) -> Option<&str> {
    children.iter().find_map(|c| match c {
        Child::Text(t) => Some(t.text.as_str()),
        Child::Box(_) => None,
    })
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

/// Only the four sides are reserved as node ids — they are peeled from endpoint
/// paths (`a.left`), so a node named `left` could never be addressed (SPEC §18).
/// Type names are free: a type only ever appears in bars.
fn is_reserved_id(id: &str) -> bool {
    matches!(id, "top" | "bottom" | "left" | "right")
}

/// The reserved-id error, with the always-free capitalized variant as the out.
pub(super) fn reserved_error(span: Span, name: &str) -> Error {
    let mut cap = name.to_string();
    if let Some(first) = cap.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    Error::at(
        span,
        format!(
            "'{}' is reserved (an endpoint side; ids are case-sensitive — '{}' is free)",
            name, cap
        ),
    )
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
