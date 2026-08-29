//! `layout: floorplan` [SPEC 15.11] — the drawing engine's **architectural
//! dialect**. There is no floorplan engine: a floorplan scope *is* a drawing
//! scope everywhere it counts (`crate::resolve::is_drawing_layout` is the one
//! place the two names meet), so the datum, `scale:` / `unit:`, the anchors,
//! the pen, dimensions, leaders, mates and sheets all arrive unchanged.
//!
//! What lives here is the part that is genuinely the dialect's: its
//! **vocabulary** — which types belong to it — and the laws [SPEC 21] states
//! about them. Every one of those laws is read in a single pass of the
//! resolved tree ([`check`], driven by the shared type gate
//! [`crate::layout::gates`]), so a new law is a clause here and never a second
//! traversal — the shape the schematic and chart families already take.

use crate::desugar::types::{FIXTURES, OPENINGS};
use crate::error::{Code, Error};
use crate::resolve::{AttrMap, ResolvedInst};

mod opening;
#[cfg(test)]
mod tests;
pub(in crate::layout) mod wall;

/// Whether a container opens a **floorplan** scope [SPEC 15.11] — the dialect's
/// own reading, for the vocabulary gate; everything mechanical asks the drawing
/// twin, since a floorplan is a drawing.
pub(super) fn is_floorplan(attrs: &AttrMap) -> bool {
    crate::resolve::is_floorplan(attrs)
}

/// The floorplan family a type belongs to [SPEC 15.11] — the one dispatch, so
/// a type cannot be gated as one thing and read as another.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum FpKind {
    /// `|wall|` and its `|partition|` define — a `|sketch|` on the centreline.
    Wall,
    /// `|door|` / `|window|` — stationed on a wall segment, in its `[ ]`.
    Opening,
    /// The six symbol-bodied furniture types.
    Fixture,
}

/// The family of a type chain — `None` outside the vocabulary. `|floorplan|`
/// is deliberately not a member: it *creates* the scope.
pub(crate) fn fp_kind<S: AsRef<str>>(chain: &[S]) -> Option<FpKind> {
    let has = |name: &str| chain.iter().any(|t| t.as_ref() == name);
    if has("wall") {
        return Some(FpKind::Wall);
    }
    if OPENINGS.iter().any(|t| has(t)) {
        return Some(FpKind::Opening);
    }
    FIXTURES.iter().any(|t| has(t)).then_some(FpKind::Fixture)
}

/// Whether a type chain is a `|door|` / `|window|` [SPEC 15.11] — the one
/// question the drawing engine asks the dialect: an opening is placed by its
/// station on the wall's segment, so it never datum-places.
pub(in crate::layout) fn is_opening(chain: &[String]) -> bool {
    fp_kind(chain) == Some(FpKind::Opening)
}

/// The floorplan type a chain wears, as the **author** spelled it — the
/// out-of-scope gate's subject [SPEC 21], the most-derived name, so a
/// `|partition|` says `'|partition|'` and a `|couch::sofa|` says `'|couch|'`.
fn written_type(inst: &ResolvedInst) -> Option<&str> {
    fp_kind(&inst.type_chain)?;
    crate::desugar::classes::written_type(&inst.type_chain)
}

/// **Every law the floorplan family states about one node** [SPEC 21], asked
/// once per node by the shared type gate. `in_scope` is whether a
/// `layout: floorplan` encloses it; `host` the type chain of the container the
/// node is written in (`None` at the scene root).
///
/// The scope law comes first — outside a floorplan the type may not exist at
/// all, so nothing further is worth saying about it.
pub(super) fn check(inst: &ResolvedInst, in_scope: bool, host: Option<&[String]>) -> Option<Error> {
    let kind = fp_kind(&inst.type_chain)?;
    let written = written_type(inst)?;
    if !in_scope {
        return Some(
            Error::at(
                inst.span,
                format!("'|{written}|' belongs in a 'layout: floorplan'"),
            )
            .code(Code::FLOORPLAN_TYPE),
        );
    }
    match kind {
        FpKind::Opening => opening(inst, written, host),
        FpKind::Fixture => stairs(inst, written),
        FpKind::Wall => None,
    }
}

/// An opening's own laws [SPEC 15.11]: it rides in its wall's `[ ]`, it is
/// placed by `on:` / `at:` alone, and a sliding door has no leaf to hang.
fn opening(inst: &ResolvedInst, written: &str, host: Option<&[String]>) -> Option<Error> {
    if !host.is_some_and(|h| fp_kind(h) == Some(FpKind::Wall)) {
        return Some(
            Error::at(
                inst.span,
                format!("a '|{written}|' rides in its wall's '[ ]'"),
            )
            .code(Code::OPENING_HOST),
        );
    }
    if inst.attrs.get("on").is_none() {
        return Some(
            Error::at(
                inst.span,
                format!("'|{written}|' requires 'on' — the wall segment it stations on"),
            )
            .code(Code::MISSING_REQUIRED),
        );
    }
    if inst.attrs.get("translate").is_some() {
        return Some(
            Error::at(
                inst.span,
                "an opening sits at 'on:' / 'at:' — move the station, or nudge the wall",
            )
            .code(Code::OPENING_PLACED),
        );
    }
    // A sliding door draws two panels along the gap, so there is no hinge jamb
    // and no arc: naming one is a contradiction, not a nudge to ignore.
    let sliding = matches!(inst.attrs.get("symbol"),
        Some(crate::resolve::ResolvedValue::Ident(s)) if s == "sliding");
    if sliding && (inst.attrs.get("hinge").is_some() || inst.attrs.get("swing").is_some()) {
        return Some(
            Error::at(
                inst.span,
                "a sliding door has no leaf to hang — remove 'hinge:' / 'swing:'",
            )
            .code(Code::SLIDING_LEAF),
        );
    }
    None
}

/// `|stairs|` needs its tread count [SPEC 15.11] — the flight generates from
/// it, exactly as a `|line|` needs its `points:`.
fn stairs(inst: &ResolvedInst, written: &str) -> Option<Error> {
    let missing =
        inst.type_chain.iter().any(|t| t == "stairs") && inst.attrs.get("steps").is_none();
    missing.then(|| {
        Error::at(
            inst.span,
            format!("'|{written}|' requires 'steps' — its tread count"),
        )
        .code(Code::MISSING_REQUIRED)
    })
}
