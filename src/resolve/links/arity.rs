//! The schematic **wire laws** at resolve [SPEC 16.5]: the landings desugar
//! could not see, and the duplicate a repeated pair is.
//!
//! Desugar resolves every landing a scope can answer for itself — a pinless
//! wire onto a pin, a chain threading a two-pin part into its two hops — and
//! prints them explicitly ([`crate::desugar::schematic::arity`], which argues
//! why it must be that stage). What is left here is what only a resolved path
//! can answer: an endpoint reaching **into another scope** (`x - r.r1`), where
//! the writing scope had no part to read pins off. This pass applies the very
//! same law — [`arity::Wired`], [`arity::choose`], one module, no second copy —
//! and then judges the pairs.
//!
//! One pass over the resolved links, gated by [`reads_laws`], in two readings:
//!
//! 1. **every named pin is a reservation** — including every pin desugar just
//!    named, so this stage never re-spends one;
//! 2. **then every wire lands, in declaration order**, and declares its pair.
//!
//! **A chain that reaches here is still a chain.** Outside the scope desugar
//! states `a - b - c` as `a - b; b - c`, but a schematic chain says something
//! two statements cannot ([SPEC 16.5]'s series circuit), so desugar cuts one
//! only where it *resolved* the pass-through and leaves the rest whole — which
//! is the only way what it could not resolve survives printing. This pass
//! therefore reads a chain **as a chain**, by the same rule desugar reads one
//! (a two-pin part in the middle is entered on one pin and left by the other,
//! [`arity::other`]), and cuts every multi-hop wire into a link per hop, so
//! the router still only ever sees two-ended wires.
//!
//! What one statement can still repeat is an endpoint: an `&` fan expands
//! here, and its siblings share **one written endpoint**, so they share one
//! landing — keyed on that endpoint's own span in [`Landed`], never on the
//! statement's.
//!
//! **The order the three readings run in.** A statement is landed (per written
//! endpoint), cut (per hop), and fanned (per group pair) — and the fan is the
//! outermost of the three, because `&` shares an *end* [SPEC 9]: `a & b - x - c`
//! is `a - x`, `b - x`, `x - c`, not two chains. `resolve_link` expands the fan
//! first, though, so each leg arrives carrying the whole chain and every hop away
//! from the fanned group is read once per leg. [`cut`] restores the order it
//! wanted: a written hop — the two endpoints' own spans, within one statement —
//! is **one** wire however many legs read it, exactly as a written endpoint is one
//! landing. Reconciling the legs is the cut's own job and not the duplicate law's:
//! `Declared` judges the wires a sheet *declares*, and one statement declares each
//! of its hops once.
//!
//! **No wire leaves resolve with more than two ends.** The cut is therefore not
//! gated on the carrier: desugar's cascade slice can say "schematic" (so
//! `split_chain` stood down) where the resolved attrs say otherwise — a
//! `.plain { layout: flow }` worn by a `|schematic|` — and an uncut chain would
//! then reach the router as one many-ended wire, with a statement's labels
//! distributed one per hop instead of riding every hop [SPEC 9]. The two carriers
//! answer *different* questions and may disagree; this one place makes the
//! disagreement cost nothing.
//!
//! The pin order and the pin count come from the family's one table
//! ([`crate::desugar::schematic::terminal_ids`]) — the same walk the router's
//! fixed ports and the engine's role classifier read, so no two stages can
//! disagree about how many pins a part has.
//!
//! Landings are addresses only. What a wire *does* at a shared pin is
//! ROUTING.md's: two ends resolved onto one terminal carry one bit-exact
//! fixed port and merge into the implicit fan (`request::fan_groups`), drawn
//! as one lead until the split — this pass neither draws nor merges anything.

use super::super::ir::{LinkKind, ResolvedInst, ResolvedLink};
use super::super::scene::{rel_path, walk_scope};
use super::Owner;
use crate::desugar::schematic::{PartNode, arity, sch_kind, schematic_type, terminal_ids};
use crate::error::{Code, Error};
use crate::span::Span;

/// The resolved tree's adapter onto the one pin walk — the placed tree wears
/// the twin of it in [`crate::layout::schematic`].
impl PartNode for ResolvedInst {
    fn type_chain(&self) -> &[String] {
        &self.type_chain
    }
    fn attrs(&self) -> &super::super::ir::AttrMap {
        &self.attrs
    }
    fn node_id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn kids(&self) -> &[Self] {
        &self.children
    }
}

/// Apply the wire laws to the links a schematic scope owns, in place.
/// `owner[i]` is the scope walk's answer for `links[i]`; every other link is
/// left exactly as it resolved.
pub(crate) fn wire_laws(
    links: Vec<ResolvedLink>,
    owner: &[Owner],
    nodes: &[ResolvedInst],
) -> Result<Vec<ResolvedLink>, Error> {
    // Reading 1 — every pin an endpoint names, in any of the scope's
    // statements, is spoken for before any pinless landing chooses.
    let mut wired = arity::Wired::default();
    for (w, own) in links.iter().zip(owner) {
        if !reads_laws(w, *own, nodes) {
            continue;
        }
        for e in &w.endpoints {
            if let Some(a) = addressed(nodes, &e.path)
                && let Some(pin) = a.named
            {
                wired.take(&a.part, &pin);
            }
        }
    }
    // Reading 2 — land each wire, cut what it threads, judge the pairs.
    let mut landed = Landed::default();
    let mut drawn = Drawn::default();
    let mut declared = Declared::default();
    let mut out = Vec::with_capacity(links.len());
    for (w, own) in links.into_iter().zip(owner) {
        if !reads_laws(&w, *own, nodes) {
            // Not a statement of the sheet's — but the cut is the one place
            // that guarantees a two-ended wire, so it still runs (it is a
            // no-op for everything `split_chain` already stated as hops).
            out.extend(cut(w, &[], &mut drawn));
            continue;
        }
        let (w, exits) = land(w, nodes, &mut wired, &mut landed)?;
        for hop in cut(w, &exits, &mut drawn) {
            declared.judge(&hop)?;
            out.push(hop);
        }
    }
    Ok(out)
}

/// Whether the sheet's laws read this statement — a two-ended wire at all (a
/// measure or a mate never reaches a schematic scope: the statement gates
/// bounce it first, and a one-ended wire minted into a label wire is already
/// two-ended), asked of the statement's [`Owner`]:
///
/// - **`Engine`** — no. That engine already read its body's statements
///   ([SPEC 12–15]): a leader inside a nested `|drawing|` stays a leader, and
///   a pinless message into a `|sequence|`'s participant is not a landing.
///   This is the one thing the wire's own scope still has to say.
/// - **`Sheet`** — yes, the scope's own statement.
/// - **`Plain`** — **wherever it reaches into a sheet.** The router lands an
///   endpoint by the endpoint's own scope (`request::fixed`), so a law that
///   asked only the *writing* scope let `s.u1.a - s.r1` and `s.u1.b - s.r1`,
///   written outside the sheet, both arrive at `s.r1`'s bare port: two nets
///   shorted onto one pin, unreserved and undiagnosed. [`on_a_sheet`] asks the
///   endpoint instead, so both spellings of one circuit land identically.
fn reads_laws(w: &ResolvedLink, owner: Owner, nodes: &[ResolvedInst]) -> bool {
    if w.kind != LinkKind::Wire || w.endpoints.len() < 2 {
        return false;
    }
    match owner {
        Owner::Engine => false,
        Owner::Sheet => true,
        Owner::Plain => w.endpoints.iter().any(|e| on_a_sheet(nodes, &e.path)),
    }
}

/// Whether this endpoint is a sheet's own [SPEC 16]: the node it resolves to
/// belongs to the schematic family — part, pin, label or junction — and
/// `layout::schematic::check_types` has already refused one *outside* a
/// `layout: schematic`. So being a part **is** being in the scope; the wire
/// laws need no scope test of their own to say it, exactly as the router's
/// fixed ports need none ([`crate::routing::ortho::request`]). One gate, asked
/// once, upstream.
fn on_a_sheet(nodes: &[ResolvedInst], path: &str) -> bool {
    walk_scope(nodes, path.split('.'))
        .is_some_and(|inst| schematic_type(&inst.type_chain).is_some())
}

/// Land one wire's endpoints on pins [SPEC 16.5] — the law over whatever the
/// endpoints still leave open — and hand back the statement with its landings
/// written in, plus the pin each **pass-through** leaves by (indexed like the
/// endpoints; [`cut`] spends it).
fn land(
    mut w: ResolvedLink,
    nodes: &[ResolvedInst],
    wired: &mut arity::Wired,
    landed: &mut Landed,
) -> Result<(ResolvedLink, Vec<Option<String>>), Error> {
    let last = w.endpoints.len() - 1;
    // The pin each **pass-through** leaves by, at the endpoint it threads.
    let mut exits: Vec<Option<String>> = vec![None; w.endpoints.len()];
    #[allow(clippy::needless_range_loop)] // the body mutates `w.endpoints[i]`
    for i in 0..w.endpoints.len() {
        let (path, span, sided) = (
            w.endpoints[i].path.clone(),
            w.endpoints[i].span,
            w.endpoints[i].side.is_some(),
        );
        let Some(a) = addressed(nodes, &path) else {
            continue;
        };
        let entry = match &a.named {
            Some(pin) => Some(pin.clone()),
            // An authored `:side` on the **part** overrules the pin model
            // [SPEC 16.4]: the wire lands on the side the author forced, so
            // there is no pin to choose and none to spend. (A `:side` on a pin
            // is the terminal's own error, at the router's seam.)
            None if sided => None,
            // One written endpoint is one landing, however many fan siblings
            // expanded out of it — `x & y - r.r1` is one end, one pin, one
            // port. (A chain's middle endpoint is *not* this: it is one
            // endpoint read twice, which the cut below turns into two.)
            None => match landed.at(span, &a.part) {
                Some(pin) => Some(pin.to_string()),
                None => arity::choose(&a.pins, wired, &a.part, rel_path(&a.part, &w.scope), span)?,
            },
        };
        let Some(entry) = entry else { continue };
        wired.take(&a.part, &entry);
        landed.land(span, &a.part, &entry);
        w.endpoints[i].path = format!("{}.{}", a.part, entry);
        // The **pass-through** [SPEC 16.5], the same reading desugar makes for
        // the parts it can see: a two-pin part in the middle of a chain is
        // entered on one pin and left by the other. The exit is forced, never
        // chosen, so it spends no arity of its own.
        if 0 < i
            && i < last
            && a.pins.len() == 2
            && let Some(exit) = arity::other(&a.pins, &entry)
        {
            wired.take(&a.part, &exit);
            exits[i] = Some(format!("{}.{}", a.part, exit));
        }
    }
    Ok((w, exits))
}

/// Cut a statement into **one link per hop** — the router's wires are two-ended
/// everywhere, and a threaded part leaves the next hop by its other pin
/// (`exits[i]`, empty for a statement no arity law landed).
///
/// It is also where the `&` fan's legs are reconciled: `resolve_link` expanded
/// the fan around the whole chain, so a hop away from the fanned group arrives
/// once per leg — and a written hop is one wire, `drawn` being the reading of
/// "written" (the statement's span and the two endpoints' own). A statement with
/// one hop, a one-ended leader's fan and a dimension / mate chain are all left
/// exactly as they resolved.
fn cut(w: ResolvedLink, exits: &[Option<String>], drawn: &mut Drawn) -> Vec<ResolvedLink> {
    if w.one_ended || w.kind != LinkKind::Wire || w.endpoints.len() < 3 {
        return vec![w];
    }
    let last = w.endpoints.len() - 1;
    let mut out = Vec::with_capacity(last);
    for i in 0..last {
        let (from, to) = (&w.endpoints[i], &w.endpoints[i + 1]);
        if !drawn.first(&w, from.span, to.span) {
            continue;
        }
        let mut a = from.clone();
        if let Some(exit) = exits.get(i).and_then(Option::as_ref) {
            a.path = exit.clone();
        }
        let mut hop = w.clone();
        hop.endpoints = vec![a, to.clone()];
        // A chain's label rides every hop, exactly as `split_chain` gives it
        // to each of the hops it states [SPEC 9/18].
        out.push(hop);
    }
    out
}

/// What an endpoint addresses [SPEC 16.5]: the part it lands on, that part's
/// terminals in pin order, and the one it named (`None` — the pinless form —
/// is the landing arity decides). `None` for anything that is not a part with
/// pins: an ordinary box, and a `|label|`, whose one connection point is the
/// part itself.
struct Addressed {
    part: String,
    pins: Vec<Option<String>>,
    named: Option<String>,
}

fn addressed(nodes: &[ResolvedInst], path: &str) -> Option<Addressed> {
    let part_at = |p: &str| {
        let inst = walk_scope(nodes, p.split('.'))?;
        sch_kind(&inst.type_chain)?;
        let pins = terminal_ids(inst);
        (!pins.is_empty()).then_some(pins)
    };
    if let Some(pins) = part_at(path) {
        return Some(Addressed {
            part: path.to_string(),
            pins,
            named: None,
        });
    }
    // …else the path may name a terminal *inside* a part (`u7.vs`, `c24.p1`):
    // the rails a component's pins ride are anonymous, so a pin's path is the
    // part's plus its own id [SPEC 16.2].
    let (head, pin) = path.rsplit_once('.')?;
    let pins = part_at(head)?;
    pins.iter()
        .any(|p| p.as_deref() == Some(pin))
        .then(|| Addressed {
            part: head.to_string(),
            pins,
            named: Some(pin.to_string()),
        })
}

/// Where each **written endpoint** landed, by `(its own span, part)`. An `&`
/// fan expands one endpoint into a sibling per pair [SPEC 9], and the router
/// gives that shared end one port — so the law must give it one pin.
///
/// Keyed on the endpoint's own span and never the statement's; and a chain's
/// middle endpoint never reaches it twice, because a chain is still one link
/// here (one endpoint, read once) until [`land`] cuts it.
#[derive(Default)]
struct Landed(Vec<((Span, String), String)>);

impl Landed {
    fn at(&self, span: Span, part: &str) -> Option<&str> {
        if span == Span::empty() {
            return None; // generated: no written occurrence to share
        }
        self.0
            .iter()
            .find(|((s, p), _)| *s == span && p == part)
            .map(|(_, pin)| pin.as_str())
    }

    fn land(&mut self, span: Span, part: &str, pin: &str) {
        if self.at(span, part).is_none() {
            self.0.push(((span, part.to_string()), pin.to_string()));
        }
    }
}

/// The **written hops** a statement has already been cut into, by its own span
/// and the two endpoints' spans. An `&` fan expands around the whole chain
/// [SPEC 9], so every hop away from the fanned group is read once per leg —
/// and one written hop is one wire, the same reading of "written" [`Landed`]
/// makes for an endpoint.
///
/// Keyed on the statement too, so no two statements can ever collide, and on
/// the link's **scope**, because one authored statement inside a define body is
/// lifted once per host instance and every lift carries the same spans — two
/// hosts, two circuits. A generated statement (no written span) is never
/// reconciled, having no legs.
#[derive(Default)]
struct Drawn(Vec<(String, Span, Span, Span)>);

impl Drawn {
    /// Whether this is the hop's first reading — and record it if so.
    fn first(&mut self, w: &ResolvedLink, from: Span, to: Span) -> bool {
        if w.span == Span::empty() {
            return true;
        }
        let key = (w.scope.clone(), w.span, from, to);
        if self.0.contains(&key) {
            return false;
        }
        self.0.push(key);
        true
    }
}

/// The wires a sheet has already declared, by unordered **resolved** endpoint
/// pair — a repeated one means nothing on a sheet [SPEC 16.5/21]. Post-arity,
/// so two pinless landings on one part are two different wires and two
/// spellings of one pin are the same one.
#[derive(Default)]
struct Declared(Vec<((String, String), Span)>);

impl Declared {
    fn judge(&mut self, w: &ResolvedLink) -> Result<(), Error> {
        for hop in w.endpoints.windows(2) {
            let (a, b) = (&hop[0].path, &hop[1].path);
            let key = if a <= b {
                (a.clone(), b.clone())
            } else {
                (b.clone(), a.clone())
            };
            if let Some((_, prev)) = self.0.iter().find(|(k, _)| *k == key) {
                return Err(Error::at(
                    w.span,
                    format!(
                        "'{} - {}' is already wired — a repeated wire means nothing on a sheet",
                        rel_path(a, &w.scope),
                        rel_path(b, &w.scope)
                    ),
                )
                .with_related(*prev)
                .code(Code::DUPLICATE_WIRE));
            }
            self.0.push((key, w.span));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "arity_tests.rs"]
mod tests;
