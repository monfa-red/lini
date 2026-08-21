//! Auto-create: a root link's single-segment endpoint naming an id declared
//! nowhere becomes an empty `|box|` at the scene root [SPEC 3]. This gathers the
//! declared ids and the to-create ids; the caller lowers each created box through
//! the same path as a written one (so it gains its `.lini-box` class and id label).

use crate::error::{Code, Error};
use crate::span::Span;
use crate::syntax::ast::{Child, Link, Node, TextNode};
use std::collections::HashSet;

/// The ids declared **directly** in a scope (its own children) — the auto-create
/// gate [SPEC 3, 9]: a single bare endpoint not among them is created in that
/// scope. Scope-local, not recursive — a deeper same-named node does not suppress
/// the create; it instead raises a shadow warning (see [`crate::lint`]) — except
/// through **anonymous** containers, which are scope-transparent [SPEC 9]: their
/// children are this scope's, so a bare endpoint reaches them instead of minting
/// a duplicate.
pub fn declared_ids(children: &[Child]) -> HashSet<String> {
    let mut out = HashSet::new();
    collect_ids(children, &mut out);
    out
}

fn collect_ids(children: &[Child], out: &mut HashSet<String>) {
    for c in children {
        if let Child::Box(n) = c {
            match &n.id {
                Some(id) => {
                    out.insert(id.clone());
                }
                None => collect_ids(&n.children, out),
            }
        }
    }
}

/// The ids to auto-create: each single-segment link endpoint absent from `declared`, in
/// first-seen order, deduped. Multi-segment paths navigate and never create. Takes links by
/// reference so a scope can pool its own with messages gathered from its frames [SPEC 13].
/// A **capsule** endpoint never auto-creates — it *declares* [SPEC 9] — and its id counts
/// as declared here, so the pre-hoist view (the lint's) matches the real lowering.
///
/// The pure query — what a scope *would* create. The lint asks it, to name the
/// ids it must check for shadowing; the lowering asks [`to_create`], which is
/// this plus the scope's own answer about whether creating is allowed at all.
pub fn auto_created_ids(links: &[&Link], declared: &HashSet<String>) -> Vec<(String, Span)> {
    let mut capsule_ids = HashSet::new();
    for w in links {
        for ep in w.chain.iter().flat_map(|g| &g.endpoints) {
            if let Some(c) = &ep.capsule
                && let Some(id) = &c.id
            {
                capsule_ids.insert(id.clone());
            }
        }
    }
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for w in links {
        for group in &w.chain {
            for ep in &group.endpoints {
                if ep.capsule.is_some() || ep.path.len() != 1 {
                    continue; // capsules declare; multi-segment paths navigate
                }
                let id = &ep.path[0];
                if declared.contains(id) || capsule_ids.contains(id) || !seen.insert(id.clone()) {
                    continue;
                }
                out.push((id.clone(), ep.span));
            }
        }
    }
    out
}

/// What this scope actually creates. `schematic` is the scope's carrier reading
/// ([`super::Nest`]): **a schematic never invents a box** [SPEC 16.5] — a bare
/// unknown id there is a typo or a net name, so the first would-be creation is
/// an error naming the quoted form it most likely meant. The refusal lives here,
/// beside the creation it refuses, so the root walk and every body ask it once.
///
/// `minted` is what the scope's ref pass just stamped on its anonymous parts
/// ([`super::schematic::mint_refs`]): a display ref is **not** an id [SPEC
/// 16.2], so wiring one lands here — and says so, rather than reading as a
/// stray net name.
pub fn to_create(
    links: &[&Link],
    declared: &HashSet<String>,
    schematic: bool,
    minted: &HashSet<String>,
) -> Result<Vec<(String, Span)>, Error> {
    let out = auto_created_ids(links, declared);
    match (schematic, out.first()) {
        (true, Some((id, span))) if minted.contains(id) => Err(Error::at(
            *span,
            format!(
                "link endpoint '{id}' not found — a minted ref is display-only; \
                 give the part an id to wire it"
            ),
        )
        .code(Code::UNKNOWN_ENDPOINT)),
        (true, Some((id, span))) => Err(Error::at(
            *span,
            format!(
                "'{id}' is unknown — a schematic never invents a box; \
                 did you mean '- \"{id}\"' (a net label)?"
            ),
        )
        .code(Code::SCHEMATIC_INVENT)),
        _ => Ok(out),
    }
}

/// A labelled `|box#id| "id"` for an auto-created endpoint [SPEC 3]; the caller
/// lowers it (so it gains its `.lini-box` class and its centred text label)
/// exactly like a written box.
pub fn auto_box(id: &str, span: Span) -> Node {
    let mut n = super::synth::labelled(
        "box",
        TextNode {
            text: id.to_string(),
            classes: Vec::new(),
            style: Vec::new(),
            style_span: None,
            span,
        },
    );
    n.id = Some(id.to_string());
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> crate::syntax::ast::File {
        crate::syntax::parser::parse(src, &crate::lexer::lex(src).expect("lex")).expect("parse")
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
