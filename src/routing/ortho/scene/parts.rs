//! How a **schematic part** enters the scene [SPEC 16.2/16.5] — the one
//! bridge between the placed sheet and the routing contract.
//!
//! A part is a scene **leaf**, and that is an identity rather than a special
//! case: its pins, stubs, numbers and zero-size port nodes are never obstacles
//! of their own — they *address* the part (`u7.vs` resolves to the
//! component's connection frame), and their landings ride the two tables here.
//!
//! | Table | Question | Asked by |
//! |---|---|---|
//! | [`Parts::ports`] | where does a wire land, on which side? | the request builder's fixed ports (ROUTING.md Fixed ports) |
//! | [`Parts::terminals`] | is this address a terminal at all? | the `:side` gate [SPEC 16.4] |
//!
//! They are deliberately **not** the same set. A terminal owns its connection
//! geometry whether or not the part's drawing hands it a facing, so a
//! symbol-less `|label|` is a terminal with no landing; and a symbol part's
//! own path takes a convenience landing (a bare `- r1` lands on its first pin,
//! as the seat pass reads it) without being a terminal — SPEC 16.5's pinless
//! landing resolves to a *pin*, which is Phase 5's arity work.
//!
//! Every landing is computed **once**, when the part folds, and stored as a
//! point: `fixed_port` only ever selects one of its components, so two wires
//! on a pin carry the identical `f64` and ROUTING.md's implicit fan merges
//! bit-exactly.
//!
//! The tables say what a part **offers**, and that is the whole answer: a
//! schematic type may only exist inside a schematic scope [SPEC 16/21] — the
//! layout gate refuses it anywhere else — so an address in these tables is by
//! construction an address in the scope, and neither table needs a scope test
//! of its own. That is also what makes the sheet's law reach a wire written
//! **outside** it: a root wire to `s.u1.a` finds the pin here and lands on it,
//! because a pin is a pin whoever wires it.

use super::{SceneIndex, abs_rect, inside};
use crate::ast::Side;
use crate::layout::ir::PlacedNode;
use crate::layout::schematic::PartPorts;
use std::collections::{BTreeMap, BTreeSet};

/// The sheet's answers, by endpoint path. A port's `bool` marks a net
/// **run**'s — a through point ([`SceneIndex::through_port`]).
#[derive(Default)]
pub(super) struct Parts {
    ports: BTreeMap<String, (Side, (f64, f64), bool)>,
    terminals: BTreeSet<String>,
}

impl SceneIndex {
    /// Register a part's landings and fold its anatomy into scene node `i`.
    pub(super) fn fold_part(
        &mut self,
        part: PartPorts,
        n: &PlacedNode,
        path: &str,
        i: usize,
        cx: f64,
        cy: f64,
    ) {
        let key = |id: Option<String>| match id {
            Some(id) => format!("{path}.{id}"),
            None => path.to_owned(),
        };
        let through = crate::layout::schematic::is_net_run(n);
        for (id, side, at) in part.ports {
            self.parts
                .ports
                .insert(key(id), (side, (at.0 + cx, at.1 + cy), through));
        }
        for id in part.terminals {
            self.parts.terminals.insert(key(id));
        }
        self.fold(n, path, i, cx, cy);
    }

    /// Fold a part's anatomy into its one obstacle [SPEC 16.2]: every
    /// descendant address (`u7.vs`, `c24.p1`) resolves to the part itself, and
    /// no pin, stub, number or port node is ever a scene node of its own. Ink
    /// outside the connection frame — the ref / value readouts — stays an
    /// obstacle through the part's `overflow`, on the same terms the generic
    /// walk collects any node's poking descendants. **Pin anatomy never
    /// overflows**: the frame's side runs through the stub's tip, so the only
    /// ink a stub or its number can poke past it is stroke-cap dust — and the
    /// wire lands on (and covers) the lead by construction.
    ///
    /// **A net run's name never overflows either** [SPEC 16.4], for the reason
    /// SPEC 9 gives every label: it is text beside a line, an obstacle to
    /// nothing. Counting it would wall off the very trace it names — a name
    /// standing its clear space off one wire sits well inside the next wire's
    /// keep-out, and every second net would stray.
    fn fold(&mut self, n: &PlacedNode, path: &str, i: usize, ox: f64, oy: f64) {
        let frame = self.nodes[i].rect;
        let own = abs_rect(n, ox, oy);
        let named = crate::layout::schematic::is_net_run(n);
        for c in &n.children {
            let (cx, cy) = (ox + c.cx, oy + c.cy);
            let rect = abs_rect(c, cx, cy);
            let cpath = match &c.id {
                Some(id) if path.is_empty() => id.clone(),
                Some(id) => format!("{path}.{id}"),
                None => path.to_owned(),
            };
            if c.id.is_some() {
                self.by_path.insert(cpath.clone(), i);
            }
            let pin_anatomy = c
                .type_chain
                .iter()
                .any(|t| t == "pin-stub" || t == "pin-number");
            if !pin_anatomy && !named && !inside(frame, rect) && !inside(own, rect) {
                self.nodes[i].overflow.push(rect);
            }
            self.fold(c, &cpath, i, cx, cy);
        }
    }

    /// A landing's fixed port (ROUTING.md Fixed ports): the forced side and
    /// the exact **ordinate** on it — `y` on a vertical side, `x` on a
    /// horizontal one. A selection of the stored point, never a recomputation.
    pub(crate) fn fixed_port(&self, path: &str) -> Option<(Side, f64)> {
        self.parts.ports.get(path).map(|&(side, at, _)| {
            (
                side,
                match side {
                    Side::Left | Side::Right => at.1,
                    Side::Top | Side::Bottom => at.0,
                },
            )
        })
    }

    /// A net **run**'s landing is a through point [SPEC 16.4] — the run is a
    /// stretch of wire, not a stop — so its port rides the run's axis and
    /// either axis side is an honest landing. `Some(point)` for a run's port.
    pub(crate) fn through_port(&self, path: &str) -> Option<(f64, f64)> {
        self.parts
            .ports
            .get(path)
            .and_then(|&(_, at, through)| through.then_some(at))
    }

    /// Whether this endpoint is a schematic **terminal** — a part's pin, or a
    /// `|label|`, which is its own [SPEC 16.4]. A terminal owns its connection
    /// geometry, so `:side` on one is an error; whether the part's drawing
    /// gives it a landing is a separate question.
    pub(crate) fn is_terminal(&self, path: &str) -> bool {
        self.parts.terminals.contains(path)
    }
}
