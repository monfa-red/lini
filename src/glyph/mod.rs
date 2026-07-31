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
/// list behind the [SPEC 21] drawing-scope gate, the carried-`[ ]` gate at
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

/// One registered glyph: its box, its **connection ports** (schematic glyphs
/// only — the points wires land on, in pin order [SPEC 16]), and its geometry
/// fragments, each a full SVG element tagged with its paint role (`Line`
/// stroked linework, `Solid` filled detail — an arrowhead), exactly like an
/// icon's. Drafting glyphs are `GRID` tall and emit height-scaled; schematic
/// glyphs are authored at **real sheet size** (the baked pitch constants,
/// [SPEC 10.5]) and lower verbatim — never font-coupled, never fit to a box.
pub struct Glyph {
    pub width: f64,
    /// The glyph box height — `GRID` for drafting glyphs (the emitter scales
    /// by it), real sheet px for schematic ones (Phase 4's port math reads it).
    #[cfg_attr(not(test), allow(dead_code))]
    pub height: f64,
    pub ports: &'static [(f64, f64)],
    pub frags: &'static [(Role, &'static str)],
}

macro_rules! glyphs {
    ($($name:literal => $width:literal, $height:literal, [$(($px:expr, $py:expr)),*], [$(($role:ident, $frag:literal)),+ $(,)?];)+) => {
        /// The registry, sorted by name (binary-searched by [`lookup`]).
        const TABLE: &[(&str, Glyph)] = &[
            $(($name, Glyph {
                width: $width,
                height: $height,
                ports: &[$(($px, $py)),*],
                frags: &[$((Role::$role, $frag)),+],
            })),+
        ];
    };
}

glyphs! {
    // ── ISO 1101 characteristics [SPEC 15.9] — form, profile, orientation,
    // location, runout. Consumed by `|feature-control|`.
    "angularity" => 100.0, 100.0, [], [(Line, r#"<path d="M 18 78 L 72 24 M 18 78 L 86 78"/>"#)];
    "circular-runout" => 100.0, 100.0, [], [
        (Line, r#"<path d="M 22 82 L 64 40"/>"#),
        (Solid, r#"<path d="M 80 24 L 68.3 46.3 L 57.7 35.7 Z"/>"#),
    ];
    "circularity" => 100.0, 100.0, [], [(Line, r#"<path d="M 18 50 A 32 32 0 1 1 82 50 A 32 32 0 1 1 18 50"/>"#)];
    "concentricity" => 100.0, 100.0, [], [
        (Line, r#"<path d="M 36 50 A 14 14 0 1 1 64 50 A 14 14 0 1 1 36 50 M 20 50 A 30 30 0 1 1 80 50 A 30 30 0 1 1 20 50"/>"#),
    ];
    "cylindricity" => 100.0, 100.0, [], [
        (Line, r#"<path d="M 24 54 A 26 26 0 1 1 76 54 A 26 26 0 1 1 24 54 M 12 86 L 32 22 M 68 86 L 88 22"/>"#),
    ];
    // ── ISO 1302 finish vees — `|surface-finish|`'s `symbol:` variants.
    "finish-basic" => 88.0, 100.0, [], [(Line, r#"<path d="M 4 55 L 30 100 L 87.7 0"/>"#)];
    "finish-machined" => 88.0, 100.0, [], [(Line, r#"<path d="M 4 55 L 30 100 L 87.7 0 M 4 55 L 56 55"/>"#)];
    "finish-prohibited" => 88.0, 100.0, [], [
        (Line, r#"<path d="M 4 55 L 30 100 L 87.7 0 M 16 72 A 14 14 0 1 1 44 72 A 14 14 0 1 1 16 72"/>"#),
    ];
    "flatness" => 100.0, 100.0, [], [(Line, r#"<path d="M 20 68 L 42 32 L 84 32 L 62 68 Z"/>"#)];
    // ── The modifier circles — a ring at the glyph box, the letter inside.
    "modifier-free-state" => 100.0, 100.0, [], [
        (Line, r#"<path d="M 4 50 A 46 46 0 1 1 96 50 A 46 46 0 1 1 4 50 M 64 28 L 38 28 L 38 72 M 38 48 L 60 48"/>"#),
    ];
    "modifier-least" => 100.0, 100.0, [], [
        (Line, r#"<path d="M 4 50 A 46 46 0 1 1 96 50 A 46 46 0 1 1 4 50 M 40 28 L 40 72 L 66 72"/>"#),
    ];
    "modifier-maximum" => 100.0, 100.0, [], [
        (Line, r#"<path d="M 4 50 A 46 46 0 1 1 96 50 A 46 46 0 1 1 4 50 M 32 72 L 32 28 L 50 52 L 68 28 L 68 72"/>"#),
    ];
    "modifier-projected" => 100.0, 100.0, [], [
        (Line, r#"<path d="M 4 50 A 46 46 0 1 1 96 50 A 46 46 0 1 1 4 50 M 40 72 L 40 28 L 54 28 A 13 13 0 0 1 54 54 L 40 54"/>"#),
    ];
    "modifier-tangent-plane" => 100.0, 100.0, [], [
        (Line, r#"<path d="M 4 50 A 46 46 0 1 1 96 50 A 46 46 0 1 1 4 50 M 32 28 L 68 28 M 50 28 L 50 72"/>"#),
    ];
    "parallelism" => 100.0, 100.0, [], [(Line, r#"<path d="M 26 84 L 52 16 M 54 84 L 80 16"/>"#)];
    "perpendicularity" => 100.0, 100.0, [], [(Line, r#"<path d="M 50 22 L 50 78 M 16 78 L 84 78"/>"#)];
    "position" => 100.0, 100.0, [], [
        (Line, r#"<path d="M 24 50 A 26 26 0 1 1 76 50 A 26 26 0 1 1 24 50 M 50 12 L 50 88 M 12 50 L 88 50"/>"#),
    ];
    "profile-line" => 100.0, 100.0, [], [(Line, r#"<path d="M 16 68 A 34 34 0 0 1 84 68"/>"#)];
    "profile-surface" => 100.0, 100.0, [], [(Line, r#"<path d="M 16 68 A 34 34 0 0 1 84 68 Z"/>"#)];
    // ── The schematic symbol set [SPEC 16.3/16.4] — IEC bodies with their
    // lead stubs included, authored at real sheet size (the pitch constants,
    // [SPEC 10.5]); `ports` are the wire landing points, **in pin order**
    // (p1 p2 · a k · b c e · g d s · plus minus · out inp inn; one point for
    // a label symbol — gnd's at its top, power's at its bottom). One `Line`
    // fragment each: the family renders outlined, and desugar lowers the one
    // fragment to one `|path|` child whose bbox matches the glyph box.
    "sch-antenna" => 20.0, 20.0, [(10.0, 20.0)], [(Line, r#"<path d="M 10 20 L 10 6 M 0 0 L 10 6 L 20 0"/>"#)];
    "sch-bt-battery" => 72.0, 20.0, [(0.0, 10.0), (72.0, 10.0)], [(Line, r#"<path d="M 0 10 L 24 10 M 24 0 L 24 20 M 32 5 L 32 15 M 40 0 L 40 20 M 48 5 L 48 15 M 48 10 L 72 10"/>"#)];
    "sch-bt-cell" => 64.0, 20.0, [(0.0, 10.0), (64.0, 10.0)], [(Line, r#"<path d="M 0 10 L 28 10 M 28 0 L 28 20 M 36 5 L 36 15 M 36 10 L 64 10"/>"#)];
    "sch-c" => 64.0, 16.0, [(0.0, 8.0), (64.0, 8.0)], [(Line, r#"<path d="M 0 8 L 29 8 M 29 0 L 29 16 M 35 0 L 35 16 M 35 8 L 64 8"/>"#)];
    "sch-c-polarized" => 64.0, 20.0, [(0.0, 12.0), (64.0, 12.0)], [(Line, r#"<path d="M 0 12 L 29 12 M 29 4 L 29 20 M 39 4 A 18 18 0 0 0 39 20 M 37 12 L 64 12 M 6 2 L 12 2 M 9 0 L 9 5"/>"#)];
    "sch-chassis" => 16.0, 12.0, [(8.0, 0.0)], [(Line, r#"<path d="M 8 0 L 8 4 M 2 4 L 16 4 M 5 4 L 2 10 M 10 4 L 7 10 M 15 4 L 12 10"/>"#)];
    "sch-d" => 64.0, 16.0, [(0.0, 8.0), (64.0, 8.0)], [(Line, r#"<path d="M 0 8 L 24 8 M 24 0 L 40 8 L 24 16 Z M 40 0 L 40 16 M 40 8 L 64 8"/>"#)];
    "sch-d-schottky" => 64.0, 16.0, [(0.0, 8.0), (64.0, 8.0)], [(Line, r#"<path d="M 0 8 L 24 8 M 24 0 L 40 8 L 24 16 Z M 36 2 L 36 0 L 40 0 L 40 16 L 44 16 L 44 14 M 40 8 L 64 8"/>"#)];
    "sch-d-tvs" => 76.0, 16.0, [(0.0, 8.0), (76.0, 8.0)], [(Line, r#"<path d="M 0 8 L 22 8 M 22 0 L 38 8 L 22 16 Z M 54 0 L 38 8 L 54 16 Z M 41 0 L 38 2 L 38 14 L 35 16 M 54 8 L 76 8"/>"#)];
    "sch-d-zener" => 64.0, 16.0, [(0.0, 8.0), (64.0, 8.0)], [(Line, r#"<path d="M 0 8 L 24 8 M 24 0 L 40 8 L 24 16 Z M 44 0 L 40 2 L 40 14 L 36 16 M 40 8 L 64 8"/>"#)];
    "sch-earth" => 16.0, 14.0, [(8.0, 0.0)], [(Line, r#"<path d="M 8 0 L 8 6 M 0 6 L 16 6 M 3 6 L 1 12 M 8 6 L 6 12 M 13 6 L 11 12"/>"#)];
    "sch-f" => 64.0, 12.0, [(0.0, 6.0), (64.0, 6.0)], [(Line, r#"<path d="M 0 6 L 64 6 M 16 0 L 48 0 L 48 12 L 16 12 Z"/>"#)];
    "sch-fb" => 64.0, 16.0, [(0.0, 8.0), (64.0, 8.0)], [(Line, r#"<path d="M 0 8 L 26 8 M 22 16 L 30 0 L 42 0 L 34 16 Z M 38 8 L 64 8"/>"#)];
    "sch-gnd" => 16.0, 14.0, [(8.0, 0.0)], [(Line, r#"<path d="M 8 0 L 8 6 M 0 6 L 16 6 M 3 10 L 13 10 M 6 14 L 10 14"/>"#)];
    "sch-i" => 64.0, 24.0, [(0.0, 12.0), (64.0, 12.0)], [(Line, r#"<path d="M 0 12 L 20 12 M 20 12 A 12 12 0 1 0 44 12 A 12 12 0 1 0 20 12 M 44 12 L 64 12 M 26 12 L 38 12 M 34 8 L 38 12 L 34 16"/>"#)];
    "sch-l" => 64.0, 10.0, [(0.0, 10.0), (64.0, 10.0)], [(Line, r#"<path d="M 0 10 L 12 10 A 5 5 0 0 1 22 10 A 5 5 0 0 1 32 10 A 5 5 0 0 1 42 10 A 5 5 0 0 1 52 10 L 64 10"/>"#)];
    "sch-led" => 64.0, 22.0, [(0.0, 14.0), (64.0, 14.0)], [(Line, r#"<path d="M 0 14 L 24 14 M 24 6 L 40 14 L 24 22 Z M 40 6 L 40 22 M 40 14 L 64 14 M 28 6 L 34 0 M 34 3 L 34 0 L 31 0 M 36 8 L 42 2 M 42 5 L 42 2 L 39 2"/>"#)];
    "sch-nc" => 14.0, 12.0, [(0.0, 6.0)], [(Line, r#"<path d="M 0 6 L 4 6 M 4 0 L 14 12 M 14 0 L 4 12"/>"#)];
    "sch-opamp" => 56.0, 36.0, [(56.0, 18.0), (0.0, 10.0), (0.0, 26.0)], [(Line, r#"<path d="M 12 0 L 44 18 L 12 36 Z M 0 10 L 12 10 M 0 26 L 12 26 M 44 18 L 56 18 M 15 10 L 21 10 M 18 7 L 18 13 M 15 26 L 21 26"/>"#)];
    "sch-power" => 16.0, 14.0, [(8.0, 14.0)], [(Line, r#"<path d="M 8 14 L 8 5 M 2 6 L 8 0 L 14 6"/>"#)];
    "sch-q-nfet" => 56.0, 32.0, [(0.0, 16.0), (56.0, 3.0), (56.0, 29.0)], [(Line, r#"<path d="M 0 16 L 20 16 M 20 6 L 20 26 M 26 6 L 26 26 M 26 8 L 40 8 L 40 3 L 56 3 M 26 24 L 40 24 L 40 29 L 56 29 M 32 20 L 26 24 L 32 28"/>"#)];
    "sch-q-npn" => 56.0, 32.0, [(0.0, 16.0), (56.0, 3.0), (56.0, 29.0)], [(Line, r#"<path d="M 16 16 A 14 14 0 1 1 44 16 A 14 14 0 1 1 16 16 M 0 16 L 24 16 M 24 8 L 24 24 M 24 12 L 40 3 L 56 3 M 24 20 L 40 29 L 56 29 M 34 23 L 40 29 L 32 28"/>"#)];
    "sch-q-pfet" => 56.0, 32.0, [(0.0, 16.0), (56.0, 3.0), (56.0, 29.0)], [(Line, r#"<path d="M 0 16 L 14 16 M 14 16 A 3 3 0 1 0 20 16 A 3 3 0 1 0 14 16 M 20 6 L 20 26 M 26 6 L 26 26 M 26 8 L 40 8 L 40 3 L 56 3 M 26 24 L 40 24 L 40 29 L 56 29 M 32 4 L 26 8 L 32 12"/>"#)];
    "sch-q-pnp" => 56.0, 32.0, [(0.0, 16.0), (56.0, 3.0), (56.0, 29.0)], [(Line, r#"<path d="M 16 16 A 14 14 0 1 1 44 16 A 14 14 0 1 1 16 16 M 0 16 L 24 16 M 24 8 L 24 24 M 24 12 L 40 3 L 56 3 M 24 20 L 40 29 L 56 29 M 30 26 L 24 20 L 32 21"/>"#)];
    "sch-r" => 64.0, 12.0, [(0.0, 6.0), (64.0, 6.0)], [(Line, r#"<path d="M 0 6 L 12 6 M 12 0 L 52 0 L 52 12 L 12 12 Z M 52 6 L 64 6"/>"#)];
    "sch-sw-push" => 64.0, 16.0, [(0.0, 14.0), (64.0, 14.0)], [(Line, r#"<path d="M 0 14 L 24 14 M 40 14 L 64 14 M 20 8 L 44 8 M 32 8 L 32 0"/>"#)];
    "sch-sw-toggle" => 64.0, 14.0, [(0.0, 12.0), (64.0, 12.0)], [(Line, r#"<path d="M 0 12 L 20 12 M 20 12 L 44 0 M 44 12 L 64 12"/>"#)];
    "sch-v-ac" => 64.0, 24.0, [(0.0, 12.0), (64.0, 12.0)], [(Line, r#"<path d="M 0 12 L 20 12 M 20 12 A 12 12 0 1 0 44 12 A 12 12 0 1 0 20 12 M 44 12 L 64 12 M 26 12 Q 29 6 32 12 Q 35 18 38 12"/>"#)];
    "sch-v-dc" => 64.0, 24.0, [(0.0, 12.0), (64.0, 12.0)], [(Line, r#"<path d="M 0 12 L 20 12 M 20 12 A 12 12 0 1 0 44 12 A 12 12 0 1 0 20 12 M 44 12 L 64 12 M 27 8 L 33 8 M 30 5 L 30 11 M 27 17 L 33 17"/>"#)];
    "sch-y" => 64.0, 20.0, [(0.0, 10.0), (64.0, 10.0)], [(Line, r#"<path d="M 0 10 L 24 10 M 24 2 L 24 18 M 28 0 L 36 0 L 36 20 L 28 20 Z M 40 2 L 40 18 M 40 10 L 64 10"/>"#)];
    "straightness" => 100.0, 100.0, [], [(Line, r#"<path d="M 15 50 L 85 50"/>"#)];
    "symmetry" => 100.0, 100.0, [], [(Line, r#"<path d="M 16 50 L 84 50 M 30 34 L 70 34 M 30 66 L 70 66"/>"#)];
    "total-runout" => 100.0, 100.0, [], [
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
        // 14 characteristics + 5 modifiers + 3 vees + 30 schematic symbols.
        assert_eq!(all.len(), 52);
        assert!(all.windows(2).all(|w| w[0] < w[1]));
        for n in all {
            let g = lookup(n).expect(n);
            assert!(g.width > 0.0 && g.height > 0.0);
            assert!(!g.frags.is_empty());
            assert!(g.frags.iter().all(|(_, f)| f.starts_with("<path")));
        }
        assert!(lookup("no-such-glyph").is_none());
    }

    #[test]
    fn schematic_glyphs_carry_in_box_ports_and_one_line_frag() {
        // Every `sch-` glyph: ports inside its box (wires land on them,
        // [SPEC 16]); exactly one `Line` fragment (the family renders
        // outlined, and desugar lowers the one fragment to one `|path|`).
        let mut swept = 0;
        for n in names().filter(|n| n.starts_with("sch-")) {
            let g = lookup(n).unwrap();
            assert!(!g.ports.is_empty(), "{n}: a symbol has connection points");
            for (x, y) in g.ports {
                assert!(
                    (0.0..=g.width).contains(x) && (0.0..=g.height).contains(y),
                    "{n}: port ({x}, {y}) outside {}x{}",
                    g.width,
                    g.height
                );
            }
            assert_eq!(g.frags.len(), 1, "{n}: one Line fragment");
            assert!(matches!(g.frags[0].0, Role::Line));
            swept += 1;
        }
        assert_eq!(swept, 30);
        // Drafting glyphs carry no ports — they are not wired.
        assert!(lookup("flatness").unwrap().ports.is_empty());
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
