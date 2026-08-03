//! Capsule hoisting [SPEC 9/19]: an endpoint capsule (`a -> |cyl#db|`)
//! declares and links in one statement — desugar hoists the declaration to the
//! statement's position in its scope and rewrites the endpoint to the id, so
//! resolve only ever sees plain references. Anonymous capsules mint reserved
//! `lini-cap-N` ids — 1-based in statement order, skipping taken names, so a
//! re-desugared scope gaining a new capsule never collides. A drawing scope
//! rejects capsules — it never invents an endpoint [SPEC 15/21].

use crate::error::{Code, Error};
use crate::syntax::ast::{Link, Node};
use std::collections::HashSet;

/// A declaration hoisted out of a capsule endpoint. The caller stamps
/// `minted_id` onto `node` and lowers it through the one node path — riding the
/// generated id **around** that call ([`super::gather`]), so the reserved-prefix
/// check stays honest for authored `lini-` ids.
pub(super) struct Hoisted {
    pub node: Node,
    pub minted_id: Option<String>,
}

/// Hoist every capsule endpoint out of a scope's links [SPEC 9]: rewrite each
/// to its (minted) id — keeping the type name as the resolve error hint — and
/// return the declarations in statement order, span-seated at their capsule.
/// `declared` seeds the taken-name set the mint skips. An id'd capsule hoists
/// as written; declaring it twice is the ordinary duplicate-id error at
/// resolve. Idempotent: a lowered scope carries no capsules, so re-desugar
/// hoists nothing.
pub(super) fn hoist(
    links: &mut [Link],
    declared: &HashSet<String>,
    in_drawing: bool,
) -> Result<Vec<Hoisted>, Error> {
    let mut taken: HashSet<String> = declared.clone();
    let mut next = 1usize;
    let mut out = Vec::new();
    for w in links.iter_mut() {
        for ep in w.chain.iter_mut().flat_map(|g| &mut g.endpoints) {
            let Some(c) = ep.capsule.take() else { continue };
            if in_drawing {
                return Err(Error::at(
                    c.span,
                    "a drawing never invents an endpoint — declare the node, then annotate it",
                )
                .code(Code::CAPSULE_IN_DRAWING));
            }
            let minted_id = match &c.id {
                Some(_) => None,
                None => {
                    let mut id = format!("lini-cap-{next}");
                    while taken.contains(&id) {
                        next += 1;
                        id = format!("lini-cap-{next}");
                    }
                    next += 1;
                    Some(id)
                }
            };
            let id = c.id.clone().or_else(|| minted_id.clone()).expect("an id");
            taken.insert(id.clone());
            ep.from_capsule = Some(c.ty.clone().unwrap_or_else(|| "box".to_string()));
            ep.path.insert(0, id);
            out.push(Hoisted {
                node: Node {
                    // A minted id is stamped by the caller post-lowering; an
                    // authored one rides the raw node (an authored `lini-`
                    // prefix must still hit the reserved-id check).
                    id: c.id,
                    ty: c.ty,
                    label: None,
                    classes: Vec::new(),
                    style: Vec::new(),
                    style_span: None,
                    children: Vec::new(),
                    links: Vec::new(),
                    span: c.span,
                },
                minted_id,
            });
        }
    }
    Ok(out)
}
