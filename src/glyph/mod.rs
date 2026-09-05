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
    /// by it), real sheet px for schematic ones (the pose's port math reads
    /// it, [SPEC 16.1]).
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
    "sch-bz" => 64.0, 24.0, [(0.0, 12.0), (64.0, 12.0)], [(Line, r#"<path d="M 0 12 L 24 12 M 24 22 L 24 2 A 10 10 0 0 1 24 22 Z M 34 12 L 64 12"/>"#)];
    "sch-c" => 64.0, 24.0, [(0.0, 12.0), (64.0, 12.0)], [
        (Line, r#"<path d="M 0 12 L 29 12 M 35 12 L 64 12"/>"#),
        (Solid, r#"<path d="M 27.5 0 L 30.5 0 L 30.5 24 L 27.5 24 Z M 33.5 0 L 36.5 0 L 36.5 24 L 33.5 24 Z"/>"#),
    ];
    "sch-c-polarized" => 64.0, 32.0, [(0.0, 16.0), (64.0, 16.0)], [
        (Line, r#"<path d="M 0 16 L 29 16 M 39 4 A 18 18 0 0 0 39 28 M 37 16 L 64 16 M 6 2 L 12 2 M 9 0 L 9 5"/>"#),
        (Solid, r#"<path d="M 27.5 4 L 30.5 4 L 30.5 28 L 27.5 28 Z"/>"#),
    ];
    "sch-chassis" => 16.0, 12.0, [(8.0, 0.0)], [(Line, r#"<path d="M 8 0 L 8 6 M 2 6 L 16 6 M 5 6 L 2 12 M 10 6 L 7 12 M 15 6 L 12 12"/>"#)];
    "sch-d" => 64.0, 16.0, [(0.0, 8.0), (64.0, 8.0)], [(Line, r#"<path d="M 0 8 L 24 8 M 24 0 L 40 8 L 24 16 Z M 40 0 L 40 16 M 40 8 L 64 8"/>"#)];
    "sch-d-schottky" => 64.0, 16.0, [(0.0, 8.0), (64.0, 8.0)], [(Line, r#"<path d="M 0 8 L 24 8 M 24 0 L 40 8 L 24 16 Z M 36 2 L 36 0 L 40 0 L 40 16 L 44 16 L 44 14 M 40 8 L 64 8"/>"#)];
    "sch-d-tvs" => 76.0, 16.0, [(0.0, 8.0), (76.0, 8.0)], [(Line, r#"<path d="M 0 8 L 22 8 M 22 0 L 38 8 L 22 16 Z M 54 0 L 38 8 L 54 16 Z M 41 0 L 38 2 L 38 14 L 35 16 M 54 8 L 76 8"/>"#)];
    "sch-d-zener" => 64.0, 16.0, [(0.0, 8.0), (64.0, 8.0)], [(Line, r#"<path d="M 0 8 L 24 8 M 24 0 L 40 8 L 24 16 Z M 44 0 L 40 2 L 40 14 L 36 16 M 40 8 L 64 8"/>"#)];
    "sch-earth" => 16.0, 14.0, [(8.0, 0.0)], [(Line, r#"<path d="M 8 0 L 8 6 M 0 6 L 16 6 M 3 10 L 13 10 M 6 14 L 10 14"/>"#)];
    "sch-f" => 64.0, 12.0, [(0.0, 6.0), (64.0, 6.0)], [(Line, r#"<path d="M 0 6 L 64 6 M 16 0 L 48 0 L 48 12 L 16 12 Z"/>"#)];
    "sch-fb" => 64.0, 26.0, [(0.0, 13.0), (64.0, 13.0)], [(Line, r#"<path d="M 0 13 L 25.65 13 M 38.35 13 L 64 13 M 42.26 6.22 L 32.74 0.72 L 21.74 19.78 L 31.26 25.28 Z"/>"#)];
    "sch-gnd" => 16.0, 15.0, [(8.0, 0.0)], [(Line, r#"<path d="M 8 0 L 8 6 M 0 6 L 16 6 M 1 6 L 8 15 L 15 6"/>"#)];
    "sch-i" => 64.0, 24.0, [(0.0, 12.0), (64.0, 12.0)], [(Line, r#"<path d="M 0 12 L 20 12 M 20 12 A 12 12 0 1 0 44 12 A 12 12 0 1 0 20 12 M 44 12 L 64 12 M 26 12 L 38 12 M 34 8 L 38 12 L 34 16"/>"#)];
    "sch-i-ac" => 64.0, 24.0, [(0.0, 12.0), (64.0, 12.0)], [(Line, r#"<path d="M 0 12 L 20 12 M 20 12 A 12 12 0 1 0 44 12 A 12 12 0 1 0 20 12 M 44 12 L 64 12 M 26 7 Q 29 3 32 7 Q 35 11 38 7 M 26 16 L 38 16 M 34 12 L 38 16 L 34 20"/>"#)];
    "sch-l" => 64.0, 10.0, [(0.0, 5.0), (64.0, 5.0)], [(Line, r#"<path d="M 0 5 L 12 5 A 5 5 0 0 1 22 5 A 5 5 0 0 1 32 5 A 5 5 0 0 1 42 5 A 5 5 0 0 1 52 5 L 64 5"/>"#)];
    "sch-led" => 64.0, 28.0, [(0.0, 14.0), (64.0, 14.0)], [(Line, r#"<path d="M 0 14 L 24 14 M 24 6 L 40 14 L 24 22 Z M 40 6 L 40 22 M 40 14 L 64 14 M 28 6 L 34 0 M 34 3 L 34 0 L 31 0 M 36 8 L 42 2 M 42 5 L 42 2 L 39 2"/>"#)];
    "sch-m" => 64.0, 24.0, [(0.0, 12.0), (64.0, 12.0)], [(Line, r#"<path d="M 0 12 L 20 12 M 20 12 A 12 12 0 1 0 44 12 A 12 12 0 1 0 20 12 M 44 12 L 64 12 M 26 17 L 26 7 L 32 13 L 38 7 L 38 17"/>"#)];
    "sch-nc" => 12.0, 10.0, [(0.0, 5.0)], [(Line, r#"<path d="M 0 5 L 8 5 M 4 1 L 12 9 M 12 1 L 4 9"/>"#)];
    "sch-opamp" => 64.0, 64.0, [(64.0, 32.0), (0.0, 12.0), (0.0, 52.0)], [(Line, r#"<path d="M 12 4 L 60 32 L 12 60 Z M 0 12 L 12 12 M 0 52 L 12 52 M 60 32 L 64 32 M 17 17 L 25 17 M 21 13 L 21 21 M 17 47 L 25 47"/>"#)];
    "sch-power" => 16.0, 20.0, [(8.0, 20.0)], [(Line, r#"<path d="M 8 20 L 8 0 M 3 9 L 8 0 L 13 9"/>"#)];
    "sch-q-nfet" => 56.0, 48.0, [(0.0, 24.0), (56.0, 4.0), (56.0, 44.0)], [
        (Line, r#"<path d="M 10 24 A 20 20 0 1 1 50 24 A 20 20 0 1 1 10 24 M 0 24 L 20 24 M 20 14 L 20 34 M 28.5 16 L 44 16 L 44 4 L 56 4 M 28.5 32 L 44 32 L 44 44 L 56 44 M 28.5 24 L 44 24 L 44 32"/>"#),
        (Solid, r#"<path d="M 25.5 14 L 28.5 14 L 28.5 19.3 L 25.5 19.3 Z M 25.5 21.3 L 28.5 21.3 L 28.5 26.7 L 25.5 26.7 Z M 25.5 28.7 L 28.5 28.7 L 28.5 34 L 25.5 34 Z M 28 24 L 37 20.8 L 37 27.2 Z"/>"#),
    ];
    "sch-q-npn" => 56.0, 48.0, [(0.0, 24.0), (56.0, 4.0), (56.0, 44.0)], [
        (Line, r#"<path d="M 10 24 A 20 20 0 1 1 50 24 A 20 20 0 1 1 10 24 M 0 24 L 24 24 M 24 20 L 40 4 L 56 4 M 24 28 L 40 44 L 56 44"/>"#),
        (Solid, r#"<path d="M 22.5 16 L 25.5 16 L 25.5 32 L 22.5 32 Z M 36.02 40.02 L 31.92 31.4 L 27.4 35.92 Z"/>"#),
    ];
    "sch-q-pfet" => 56.0, 48.0, [(0.0, 24.0), (56.0, 4.0), (56.0, 44.0)], [
        (Line, r#"<path d="M 10 24 A 20 20 0 1 1 50 24 A 20 20 0 1 1 10 24 M 0 24 L 20 24 M 20 14 L 20 34 M 28.5 16 L 44 16 L 44 4 L 56 4 M 28.5 32 L 44 32 L 44 44 L 56 44 M 28.5 24 L 44 24 L 44 32"/>"#),
        (Solid, r#"<path d="M 25.5 14 L 28.5 14 L 28.5 19.3 L 25.5 19.3 Z M 25.5 21.3 L 28.5 21.3 L 28.5 26.7 L 25.5 26.7 Z M 25.5 28.7 L 28.5 28.7 L 28.5 34 L 25.5 34 Z M 37 24 L 28 27.2 L 28 20.8 Z"/>"#),
    ];
    "sch-q-pnp" => 56.0, 48.0, [(0.0, 24.0), (56.0, 4.0), (56.0, 44.0)], [
        (Line, r#"<path d="M 10 24 A 20 20 0 1 1 50 24 A 20 20 0 1 1 10 24 M 0 24 L 24 24 M 24 20 L 40 4 L 56 4 M 24 28 L 40 44 L 56 44"/>"#),
        (Solid, r#"<path d="M 22.5 16 L 25.5 16 L 25.5 32 L 22.5 32 Z M 26.12 30.12 L 34.75 34.23 L 30.23 38.75 Z"/>"#),
    ];
    "sch-r" => 64.0, 12.0, [(0.0, 6.0), (64.0, 6.0)], [(Line, r#"<path d="M 0 6 L 16 6 M 16 0 L 48 0 L 48 12 L 16 12 Z M 48 6 L 64 6"/>"#)];
    "sch-r-ntc" => 64.0, 20.0, [(0.0, 10.0), (64.0, 10.0)], [(Line, r#"<path d="M 0 10 L 16 10 M 16 4 L 48 4 L 48 16 L 16 16 Z M 48 10 L 64 10 M 14 20 L 22 20 L 50 0"/>"#)];
    "sch-r-pot" => 64.0, 24.0, [(0.0, 12.0), (64.0, 12.0), (32.0, 24.0)], [
        (Line, r#"<path d="M 0 12 L 16 12 M 16 6 L 48 6 L 48 18 L 16 18 Z M 48 12 L 64 12 M 32 24 L 32 21"/>"#),
        (Solid, r#"<path d="M 32 18 L 28.8 21.6 L 35.2 21.6 Z"/>"#),
    ];
    "sch-sw-push" => 64.0, 28.0, [(0.0, 14.0), (64.0, 14.0)], [(Line, r#"<path d="M 0 14 L 24 14 M 40 14 L 64 14 M 20 8 L 44 8 M 32 8 L 32 0"/>"#)];
    "sch-sw-toggle" => 64.0, 24.0, [(0.0, 12.0), (64.0, 12.0)], [(Line, r#"<path d="M 0 12 L 20 12 M 20 12 L 44 0 M 44 12 L 64 12"/>"#)];
    "sch-tp" => 32.0, 12.0, [(0.0, 6.0)], [(Line, r#"<path d="M 0 6 L 20 6 M 20 6 A 6 6 0 1 0 32 6 A 6 6 0 1 0 20 6"/>"#)];
    "sch-v-ac" => 64.0, 24.0, [(0.0, 12.0), (64.0, 12.0)], [(Line, r#"<path d="M 0 12 L 20 12 M 20 12 A 12 12 0 1 0 44 12 A 12 12 0 1 0 20 12 M 44 12 L 64 12 M 26 12 Q 29 6 32 12 Q 35 18 38 12"/>"#)];
    "sch-v-dc" => 64.0, 24.0, [(0.0, 12.0), (64.0, 12.0)], [(Line, r#"<path d="M 0 12 L 20 12 M 20 12 A 12 12 0 1 0 44 12 A 12 12 0 1 0 20 12 M 44 12 L 64 12 M 29 8 L 35 8 M 32 5 L 32 11 M 29 17 L 35 17"/>"#)];
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
        // 14 characteristics + 5 modifiers + 3 vees + 36 schematic symbols.
        assert_eq!(all.len(), 58);
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
            // One stroked `Line` fragment, and at most one `Solid` beside it —
            // the filled detail a drawing carries (plates, bars, arrowheads),
            // which desugar lowers as an overlay on the linework [SPEC 16.3].
            assert!(matches!(g.frags[0].0, Role::Line));
            assert!(
                g.frags.len() <= 2 && g.frags[1..].iter().all(|(r, _)| *r == Role::Solid),
                "{n}: one Line fragment, optionally one Solid"
            );
            swept += 1;
        }
        assert_eq!(swept, 36);
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
