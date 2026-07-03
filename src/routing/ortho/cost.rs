//! Law 3's economy constants (ROUTING.md §The Four Laws) — the one place the
//! cost formula's numbers live.
//!
//! cost = length + 2·clearance per turn + 4·clearance per crossing. A
//! crossing is worth a `4·clearance` detour and no more, so long orbits never
//! beat short crossings; turns cost real length, so straight beats dogleg
//! beats staircase. Pitch may compress to half the clearance — the one relief
//! valve; layout is never the relief.
//!
//! A per-diagram override (e.g. `link-crossing-cost: 0.5;` scaling these) is
//! a plausible future property: keep the constants threaded through call
//! sites via these functions, never inlined, so that knob stays a one-line
//! change.

/// What one 90° turn costs, in px of equivalent length.
pub(crate) fn turn_cost(clearance: f64) -> f64 {
    2.0 * clearance
}

/// What crossing one drawn link costs, in px of equivalent length.
pub(crate) fn cross_cost(clearance: f64) -> f64 {
    4.0 * clearance
}

/// Law 1's floor: the tightest link-to-link pitch a crowded channel or side
/// may compress to. Node clearance never shrinks.
pub(crate) fn min_pitch(clearance: f64) -> f64 {
    clearance / 2.0
}
