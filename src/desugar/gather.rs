//! The per-scope **statement gather** [SPEC 19]: everything a scope's body must
//! settle *before* any of it lowers, in one place.
//!
//! **The step order, once: hoist, mint, land, pose, lower.** Each step needs
//! every earlier one finished, and the chain of needs is the whole argument:
//!
//! - a scope's children arrive from two sources (the `define` bodies in its
//!   type chain, then its own) and two of its statements *declare* — a capsule
//!   endpoint **hoists** its part ([`super::capsule`]) and a label wire
//!   **mints** its tag ([`super::labelwire`]); hoisting leads so that a wire's
//!   tag terminator is one lookup whichever spelling wrote it;
//! - the landings ([`super::schematic::arity`]) resolve against that finished
//!   child list, and must precede the lowering that would `split_chain` the
//!   statement a pass-through is defined over;
//! - [`super::autopose::choose`] decides off the finished children *and* the
//!   rewritten wires — a satellite it cannot see is one that never turns, and
//!   a landing it cannot see is one turned the wrong way;
//! - lowering is last because a pose is structural: the lowering it precedes
//!   is what applies it.
//!
//! The gather owns one wrinkle: a minted id is stamped **before** the pose (the
//! chooser matches a rewritten endpoint against it) but must not reach
//! [`super::lower_node`]'s reserved-`lini-` gate, so it rides around that call.

use super::{Lower, Nest, autopose, capsule, labelwire, lower_child, lower_node, scene, schematic};
use crate::error::Error;
use crate::syntax::ast::{Child, Link};

/// One scope's statements, gathered and settled: its children (declared,
/// define-contributed, and hoisted alike) and its links, rewritten.
pub(super) struct Scope {
    pub kids: Vec<Child>,
    pub links: Vec<Link>,
    /// Where the scope's **own** statements begin in `links` — the define
    /// bodies in its type chain contribute the ones before it. Produced by the
    /// gather rather than measured by the caller: the landing step below cuts
    /// a statement into hops, so an index taken before it no longer points at
    /// what it named ([`Scope::own_links`]).
    own_at: usize,
    /// Which of `kids` carry a **compiler-minted** id (`lini-label-N`,
    /// `lini-cap-N`) — the ids [`lower_minted`] rides around the lowering.
    minted: Vec<bool>,
    nest: Nest,
    /// Whether **this very container** is the schematic — the pose chooser's
    /// question, and the one schematic reading that must *not* reach: placement
    /// does not cascade [SPEC 16], so a nested `|row|` inside a sheet poses
    /// nothing even though `nest.schematic` reinterprets its statements.
    poses: bool,
}

impl Scope {
    /// Gather `kids` + `links` through the module doc's first three steps —
    /// hoist, mint (anywhere the schematic laws reach, `nest.schematic`), land
    /// — each declaring step appending to the child list in statement order.
    pub(super) fn gather(
        cx: &Lower,
        mut kids: Vec<Child>,
        mut links: Vec<Link>,
        own_at: usize,
        nest: Nest,
        poses: bool,
    ) -> Result<Scope, Error> {
        let base = kids.len();
        let mut taken = scene::declared_ids(&kids);
        // Whether each **appended** declaration's id is the compiler's own.
        let mut generated: Vec<bool> = Vec::new();
        // Step 1 — capsules hoist, into `kids`, so the mint below sees an
        // inline part as an ordinary declared child [SPEC 16.5].
        for h in capsule::hoist(&mut links, &taken, nest.drawing)? {
            let mut node = h.node;
            let minted = h.minted_id.is_some();
            if let Some(id) = h.minted_id {
                node.id = Some(id);
            }
            taken.extend(node.id.clone());
            kids.push(Child::Box(node));
            generated.push(minted);
        }
        // Step 2 — the label wires mint their tags; a name on a pin another
        // statement wires mints none and waits for that statement's hops.
        let mut absorbed = Vec::new();
        let mut own_at = own_at;
        if nest.schematic {
            let (nodes, names) = labelwire::mint(cx, &mut kids, &mut links, &taken, &mut own_at)?;
            for node in nodes {
                kids.push(Child::Box(node));
                generated.push(true);
            }
            absorbed = names;
        }
        // Statement order, which for span-seated declarations is span order —
        // the order `lini desugar` prints them in. Every generated declaration
        // is **merged in at its own statement's position**, not appended: the
        // printer emits a body in span order, so an appended one would print
        // ahead of an authored node declared after its wire and the two
        // programs would differ by that much [SPEC 19]. A tie keeps the
        // authored child first.
        let mut tail: Vec<(Child, bool)> =
            kids.split_off(base).into_iter().zip(generated).collect();
        tail.sort_by_key(|(c, _)| c.span().start);
        let mut minted = Vec::with_capacity(kids.len() + tail.len());
        let mut merged = Vec::with_capacity(kids.len() + tail.len());
        let mut pending = tail.into_iter().peekable();
        for child in kids {
            while pending
                .peek()
                .is_some_and(|(t, _)| t.span().start < child.span().start)
            {
                let (t, g) = pending.next().expect("peeked");
                merged.push(t);
                minted.push(g);
            }
            merged.push(child);
            minted.push(false);
        }
        for (t, g) in pending {
            merged.push(t);
            minted.push(g);
        }
        let kids = merged;
        // Step 3 — with every part and wire of the scope in hand, its
        // **landings** resolve [SPEC 16.5]: a pinless wire takes a pin, a chain
        // threading a two-pin part states its two hops.
        if nest.schematic {
            (links, own_at) = schematic::arity::land(
                cx,
                &kids,
                links,
                &mut schematic::arity::Wired::default(),
                own_at,
            )?;
            // Step 4 — the absorbed names ride the hops that touch their pins.
            labelwire::attach(&mut links, absorbed);
        }
        Ok(Scope {
            kids,
            links,
            own_at,
            minted,
            nest,
            poses,
        })
    }

    /// The scope's **own** statements — what its body wrote, past the ones its
    /// define bodies contributed [SPEC 3]: the slice auto-create reads, since a
    /// define's ids are the define's own affair.
    pub(super) fn own_links(&self) -> &[Link] {
        &self.links[self.own_at..]
    }

    /// Steps 4 and 5 — pose the scope's satellites [SPEC 16.1], then lower
    /// every child through the one node path.
    pub(super) fn lower(&self, cx: &Lower) -> Result<Vec<Child>, Error> {
        let posed = autopose::choose(cx, &self.kids, &self.links, self.poses);
        let mut out = Vec::with_capacity(posed.len());
        for (i, child) in posed.iter().enumerate() {
            out.push(if self.minted[i] {
                lower_minted(cx, child, self.nest)?
            } else {
                lower_child(cx, child, self.nest)?
            });
        }
        Ok(out)
    }
}

/// Lower a child whose id the compiler minted: [`lower_node`]'s reserved-`lini-`
/// gate is for **authored** ids [SPEC 21/23], so the minted one is lifted off,
/// the bare node lowered, and the id stamped back.
fn lower_minted(cx: &Lower, child: &Child, nest: Nest) -> Result<Child, Error> {
    let Child::Box(node) = child else {
        return lower_child(cx, child, nest);
    };
    let mut bare = node.clone();
    let id = bare.id.take();
    let mut lowered = lower_node(cx, &bare, nest)?;
    lowered.id = id;
    Ok(Child::Box(lowered))
}
