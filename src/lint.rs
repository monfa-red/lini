//! Lint pass — stylistic / advisory warnings that are not parse/resolve errors
//! [SPEC 20]. It runs on the parsed file and reuses the desugar auto-create gate
//! so its view of what will be created matches the real lowering.
//!
//! Three warnings live here:
//! - **link labels split** — a link carries a head label *and* a `[ ]` of labels;
//!   they read better kept together.
//! - **auto-create shadows a node** — a bare link endpoint is auto-created in its
//!   scope while a same-named node already exists elsewhere in the tree.
//! - **`pin:` on a mated child** — a mate seats the part, so its `pin:` is
//!   ignored [SPEC 15.5].

use crate::ast::ChainOp;
use crate::desugar::scene::{auto_created_ids, declared_ids};
use crate::error::Diagnostic;
use crate::syntax::ast::{Child, File, Link};
use std::collections::HashMap;

pub fn lint(file: &File) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    lint_split_labels(file, &mut out);
    lint_auto_create_shadows(file, &mut out);
    lint_pinned_mates(file, &mut out);
    out
}

/// A `pin:` on a mated child is ignored — the mate seats the part [SPEC 15.5].
/// Scope-local, like the placement itself: each mate's endpoints are matched
/// against the sibling nodes of the body the mate is written in.
fn lint_pinned_mates(file: &File, out: &mut Vec<Diagnostic>) {
    fn scan(children: &[Child], links: &[Link], out: &mut Vec<Diagnostic>) {
        let pinned: Vec<&str> = children
            .iter()
            .filter_map(|c| match c {
                Child::Box(n) if n.style.iter().any(|d| d.name == "pin") && n.id.is_some() => {
                    n.id.as_deref()
                }
                _ => None,
            })
            .collect();
        for w in links.iter().filter(|w| matches!(w.op, ChainOp::Mate)) {
            for ep in w.chain.iter().flat_map(|g| &g.endpoints) {
                if let [first, ..] = ep.path.as_slice()
                    && pinned.contains(&first.as_str())
                {
                    out.push(Diagnostic::warn(
                        w.span,
                        format!("'pin' on '{first}' is ignored — the mate seats it"),
                    ));
                }
            }
        }
        for c in children {
            if let Child::Box(n) = c {
                scan(&n.children, &n.links, out);
            }
        }
    }
    scan(&file.instances, &file.links, out);
}

/// A link with both a head label and a `[ ]` of labels [SPEC 20]: keep them
/// together in the `[ ]`.
fn lint_split_labels(file: &File, out: &mut Vec<Diagnostic>) {
    let mut visit = |w: &Link| {
        if w.label.is_some() && !w.labels.is_empty() {
            out.push(Diagnostic::warn(
                w.span,
                "keep a link's labels together — write 'a -> b [ \"x\" \"y\" ]'",
            ));
        }
    };
    for_each_link(file, &mut visit);
}

/// A bare endpoint auto-created in its scope while a same-named node exists
/// elsewhere in the tree [SPEC 3/19]: the box is still made here, with a
/// warning that names the other match.
fn lint_auto_create_shadows(file: &File, out: &mut Vec<Diagnostic>) {
    let mut id_paths: HashMap<String, Vec<String>> = HashMap::new();
    for c in &file.instances {
        collect_paths(c, &mut Vec::new(), &mut id_paths);
    }
    let root_drawing = matches!(root_layout(&file.stylesheet), Some(l) if l == "drawing");
    shadow_scope(
        &file.instances,
        &file.links,
        &[],
        &id_paths,
        root_drawing,
        out,
    );
}

fn shadow_scope(
    children: &[Child],
    links: &[Link],
    prefix: &[String],
    id_paths: &HashMap<String, Vec<String>>,
    scope_is_drawing: bool,
    out: &mut Vec<Diagnostic>,
) {
    // A drawing scope never auto-creates [SPEC 15] — its endpoints point at
    // real (or, in a `|detail|`, re-laid) geometry, so a bare id there is a
    // reference, not an invented box. Skip the shadow check for it.
    if scope_is_drawing {
        for c in children {
            if let Child::Box(n) = c {
                let mut sub = prefix.to_vec();
                if let Some(id) = &n.id {
                    sub.push(id.clone());
                }
                shadow_scope(
                    &n.children,
                    &n.links,
                    &sub,
                    id_paths,
                    is_drawing_node(n),
                    out,
                );
            }
        }
        return;
    }
    let declared = declared_ids(children);
    let link_refs: Vec<&Link> = links.iter().collect();
    for (id, span) in auto_created_ids(&link_refs, &declared) {
        let here = join_path(prefix, &id);
        if let Some(other) = id_paths
            .get(&id)
            .and_then(|paths| paths.iter().find(|p| **p != here))
        {
            let scope = if prefix.is_empty() {
                "scene root".to_string()
            } else {
                prefix.join(".")
            };
            out.push(Diagnostic::warn(
                span,
                format!(
                    "endpoint '{id}' auto-created at {scope} — a node '{id}' also exists at '{other}'"
                ),
            ));
        }
    }
    for c in children {
        if let Child::Box(n) = c {
            let mut sub = prefix.to_vec();
            if let Some(id) = &n.id {
                sub.push(id.clone());
            }
            shadow_scope(
                &n.children,
                &n.links,
                &sub,
                id_paths,
                is_drawing_node(n),
                out,
            );
        }
    }
}

/// Whether a node opens a drawing scope [SPEC 15] — a `|drawing|` / `|detail|`
/// (or a define over one, caught by the type name) or an explicit
/// `layout: drawing`. The lint's twin of desugar's `is_drawing_body`, on the
/// raw AST (no resolved type chain) — so it reads the written type and style.
fn is_drawing_node(n: &crate::syntax::ast::Node) -> bool {
    matches!(n.ty.as_deref(), Some("drawing") | Some("detail"))
        || n.style
            .iter()
            .any(|d| d.name == "layout" && decl_ident(d) == Some("drawing"))
}

fn root_layout(stylesheet: &[crate::syntax::ast::StyleItem]) -> Option<String> {
    stylesheet.iter().find_map(|it| match it {
        crate::syntax::ast::StyleItem::RootDecl(d) if d.name == "layout" => {
            decl_ident(d).map(str::to_string)
        }
        _ => None,
    })
}

fn decl_ident(d: &crate::syntax::ast::Decl) -> Option<&str> {
    match d.groups.first().and_then(|g| g.first()) {
        Some(crate::syntax::ast::Value::Ident(s)) => Some(s),
        _ => None,
    }
}

/// Every node id mapped to its full dot-paths, over the whole parsed tree.
fn collect_paths(child: &Child, stack: &mut Vec<String>, out: &mut HashMap<String, Vec<String>>) {
    if let Child::Box(n) = child {
        if let Some(id) = &n.id {
            stack.push(id.clone());
            out.entry(id.clone()).or_default().push(stack.join("."));
        }
        for c in &n.children {
            collect_paths(c, stack, out);
        }
        if n.id.is_some() {
            stack.pop();
        }
    }
}

fn join_path(prefix: &[String], id: &str) -> String {
    if prefix.is_empty() {
        id.to_string()
    } else {
        format!("{}.{}", prefix.join("."), id)
    }
}

/// Visit every link in the file — the root links and each container body's.
fn for_each_link(file: &File, visit: &mut impl FnMut(&Link)) {
    for w in &file.links {
        visit(w);
    }
    for c in &file.instances {
        for_each_link_child(c, visit);
    }
}

fn for_each_link_child(child: &Child, visit: &mut impl FnMut(&Link)) {
    if let Child::Box(n) = child {
        for w in &n.links {
            visit(w);
        }
        for c in &n.children {
            for_each_link_child(c, visit);
        }
    }
}

#[cfg(test)]
mod tests {
    fn warnings(src: &str) -> Vec<String> {
        crate::lint_str(src)
            .expect("lint")
            .into_iter()
            .map(|d| d.message)
            .collect()
    }

    #[test]
    fn inline_paint_is_not_linted() {
        // Inline paint in an instance block is idiomatic — no warning.
        assert!(warnings("|box#x| { fill: red; stroke: blue; }\n").is_empty());
    }

    #[test]
    fn pin_on_a_mated_child_warns() {
        let w = warnings(
            "{ layout: drawing }\n|rect#a| { width: 20; height: 20 }\n|rect#b| { width: 20; height: 20; pin: center }\na:right || b:left\n",
        );
        assert!(
            w.iter()
                .any(|m| m == "'pin' on 'b' is ignored — the mate seats it"),
            "{w:?}"
        );
        // An unmated pinned node, or a mate without pins, stays silent.
        assert!(
            warnings("{ layout: drawing }\n|rect#a| { width: 20; height: 20 }\n|rect#b| { width: 20; height: 20 }\na:right || b:left\n")
                .is_empty()
        );
    }

    #[test]
    fn a_split_link_label_warns() {
        let w = warnings("a -> b \"x\" [ \"y\" ]\n");
        assert!(
            w.iter()
                .any(|m| m.contains("keep a link's labels together")),
            "{w:?}"
        );
    }

    #[test]
    fn together_link_labels_do_not_warn() {
        assert!(
            warnings("a -> b [ \"x\" \"y\" ]\n")
                .iter()
                .all(|m| !m.contains("together"))
        );
        assert!(
            warnings("a -> b \"x\"\n")
                .iter()
                .all(|m| !m.contains("together"))
        );
    }

    #[test]
    fn auto_create_shadowing_a_deeper_node_warns() {
        // `cat` exists at kitchen.cat; a root link auto-creates a root `cat`.
        let w = warnings("|group#kitchen| [ |box#cat| ]\ncat -> dog\n");
        assert!(
            w.iter().any(|m| m.contains("also exists at 'kitchen.cat'")),
            "{w:?}"
        );
    }

    #[test]
    fn a_clean_auto_create_does_not_warn() {
        assert!(
            warnings("cat -> dog\n")
                .iter()
                .all(|m| !m.contains("also exists"))
        );
    }
}
