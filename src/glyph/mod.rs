//! The drafting-glyph registry [SPEC 15.9] — the ISO 1101 characteristic
//! symbols, the modifier circles (Ⓜ Ⓛ Ⓕ Ⓣ Ⓟ), and the ISO 1302 finish vees,
//! as path data on a shared grid. The lookup/suggest shape mirrors
//! [`crate::icon`]; the render emitter reuses the icon role groups. The one
//! law that differs is **sizing**: a glyph is never fit to a box — it emits
//! in natural units, its height following the annotation `font-size`, its
//! line weight the statement's `stroke-width`, so every symbol reads at
//! dimension-linework weight at every view scale.

use crate::icon::Role;

/// The authoring grid: every glyph is `GRID` units tall (y `0` the top,
/// `GRID` the bottom); its `width` varies. The emitter scales by
/// `height / GRID`, uniformly — height-derived, never fit-to-box.
pub const GRID: f64 = 100.0;

/// The drafting-symbol type a node's `.lini-*` chain wears, if any — the one
/// list behind the [SPEC 20] drawing-scope gate, the carried-`[ ]` gate at
/// resolve, and the layout lowering dispatch [SPEC 15.9].
pub fn drafting_type(chain: &[String]) -> Option<&'static str> {
    chain.iter().find_map(|t| match t.as_str() {
        "surface-finish" => Some("surface-finish"),
        "feature-control" => Some("feature-control"),
        "control" => Some("control"),
        "datum" => Some("datum"),
        _ => None,
    })
}

/// The finish vee's anatomy on the grid [SPEC 15.9]: the tip (the point that
/// stands on the surface) at x `FINISH_TIP_X`, y `GRID`; the long leg's apex
/// at x `FINISH_APEX_X`, y `0` — the indication rides there. Legs run 30° off
/// vertical (60° to the surface, ISO 1302), the short leg to 45 % height.
pub const FINISH_TIP_X: f64 = 30.0;
pub const FINISH_APEX_X: f64 = 87.7;

/// One registered glyph: its grid width and its geometry fragments, each a
/// full SVG element tagged with its paint role (`Line` stroked linework,
/// `Solid` filled detail — an arrowhead), exactly like an icon's.
pub struct Glyph {
    pub width: f64,
    pub frags: &'static [(Role, &'static str)],
}

macro_rules! glyphs {
    ($($name:literal => $width:literal, [$(($role:ident, $frag:literal)),+ $(,)?];)+) => {
        /// The registry, sorted by name (binary-searched by [`lookup`]).
        const TABLE: &[(&str, Glyph)] = &[
            $(($name, Glyph { width: $width, frags: &[$((Role::$role, $frag)),+] })),+
        ];
    };
}

glyphs! {
    // ── ISO 1101 characteristics [SPEC 15.9] — form, profile, orientation,
    // location, runout. Consumed by `|feature-control|`.
    "angularity" => 100.0, [(Line, r#"<path d="M 18 78 L 72 24 M 18 78 L 86 78"/>"#)];
    "circular-runout" => 100.0, [
        (Line, r#"<path d="M 22 82 L 64 40"/>"#),
        (Solid, r#"<path d="M 80 24 L 68.3 46.3 L 57.7 35.7 Z"/>"#),
    ];
    "circularity" => 100.0, [(Line, r#"<path d="M 18 50 A 32 32 0 1 1 82 50 A 32 32 0 1 1 18 50"/>"#)];
    "concentricity" => 100.0, [
        (Line, r#"<path d="M 36 50 A 14 14 0 1 1 64 50 A 14 14 0 1 1 36 50 M 20 50 A 30 30 0 1 1 80 50 A 30 30 0 1 1 20 50"/>"#),
    ];
    "cylindricity" => 100.0, [
        (Line, r#"<path d="M 24 54 A 26 26 0 1 1 76 54 A 26 26 0 1 1 24 54 M 12 86 L 32 22 M 68 86 L 88 22"/>"#),
    ];
    // ── ISO 1302 finish vees — `|surface-finish|`'s `symbol:` variants.
    "finish-basic" => 88.0, [(Line, r#"<path d="M 4 55 L 30 100 L 87.7 0"/>"#)];
    "finish-machined" => 88.0, [(Line, r#"<path d="M 4 55 L 30 100 L 87.7 0 M 4 55 L 56 55"/>"#)];
    "finish-prohibited" => 88.0, [
        (Line, r#"<path d="M 4 55 L 30 100 L 87.7 0 M 16 72 A 14 14 0 1 1 44 72 A 14 14 0 1 1 16 72"/>"#),
    ];
    "flatness" => 100.0, [(Line, r#"<path d="M 20 68 L 42 32 L 84 32 L 62 68 Z"/>"#)];
    // ── The modifier circles — a ring at the glyph box, the letter inside.
    "modifier-free-state" => 100.0, [
        (Line, r#"<path d="M 4 50 A 46 46 0 1 1 96 50 A 46 46 0 1 1 4 50 M 64 28 L 38 28 L 38 72 M 38 48 L 60 48"/>"#),
    ];
    "modifier-least" => 100.0, [
        (Line, r#"<path d="M 4 50 A 46 46 0 1 1 96 50 A 46 46 0 1 1 4 50 M 40 28 L 40 72 L 66 72"/>"#),
    ];
    "modifier-maximum" => 100.0, [
        (Line, r#"<path d="M 4 50 A 46 46 0 1 1 96 50 A 46 46 0 1 1 4 50 M 32 72 L 32 28 L 50 52 L 68 28 L 68 72"/>"#),
    ];
    "modifier-projected" => 100.0, [
        (Line, r#"<path d="M 4 50 A 46 46 0 1 1 96 50 A 46 46 0 1 1 4 50 M 40 72 L 40 28 L 54 28 A 13 13 0 0 1 54 54 L 40 54"/>"#),
    ];
    "modifier-tangent-plane" => 100.0, [
        (Line, r#"<path d="M 4 50 A 46 46 0 1 1 96 50 A 46 46 0 1 1 4 50 M 32 28 L 68 28 M 50 28 L 50 72"/>"#),
    ];
    "parallelism" => 100.0, [(Line, r#"<path d="M 26 84 L 52 16 M 54 84 L 80 16"/>"#)];
    "perpendicularity" => 100.0, [(Line, r#"<path d="M 50 22 L 50 78 M 16 78 L 84 78"/>"#)];
    "position" => 100.0, [
        (Line, r#"<path d="M 24 50 A 26 26 0 1 1 76 50 A 26 26 0 1 1 24 50 M 50 12 L 50 88 M 12 50 L 88 50"/>"#),
    ];
    "profile-line" => 100.0, [(Line, r#"<path d="M 16 68 A 34 34 0 0 1 84 68"/>"#)];
    "profile-surface" => 100.0, [(Line, r#"<path d="M 16 68 A 34 34 0 0 1 84 68 Z"/>"#)];
    "straightness" => 100.0, [(Line, r#"<path d="M 15 50 L 85 50"/>"#)];
    "symmetry" => 100.0, [(Line, r#"<path d="M 16 50 L 84 50 M 30 34 L 70 34 M 30 66 L 70 66"/>"#)];
    "total-runout" => 100.0, [
        (Line, r#"<path d="M 14 84 L 56 84 M 14 84 L 42 56 M 56 84 L 84 56"/>"#),
        (Solid, r#"<path d="M 54 44 L 46.2 60.2 L 37.8 51.8 Z"/>"#),
        (Solid, r#"<path d="M 96 44 L 88.2 60.2 L 79.8 51.8 Z"/>"#),
    ];
}

/// The glyph registered under `name`, or `None`.
pub fn lookup(name: &str) -> Option<&'static Glyph> {
    let i = TABLE.binary_search_by(|(n, _)| n.cmp(&name)).ok()?;
    Some(&TABLE[i].1)
}

/// Every registered name, sorted — the basis for [`suggest`].
/// (The unknown-characteristic did-you-mean consumes this from Stage 2's
/// `|feature-control|` validation; exercised by the registry tests now.)
#[cfg_attr(not(test), allow(dead_code))]
pub fn names() -> impl Iterator<Item = &'static str> {
    TABLE.iter().map(|(n, _)| *n)
}

/// Up to three names closest to `name`, for a "did you mean …?" hint.
#[cfg_attr(not(test), allow(dead_code))]
pub fn suggest(name: &str) -> Vec<&'static str> {
    crate::suggest::nearest(name, names(), 3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_and_lookup_hits_every_glyph() {
        let all: Vec<_> = names().collect();
        assert_eq!(all.len(), 22); // 14 characteristics + 5 modifiers + 3 vees
        assert!(all.windows(2).all(|w| w[0] < w[1]));
        for n in all {
            let g = lookup(n).expect(n);
            assert!(g.width > 0.0);
            assert!(!g.frags.is_empty());
            assert!(g.frags.iter().all(|(_, f)| f.starts_with("<path")));
        }
        assert!(lookup("no-such-glyph").is_none());
    }

    #[test]
    fn the_three_vees_share_one_anatomy() {
        // Every variant's linework starts at the same tip/legs, so the seat
        // anchor and indication position hold across `symbol:` values.
        for v in ["finish-basic", "finish-machined", "finish-prohibited"] {
            let g = lookup(v).unwrap();
            assert_eq!(g.width, 88.0);
            assert!(g.frags[0].1.contains("M 4 55 L 30 100 L 87.7 0"));
            // The anatomy constants sit on the glyph: tip left of apex, both
            // inside the width.
            assert!(FINISH_TIP_X < FINISH_APEX_X && FINISH_APEX_X < g.width);
        }
    }

    #[test]
    fn suggest_corrects_a_typo() {
        assert_eq!(suggest("flatnes").first(), Some(&"flatness"));
    }
}
