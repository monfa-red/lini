//! The deterministic series colour walk ([CHARTS.md] §10). One ordered hue
//! sequence, skipping `red` (reserved for danger), wrapping when exhausted — shared
//! by series colour assignment and (later) pie / bubble per-datum colour, so the
//! walk lives in exactly one place.

/// The hue order, `red` deliberately absent.
const WALK: &[&str] = &[
    "rose", "orange", "amber", "lime", "green", "teal", "sky", "blue", "purple", "gray",
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
        assert_eq!(hue(1), "orange");
        assert!(!WALK.contains(&"red"), "red is reserved for danger");
        assert_eq!(hue(WALK.len()), hue(0), "the walk wraps when exhausted");
    }
}
