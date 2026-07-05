//! The drawing layout family [SPEC 15]. Built in stages (PLAN.md): stage 2 is
//! the **sketch pen** — `|sketch|` is a closed primitive usable in any layout,
//! so the pen lives here while the drawing *engine* (datum placement, mates,
//! annotations) lands in later stages.

pub(crate) mod geometry;
pub(crate) mod pen;
