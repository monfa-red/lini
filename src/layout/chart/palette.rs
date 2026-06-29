//! The deterministic series colour walk ([CHARTS.md] §10). One ordered hue
//! sequence, skipping `red` (reserved for danger), wrapping when exhausted — shared
//! by series colour assignment and (later) pie / bubble per-datum colour, so the
//! walk lives in exactly one place.

use crate::resolve::ResolvedValue;

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

/// The `-deep` tier of a palette hue, for a fill shape's edge ([CHARTS.md] §10):
/// `--teal` → `--teal-deep`, `--green-soft` → `--green-deep`. A non-palette colour
/// (a hex, a gradient) is its own edge. Shared by the area edge and the bars / slice
/// default outline, so the outlined look derives its edge in one place.
pub fn deepen(color: &ResolvedValue) -> ResolvedValue {
    if let ResolvedValue::LiveVar { name, raw } = color {
        let base = match name.rsplit_once('-') {
            Some((b, "wash" | "soft" | "deep" | "ink")) => b,
            _ => name.as_str(),
        };
        return ResolvedValue::LiveVar {
            name: format!("{base}-deep"),
            raw: *raw,
        };
    }
    color.clone()
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
