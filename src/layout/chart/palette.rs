//! The deterministic series colour walk ([CHARTS.md] §10). One ordered hue
//! sequence, skipping `red` (reserved for danger), wrapping when exhausted — shared
//! by series colour assignment and (later) pie / bubble per-datum colour, so the
//! walk lives in exactly one place.

/// The hue order, `red` deliberately absent. Interleaved around the hue wheel (every
/// adjacent pair ≥ ~90° apart in OKLCH hue), not marched warm→cool, so neighbouring
/// series read as distinct — the common 2–4-series case gets the strongest contrast.
const WALK: &[&str] = &[
    "rose", "teal", "orange", "sky", "amber", "purple", "green", "blue", "lime", "gray",
];

/// The palette-var hue name for the `i`-th coloured datum (the bare base tier — a
/// `--name` reference resolved by the renderer, so it themes / flips / bakes).
pub fn hue(i: usize) -> &'static str {
    WALK[i % WALK.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walk_skips_red_and_wraps() {
        assert_eq!(hue(0), "rose");
        assert_eq!(hue(1), "teal");
        assert!(!WALK.contains(&"red"), "red is reserved for danger");
        assert_eq!(hue(WALK.len()), hue(0), "the walk wraps when exhausted");
    }
}
