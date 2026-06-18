//! Scene-tree resolution (SPEC §3–§8). Each instance node becomes a
//! [`ResolvedInst`]: its type cascade resolved, descendant/class/block layers
//! applied, caption/label sugar expanded into `|caption|`/`|text|` children,
//! text properties inherited, define bodies materialised (with scoped ids), and
//! internal wires lifted out for the wire pass.

use super::cascade::{NodeFacts, Stylesheet};
use super::ir::{AttrMap, MarkerKind, ResolvedInst, ResolvedValue, ShapeKind, VarTable};
use super::merge::{collapse, resolve_markers};
use super::types::Types;
use super::value::resolve_groups;
use crate::error::Error;
use crate::span::Span;
use crate::syntax::ast::{Block, Decl, Node, Value, Wire};
use std::collections::HashMap;

/// Text properties that cascade to descendant text (SPEC §10): nearest ancestor
/// wins, a node's own value beats inherited.
pub(super) const INHERITED_TEXT: &[&str] = &[
    "font-family",
    "font-size",
    "font-weight",
    "font-style",
    "text-align",
    "line-height",
    "letter-spacing",
    "color",
];

/// An internal wire lifted from a body or define, with its host's dot-path
/// prefix — resolved at program level once the whole tree (and its path index)
/// exists.
pub struct LiftedWire {
    pub wire: Wire,
    pub prefix: Vec<String>,
}

/// Everything node resolution reads but does not mutate.
pub struct SceneCtx<'a> {
    pub types: &'a Types<'a>,
    pub sheet: &'a Stylesheet,
    pub vars: &'a VarTable,
}

/// Resolve the top-level instances into scene nodes, collecting lifted internal
/// wires. `text_ctx` seeds the inheritable text properties from the root config.
pub fn resolve_instances(
    instances: &[Node],
    ctx: &SceneCtx,
    text_ctx: &AttrMap,
    id_seen: &mut HashMap<String, Span>,
    lifted: &mut Vec<LiftedWire>,
) -> Result<Vec<ResolvedInst>, Error> {
    let mut ancestors = Vec::new();
    let mut nodes = Vec::with_capacity(instances.len());
    for node in instances {
        nodes.push(resolve_node(
            node, ctx, &mut ancestors, &[], text_ctx, id_seen, lifted,
        )?);
    }
    nodes.retain(|n| !is_blank_anon_text(n));
    Ok(nodes)
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
    lifted: &mut Vec<LiftedWire>,
) -> Result<ResolvedInst, Error> {
    let type_name = node.ty.as_deref().unwrap_or("box");
    let rt = ctx.types.resolve(type_name, node.span)?;

    for class in &node.classes {
        if !ctx.sheet.defines_class(class) {
            return Err(Error::at(node.span, format!("unknown class '.{}'", class)));
        }
    }

    if let Some(id) = &node.id {
        if is_reserved_id(id, ctx.types) {
            return Err(reserved_error(node.span, id));
        }
        let full = join_path(path_prefix, id);
        if let Some(prev) = id_seen.get(&full) {
            return Err(
                Error::at(node.span, format!("duplicate id '{}'", id)).with_related(*prev)
            );
        }
        id_seen.insert(full, node.span);
    }

    // Matcher identity: every type name in the chain plus the primitive, and the
    // applied classes.
    let mut facts_types = rt.type_chain.clone();
    facts_types.push(rt.kind.as_str().to_string());
    let facts = NodeFacts {
        types: facts_types,
        classes: node.classes.clone(),
    };

    // The cascade ladder, least-specific first (SPEC §12): type defaults, then
    // descendant + class layers, then the instance's own block.
    let mut ordered: Vec<(String, ResolvedValue)> = rt.defaults.clone();
    ordered.extend(ctx.sheet.node_layers(ancestors, &facts));
    if let Some(block) = &node.block {
        for d in &block.decls {
            ordered.push((d.name.clone(), resolve_groups(&d.groups, d.span, ctx.vars)?));
        }
    }

    let markers = resolve_markers(&ordered, MarkerKind::None, MarkerKind::None, node.span)?;
    let mut attrs = collapse(&ordered);

    let text_like = rt.kind == ShapeKind::Text;
    if text_like {
        if let Some(prop) = ["width", "height"].iter().find(|p| attrs.get(p).is_some()) {
            return Err(Error::at(
                node.span,
                format!("'{}' is not a text property; use 'font-size'", prop),
            ));
        }
        for name in INHERITED_TEXT {
            if attrs.get(name).is_none()
                && let Some(v) = text_ctx.get(name)
            {
                attrs.insert(*name, v.clone());
            }
        }
    }

    if rt.kind == ShapeKind::Slant
        && let Some(skew) = attrs.number("skew")
        && (skew <= -89.0 || skew >= 89.0)
    {
        return Err(Error::at(
            node.span,
            format!("skew: {} must be in (-89, 89)", skew),
        ));
    }

    // Inherited text context for children: overlay this node's text props.
    let mut child_text_ctx = text_ctx.clone();
    for name in INHERITED_TEXT {
        if let Some(v) = attrs.get(name) {
            child_text_ctx.insert(*name, v.clone());
        }
    }

    let is_group = rt.type_chain.iter().any(|t| t == "group");
    // A `|text|` carries its label as content; an `|icon|` carries it as the
    // glyph name (SPEC §7). Both consume their own labels — every other shape
    // stacks them as `|text|` children via label sugar.
    let consumes_label = text_like || rt.kind == ShapeKind::Icon;
    let own_label =
        consumes_label.then(|| (!node.labels.is_empty()).then(|| node.labels.join("\n")))
            .flatten();

    // Body order (SPEC §3): define intrinsic children, then label sugar, then
    // the block's own children.
    let mut child_nodes: Vec<Node> = rt.body_nodes.clone();
    if !consumes_label {
        child_nodes.extend(label_sugar(&node.labels, is_group, node.span));
    }
    if let Some(block) = &node.block {
        child_nodes.extend(block.nodes.iter().cloned());
    }

    let mut child_prefix = path_prefix.to_vec();
    if let Some(id) = &node.id {
        child_prefix.push(id.clone());
    }

    // Internal wires (define body + block) lift to program level, prefixed by
    // this node's path.
    for w in rt.body_wires.iter().chain(node.block.iter().flat_map(|b| &b.wires)) {
        lifted.push(LiftedWire {
            wire: w.clone(),
            prefix: child_prefix.clone(),
        });
    }

    ancestors.push(facts);
    let mut children = Vec::with_capacity(child_nodes.len());
    for child in &child_nodes {
        children.push(resolve_node(
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
    children.retain(|c| !is_blank_anon_text(c));

    Ok(ResolvedInst {
        id: node.id.clone(),
        shape: rt.kind,
        type_chain: rt.type_chain,
        applied_styles: node.classes.clone(),
        label: own_label,
        attrs,
        markers,
        children,
        span: node.span,
    })
}

/// Expand a host's positional labels into synthesized child nodes (SPEC §8): a
/// group's 1st label is a top `|caption|`, its 2nd a bottom `|caption|`, the
/// rest plain centred `|text|`; every other shape stacks all labels as `|text|`.
/// Empty labels resolve to blank text and are dropped downstream.
fn label_sugar(labels: &[String], is_group: bool, span: Span) -> Vec<Node> {
    labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let (ty, footer) = match (is_group, i) {
                (true, 0) => ("caption", false),
                (true, 1) => ("caption", true),
                _ => ("text", false),
            };
            let block = footer.then(|| Block {
                decls: vec![ident_decl("side", "bottom", span)],
                nodes: Vec::new(),
                wires: Vec::new(),
            });
            Node {
                id: None,
                ty: Some(ty.to_string()),
                labels: vec![label.clone()],
                classes: Vec::new(),
                block,
                span,
            }
        })
        .collect()
}

fn ident_decl(name: &str, value: &str, span: Span) -> Decl {
    Decl {
        name: name.to_string(),
        groups: vec![vec![Value::Ident(value.to_string())]],
        span,
    }
}

/// A `|text|` with no visible content and no id — from a `""` label. SPEC §8:
/// `""` suppresses the label, so the node is dropped (a kept empty text would
/// reserve a band / centred slot and emit an empty `<text>`).
fn is_blank_anon_text(r: &ResolvedInst) -> bool {
    r.id.is_none() && r.shape == ShapeKind::Text && r.label.as_deref().is_none_or(str::is_empty)
}

/// Type names (primitives, templates, defines), the four sides, the `wire` rule
/// target, and the reserved-for-future `rect` / `circle` cannot be node ids (SPEC §18).
fn is_reserved_id(id: &str, types: &Types) -> bool {
    types.is_known(id) || matches!(id, "wire" | "rect" | "circle" | "top" | "bottom" | "left" | "right")
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
            "'{}' is reserved (ids are case-sensitive — '{}' is free)",
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

    /// An endpoint is an exact path from the wire's scope (the caller prepends
    /// the scope prefix). There is no search.
    pub fn resolve(&self, query: &[String]) -> Option<String> {
        let joined = query.join(".");
        self.contains(&joined).then_some(joined)
    }

    /// Whether any node anywhere carries this final id — the auto-create gate
    /// (only ids absent everywhere materialize).
    pub fn has_final_segment(&self, seg: &str) -> bool {
        self.paths.iter().any(|p| final_segment(p) == seg)
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
