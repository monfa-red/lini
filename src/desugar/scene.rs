//! Auto-create: a root link's single-segment endpoint naming an id declared
//! nowhere becomes an empty `|box|` at the scene root (SPEC §3). This gathers the
//! declared ids and the to-create ids; the caller lowers each created box through
//! the same path as a written one (so it gains its `.lini-box` class and id label).

use crate::span::Span;
use crate::syntax::ast::{Child, Link, Node};
use std::collections::HashSet;

/// Every node id anywhere in `instances` — the auto-create gate: an id present as
/// any node's id, at any depth, is never auto-created. Run on the *lowered*
/// instances, so define-body children (inlined by then) count as declared.
pub fn declared_ids(instances: &[Child]) -> HashSet<String> {
    let mut out = HashSet::new();
    for c in instances {
        collect_ids(c, &mut out);
    }
    out
}

fn collect_ids(child: &Child, out: &mut HashSet<String>) {
    if let Child::Box(n) = child {
        if let Some(id) = &n.id {
            out.insert(id.clone());
        }
        for c in &n.children {
            collect_ids(c, out);
        }
    }
}

/// The ids to auto-create: each single-segment root-link endpoint absent from
/// `declared`, in first-seen order, deduped. Multi-segment paths navigate and
/// never create.
pub fn auto_created_ids(links: &[Link], declared: &HashSet<String>) -> Vec<(String, Span)> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for w in links {
        for group in &w.chain {
            for ep in &group.endpoints {
                if ep.path.len() != 1 {
                    continue; // multi-segment paths navigate, never create
                }
                let id = &ep.path[0];
                if declared.contains(id) || !seen.insert(id.clone()) {
                    continue;
                }
                out.push((id.clone(), ep.span));
            }
        }
    }
    out
}

/// A bare `|box|` for an auto-created endpoint; the caller lowers it (so it gains
/// its `.lini-box` class and id-as-label) exactly like a written box.
pub fn auto_box(id: &str, span: Span) -> Node {
    Node {
        id: Some(id.to_string()),
        ty: Some("box".to_string()),
        classes: Vec::new(),
        style: Vec::new(),
        style_span: None,
        children: Vec::new(),
        links: Vec::new(),
        span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> crate::syntax::ast::File {
        crate::syntax::parser::parse(&crate::lexer::lex(src).expect("lex")).expect("parse")
    }
    fn auto_ids(src: &str) -> Vec<String> {
        let f = parse(src);
        let declared = declared_ids(&f.instances);
        auto_created_ids(&f.links, &declared)
            .into_iter()
            .map(|(s, _)| s)
            .collect()
    }

    #[test]
    fn undeclared_root_link_ids_are_auto_created() {
        assert_eq!(auto_ids("cat -> dog\n"), vec!["cat", "dog"]);
    }

    #[test]
    fn a_declared_id_is_not_auto_created() {
        assert_eq!(auto_ids("cat |box|\ncat -> dog\n"), vec!["dog"]);
    }

    #[test]
    fn a_multi_segment_path_never_creates() {
        // `g.x` navigates into the group; only the single-segment, undeclared `y`
        // is created.
        assert_eq!(auto_ids("g |group| [ x |box| ]\ng.x -> y\n"), vec!["y"]);
    }
}
