//! **The arity law** [SPEC 16.5] — which pin a landing takes — stated once and
//! applied twice.
//!
//! A wire to a 1-pin part lands on it; to a 2-pin part, on the next free pin in
//! the type's pin order (both taken is an error naming one); to a 3+-pin part it
//! is an error suggesting a pin. Dangling pins are legal. A chain **passes
//! through** a 2-pin part: the named (or next-free) pin is the entry, the other
//! pin the exit.
//!
//! **Resolution happens here, at desugar** ([`land`]), because two things
//! downstream read a landing before resolve ever runs:
//!
//! - the **lowered form is a program**: outside this scope desugar states a chain
//!   as a link per hop ([`crate::desugar::labels::split_chain`]), which destroys
//!   the statement a pass-through is defined over — so a chain resolved any
//!   later compiles differently from its own `lini desugar` output
//!   (`tests/oracle.rs`'s binding fixed point). Hence [`land`] cuts a schematic
//!   chain itself, **only where it resolved the pass-through** (both pins
//!   written down), and `split_chain` does not run in this scope at all: what
//!   this stage cannot resolve stays a chain, because only the chain still says
//!   what it means;
//! - the **pose chooser** turns a satellite to face the pin its wire names
//!   ([`crate::desugar::autopose`]), so a landing it cannot see is a part turned
//!   the wrong way.
//!
//! So the landings this stage can see — the scope's own statements onto the
//! scope's own parts — are rewritten to explicit pin paths *before* the pose,
//! and print that way. What desugar cannot see (a path reaching into another
//! scope, an anonymous container's part included — see [`child_by_id`]) stays
//! for [`crate::resolve::links`], chain and all: that pass applies **this** law
//! through [`Wired`], [`choose`] and [`other`] rather than a second copy of it,
//! cuts the chains it resolves, and judges duplicates over the resolved pairs.
//!
//! Idempotent by construction: the lowered form names every pin it resolved, and
//! a named pin is a reservation, never a choice — re-desugaring rewrites nothing
//! (a lowered part states its family as a class, so this pass does not even see
//! one).

use super::super::Lower;
use super::{SchKind, part_pin_ids, pins_of, sch_kind};
use crate::ast::ChainOp;
use crate::error::{Code, Error};
use crate::span::Span;
use crate::syntax::ast::{Child, Endpoint, Link, Node};

// ───────────────────────── the law ─────────────────────────

/// The pins already spoken for, by `(part path, pin id)` — the free/taken
/// bookkeeping. **One table shape, one law**: a pin is taken by being *named*,
/// by a pinless landing choosing it, and by a pass-through leaving through it.
/// Desugar keeps one per scope it resolves; resolve keeps one for the sheet.
#[derive(Default)]
pub(crate) struct Wired(Vec<(String, String)>);

impl Wired {
    pub(crate) fn holds(&self, part: &str, pin: &str) -> bool {
        self.0.iter().any(|(p, t)| p == part && t == pin)
    }

    pub(crate) fn take(&mut self, part: &str, pin: &str) {
        if !self.holds(part, pin) {
            self.0.push((part.to_string(), pin.to_string()));
        }
    }
}

/// The pin a **pinless** landing on `pins` takes [SPEC 16.5], or the error
/// naming one ([SPEC 21]'s two rows). `Ok(None)` means *leave the landing
/// alone*: the pin it would take is anonymous, so no path could name it
/// [SPEC 9] — and no error could tell the author what to write either.
/// `spelled` is the part as its own scope spells it, for the message.
pub(crate) fn choose(
    pins: &[Option<String>],
    wired: &Wired,
    part: &str,
    spelled: &str,
    span: Span,
) -> Result<Option<String>, Error> {
    if pins.len() == 1 {
        return Ok(pins[0].clone());
    }
    let named = pins.iter().flatten();
    let Some(first) = named.clone().next() else {
        return Ok(None);
    };
    if pins.len() > 2 {
        return Err(Error::at(
            span,
            format!(
                "'{spelled}' has {} pins — name one ('{spelled}.{first}')",
                pins.len()
            ),
        )
        .code(Code::SCHEMATIC_ARITY));
    }
    match named.clone().find(|p| !wired.holds(part, p)) {
        Some(free) => Ok(Some(free.clone())),
        None => Err(Error::at(
            span,
            format!("both pins of '{spelled}' are wired — name one ('{spelled}.{first}')"),
        )
        .code(Code::SCHEMATIC_ARITY)),
    }
}

/// The part's **other** pin — what a pass-through leaves by [SPEC 16.5].
/// `None` when it is anonymous, so no path could name it [SPEC 9].
///
/// **One law, one reading, both stages**: a chain is whole at each of them — this
/// one cuts only what it resolved, and leaves whole what it did not, precisely
/// so that resolve still reads the middle of a chain as the middle of a chain.
/// Both then ask this for the pin to leave by.
pub(crate) fn other(pins: &[Option<String>], pin: &str) -> Option<String> {
    pins.iter().flatten().find(|p| *p != pin).cloned()
}

// ───────────────────── the desugar-side resolution ─────────────────────

/// Resolve this scope's landings onto explicit pins [SPEC 16.5]: every wire
/// endpoint naming one of `kids` is rewritten to `part.pin`, and a statement
/// threading a two-pin part is cut into a link per hop so that its exit can be
/// the other pin. Runs on the **gathered** statements — capsules hoisted, label
/// wires minted — and before the pose chooser reads them.
///
/// Endpoints this scope cannot answer for (a path into a nested container, an
/// unknown id) are left exactly as written for resolve.
///
/// A statement can become several here, so any index into `links` moves: the
/// caller's `own_at` — where its own statements begin, the define-body ones
/// preceding them — comes back as the index into the landed list.
pub(in crate::desugar) fn land(
    cx: &Lower,
    kids: &[Child],
    links: Vec<Link>,
    wired: &mut Wired,
    own_at: usize,
) -> Result<(Vec<Link>, usize), Error> {
    // Every pin an endpoint **names** is spoken for before any pinless landing
    // chooses, so `u1.b - r1` takes p2 whether the explicit `r1.p1 - u1.a` was
    // written above it or below.
    for w in &links {
        for ep in w.chain.iter().flat_map(|g| &g.endpoints) {
            if let Some(a) = addressed(cx, kids, ep)
                && let Some(pin) = a.named
            {
                wired.take(&a.part, &pin);
            }
        }
    }
    let mut out = Vec::with_capacity(links.len());
    let mut own = own_at;
    for (i, w) in links.into_iter().enumerate() {
        let hops = land_statement(cx, kids, w, wired)?;
        if i < own_at {
            own += hops.len() - 1;
        }
        out.extend(hops);
    }
    Ok((out, own))
}

/// One statement's landings, and the links it becomes.
fn land_statement(
    cx: &Lower,
    kids: &[Child],
    w: Link,
    wired: &mut Wired,
) -> Result<Vec<Link>, Error> {
    if !matches!(w.op(), ChainOp::Wire(_)) || w.chain.len() < 2 {
        return Ok(vec![w]);
    }
    let last = w.chain.len() - 1;
    // Per endpoint occurrence, by (group, index within it): the pin it lands
    // on, and — a pass-through — the pin the next hop leaves by.
    let mut landed: Vec<Vec<(Option<String>, Option<String>)>> = Vec::new();
    for (g, group) in w.chain.iter().enumerate() {
        let mut row = Vec::with_capacity(group.endpoints.len());
        for ep in &group.endpoints {
            row.push(land_endpoint(cx, kids, ep, 0 < g && g < last, wired)?);
        }
        landed.push(row);
    }
    // **Cut only where a pass-through was resolved.** The entry pin ends one
    // wire and the exit pin starts the next, which a single statement cannot
    // spell — so that cut has to happen, and here, where both pins are known.
    //
    // A mid group this stage could *not* answer for (a part in another scope)
    // keeps the chain going instead: what the statement says about it must
    // survive printing, and only the chain itself says it. Resolve reads such a
    // chain as a chain, applies the same law, and cuts it there.
    let cuts: Vec<usize> = (0..w.chain.len())
        .filter(|&g| landed[g].iter().any(|(_, exit)| exit.is_some()))
        .collect();
    let mut out = Vec::with_capacity(cuts.len() + 1);
    let mut start = 0usize;
    for end in cuts.into_iter().chain(std::iter::once(last)) {
        let mut hop = Link {
            chain: w.chain[start..=end].to_vec(),
            ops: w.ops[start..end].to_vec(),
            classes: w.classes.clone(),
            style: w.style.clone(),
            style_span: w.style_span,
            label: w.label.clone(),
            labels: w.labels.clone(),
            span: w.span,
        };
        for (k, group) in hop.chain.iter_mut().enumerate() {
            let g = start + k;
            for (ep, (entry, exit)) in group.endpoints.iter_mut().zip(&landed[g]) {
                // A cut's own group **leaves** by the exit pin; every other
                // reading of a group is its entry.
                let pin = if g == start && g > 0 {
                    exit.as_ref()
                } else {
                    entry.as_ref()
                };
                if let Some(pin) = pin {
                    let part = ep.path.first().cloned().unwrap_or_default();
                    ep.path = vec![part, pin.clone()];
                }
            }
        }
        out.push(hop);
        start = end;
    }
    Ok(out)
}

/// One endpoint's landing: the pin it takes, and the pin a chain leaves by when
/// it threads a two-pin part here.
fn land_endpoint(
    cx: &Lower,
    kids: &[Child],
    ep: &Endpoint,
    mid_chain: bool,
    wired: &mut Wired,
) -> Result<(Option<String>, Option<String>), Error> {
    let Some(a) = addressed(cx, kids, ep) else {
        return Ok((None, None));
    };
    let entry = match &a.named {
        Some(pin) => Some(pin.clone()),
        // An authored `:side` on the **part** overrules the pin model
        // [SPEC 16.4]: the wire lands on the side the author forced, so there
        // is no pin to choose and none to spend.
        None if ep.point.is_some() => None,
        None => choose(&a.pins, wired, &a.part, &a.part, ep.span)?,
    };
    let Some(entry) = entry else {
        return Ok((None, None));
    };
    wired.take(&a.part, &entry);
    // The **pass-through** [SPEC 16.5]: a two-pin part in the middle of a chain
    // is entered on one pin and left by the other — a series circuit in one
    // line. The exit is forced, never chosen, so it spends no arity of its own.
    let exit = (mid_chain && a.pins.len() == 2)
        .then(|| other(&a.pins, &entry))
        .flatten();
    if let Some(exit) = &exit {
        wired.take(&a.part, exit);
    }
    Ok((Some(entry), exit))
}

/// What an endpoint addresses among the parts **this scope declares**: the
/// part, its terminals in pin order, and the one the path named. `None` for
/// anything this stage cannot answer for — a path past its own children (into a
/// nested container, named or anonymous — see [`child_by_id`]), an unknown id, a
/// `|label|` (whose one connection point is the part itself), or a lowered part
/// (whose family is a class, so re-desugaring resolves nothing a second time).
struct Addressed {
    part: String,
    pins: Vec<Option<String>>,
    named: Option<String>,
}

fn addressed(cx: &Lower, kids: &[Child], ep: &Endpoint) -> Option<Addressed> {
    if ep.copy.is_some() || ep.path.len() > 2 {
        return None;
    }
    let head = ep.path.first()?;
    let node = child_by_id(kids, head)?;
    let chain = cx.authored_chain(node);
    sch_kind(&chain)?;
    let pins = authored_terminal_ids(cx, node, &chain);
    if pins.is_empty() {
        return None;
    }
    let named = match ep.path.get(1) {
        None => None,
        // A path naming something the part does not offer is not this stage's
        // to judge (resolve words the unknown-endpoint error).
        Some(pin) => Some(pins.iter().flatten().find(|p| *p == pin)?.clone()),
    };
    Some(Addressed {
        part: head.clone(),
        pins,
        named,
    })
}

/// The terminals an **authored** part offers, in pin order — the twin of
/// [`super::terminal_ids`] over the tree desugar still holds. Two readers,
/// because the two trees are genuinely different data (an AST node's family and
/// pins need the lowering context; a lowered node's are classes and attrs); the
/// **table** underneath is the one in [`super::family`], and neither reader
/// keeps a list of its own.
pub(crate) fn authored_terminal_ids(
    cx: &Lower,
    node: &Node,
    chain: &[String],
) -> Vec<Option<String>> {
    match sch_kind(chain) {
        Some(SchKind::Component) => pins_of(cx, node, chain)
            .iter()
            .map(|p| p.id.clone())
            .collect(),
        Some(_) => {
            let symbol = cx.chain_ident(chain, &node.style, "symbol");
            part_pin_ids(chain, symbol.as_deref())
                .iter()
                .map(|p| Some((*p).to_string()))
                .collect()
        }
        None => Vec::new(),
    }
}

/// The part this scope **declares** under `id` — its own children, and no
/// further.
///
/// It deliberately does *not* follow scope transparency [SPEC 9] into an
/// anonymous container, even though a path does: an anonymous container runs
/// its own gather, with its own table, over its own statements, and two tables
/// spending one part's pins is a short. So a gather lands **the parts it
/// declares**, and everything reached past that — an anonymous container's part
/// exactly like a named one's (`r.r1`) — defers to resolve, whose one table
/// sees every pin this stage has already named.
///
/// The other direction (the enclosing scope owning a transparent child's pins)
/// was rejected: the inner gather would then have to defer *its own* statements,
/// and this scope's own landings would be the ones a second table could spend.
/// Deferring outward costs nothing in meaning — a chain through a deferred part
/// stays a chain all the way to resolve, which reads it by this same law — only
/// the *choice* of pin waits for the one table that can make it.
fn child_by_id<'a>(kids: &'a [Child], id: &str) -> Option<&'a Node> {
    kids.iter()
        .filter_map(|c| match c {
            Child::Box(b) => Some(b),
            _ => None,
        })
        .find(|b| b.id.as_deref() == Some(id))
}
