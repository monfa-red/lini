//! Auto-create: a root link's single-segment endpoint naming an id declared
//! nowhere becomes an empty `|box|` at the scene root [SPEC 3]. This gathers the
//! declared ids and the to-create ids; the caller lowers each created box through
//! the same path as a written one (so it gains its `.lini-box` class and id label).

use crate::span::Span;
use crate::syntax::ast::{Child, Link, Node, TextNode};
use std::collections::HashSet;

/// The ids declared **directly** in a scope (its own children) — the auto-create
/// gate [SPEC 3, 9]: a single bare endpoint not among them is created in that
/// scope. Scope-local, not recursive — a deeper same-named node does not suppress
/// the create; it instead raises a shadow warning (see [`crate::lint`]).
pub fn declared_ids(children: &[Child]) -> HashSet<String> {
    children
        .iter()
        .filter_map(|c| match c {
            Child::Box(n) => n.id.clone(),
            Child::Text(_) => None,
        })
        .collect()
}

/// The ids to auto-create: each single-segment link endpoint absent from `declared`, in
/// first-seen order, deduped. Multi-segment paths navigate and never create. Takes links by
/// reference so a scope can pool its own with messages gathered from its frames [SPEC 13].
pub fn auto_created_ids(links: &[&Link], declared: &HashSet<String>) -> Vec<(String, Span)> {
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

/// A labelled `|box#id| "id"` for an auto-created endpoint [SPEC 3]; the caller
/// lowers it (so it gains its `.lini-box` class and its centred text label)
/// exactly like a written box.
pub fn auto_box(id: &str, span: Span) -> Node {
    Node {
        id: Some(id.to_string()),
        ty: Some("box".to_string()),
        label: Some(TextNode {
            text: id.to_string(),
            style: Vec::new(),
            style_span: None,
            span,
        }),
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
        let links: Vec<&Link> = f.links.iter().collect();
        auto_created_ids(&links, &declared)
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
        assert_eq!(auto_ids("|box#cat|\ncat -> dog\n"), vec!["dog"]);
    }

    #[test]
    fn a_multi_segment_path_never_creates() {
        // `g.x` navigates into the group; only the single-segment, undeclared `y`
        // is created.
        assert_eq!(auto_ids("|group#g| [ |box#x| ]\ng.x -> y\n"), vec!["y"]);
    }
}
