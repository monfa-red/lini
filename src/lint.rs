//! Lint pass — stylistic / advisory warnings that are not parse/resolve errors
//! [SPEC 20]. It runs on the parsed file and reuses the desugar auto-create gate
//! so its view of what will be created matches the real lowering.
//!
//! Two warnings live here:
//! - **link labels split** — a link carries a head label *and* a `[ ]` of labels;
//!   they read better kept together.
//! - **auto-create shadows a node** — a bare link endpoint is auto-created in its
//!   scope while a same-named node already exists elsewhere in the tree.

use crate::desugar::scene::{auto_created_ids, declared_ids};
use crate::error::Diagnostic;
use crate::syntax::ast::{Child, File, Link};
use std::collections::HashMap;

pub fn lint(file: &File) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    lint_split_labels(file, &mut out);
    lint_auto_create_shadows(file, &mut out);
    out
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
    shadow_scope(&file.instances, &file.links, &[], &id_paths, out);
}

fn shadow_scope(
    children: &[Child],
    links: &[Link],
    prefix: &[String],
    id_paths: &HashMap<String, Vec<String>>,
    out: &mut Vec<Diagnostic>,
) {
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
            shadow_scope(&n.children, &n.links, &sub, id_paths, out);
        }
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
