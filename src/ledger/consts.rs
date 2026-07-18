//! Shared chrome / look constants [SPEC 10.5] — the drawing chrome set, the
//! look tunables, and the cross-file baked fallbacks, in one home so the
//! whole look is tuned from this module.

// ── The dimension / leader anatomy [SPEC 15.6/15.7] — baked sheet constants,
// never scaled by the view.
/// The drawing scope's `clearance` default for its dimensions [SPEC 15.6] —
/// pushed into the link base beside the thin stroke, below every user rule.
/// Row offsets derive from painted bounds + clearance; 4 stands a first
/// bottom row's value text 4 off the geometry, which puts its dim line at
/// the old fixed offset (text reach 14 + 4 = 18) — the visual anchor.
pub(crate) const DIM_CLEARANCE: f64 = 4.0;
pub(crate) const EXT_GAP: f64 = 3.0;
pub(crate) const EXT_OVERSHOOT: f64 = 3.0;
/// The drafting-slender arrow, 3 : 1 [SPEC 15.6] — length × half-width, at
/// stroke-width 1; both scale with the dim's `stroke-width` (drafting strokes
/// stay 1–2, so the heads read at ISO 129's arrow-≈-text-height weight).
pub(crate) const ARROW_LEN: f64 = 12.0;
pub(crate) const ARROW_HALF: f64 = 2.0;
pub(crate) const NOTE_OFFSET: f64 = 14.0;
pub(crate) const NOTE_LANDING: f64 = 8.0;
/// Stacked deviations draw at this fraction of the dimension font [SPEC 15.6].
pub(crate) const TOL_STACK: f64 = 0.7;
/// The GD&T datum triangle's side [SPEC 15.7] — a chunkier symbol than an
/// arrow, with a floor so it never vanishes on thin leaders.
pub(crate) const DATUM_SIZE: f64 = 11.0;

// ── Break and centerline chrome [SPEC 15.5].
/// The sheet-space daylight a break leaves between the pieces.
pub(crate) const BREAK_GAP: f64 = 12.0;
/// Centre marks, auto centerlines, and break lines overhang the geometry they
/// mark by this sheet-space constant — never scaled.
pub(crate) const CENTER_MARK_OVERHANG: f64 = 4.0;

// ── The cutting-plane anatomy [SPEC 15.8] — baked sheet constants.
/// The chain line runs past the geometry by this on each end — a plane-line
/// overshoot, a different concept from the centre-mark overhang.
pub(crate) const PLANE_OVERHANG: f64 = 6.0;
/// The thick end stroke's length and (geometry) weight.
pub(crate) const PLANE_THICK_END: f64 = 10.0;
pub(crate) const PLANE_THICK_WIDTH: f64 = 2.0;
/// The viewing arrow's shaft, from the line end out along the sight line.
pub(crate) const PLANE_ARROW_SHAFT: f64 = 13.0;
/// The section letter, just past each arrow.
pub(crate) const PLANE_LETTER_GAP: f64 = 7.0;
pub(crate) const PLANE_LETTER_SIZE: f64 = 12.0;

// ── ISO metric 60° thread depths per side, as fractions of the pitch
// [SPEC 15.3/15.4]: external `h3 = d − 1.2269 × P` (major to root), internal
// `H1 = 0.54125 × P` (drill to major).
pub(crate) const THREAD_DEPTH: f64 = 0.61343;
pub(crate) const THREAD_DEPTH_INTERNAL: f64 = 0.54125;

// ── The drafting hatch tile [SPEC 10.3].
/// Default pitch, sheet-space px.
pub(crate) const HATCH_PITCH: f64 = 6.0;
/// The texture's fixed line width — a texture, not a stroke.
pub(crate) const HATCH_LINE_WIDTH: f64 = 0.75;

// ── A drawing scope's links [SPEC 15.1, 10.5]: geometry keeps stroke 2, the
// annotation wires thin to 1 and their text reads at the caption size.
pub(crate) const DRAWING_LINK_STROKE_WIDTH: f64 = 1.0;
pub(crate) const DRAWING_LINK_FONT_SIZE: f64 = 12.0;

// ── Cross-file baked defaults [SPEC 10.5].
/// The baked `clearance` — cascaded onto every link by the link bundle, so
/// per-site fallbacks are unreachable; they still agree here.
pub(crate) const DEFAULT_CLEARANCE: f64 = 16.0;
/// The baked root `font-size` (body text).
pub(crate) const ROOT_FONT_SIZE: f64 = 15.0;
/// The default ISO 5457 sheet — A4 portrait, mm [SPEC 15.8].
pub(crate) const A4: (f64, f64) = (210.0, 297.0);

// ── Look tunables.
/// Multi-line leading: lines stack at `font-size × 1.2` [SPEC 5] —
/// measurement (layout) and emission (render) must agree.
pub(crate) const TEXT_LEADING: f64 = 1.2;
/// The wavy stroke's shape [SPEC 7], world units, tuned against the default
/// clearance: the wavelength reads as a clear wiggle and the amplitude stays
/// well under a corner's fillet radius, so the wave never touches itself on
/// the inside of a turn (the label cut widens its mask by the amplitude).
pub(crate) const WAVY_WAVELENGTH: f64 = 12.0;
pub(crate) const WAVY_AMPLITUDE: f64 = 1.4;
/// A `natural` curve's control-point pull (ROUTING.md The natural strategy):
/// the fraction of each spline span's chord used as the tangent handle
/// length. One number, no user-facing knob — tuned by eye against rendered
/// mindmaps.
pub(crate) const NATURAL_PULL: f64 = 0.5;
/// A `natural` wire's dodge budget (ROUTING.md The natural strategy):
/// escalation rounds on the one body a wire may detour before it falls
/// back to its smooth direct fit and reports what it crosses. Part of the
/// routing contract, like the Law-3 cost constants.
pub(crate) const DODGE_ROUNDS: usize = 6;
/// The note dog-ear [SPEC 8]: fold size as a height fraction, capped.
pub(crate) const NOTE_FOLD_FRAC: f64 = 0.34;
pub(crate) const NOTE_FOLD_MAX: f64 = 15.0;
/// ISO 5457 sheet furniture, mm: the frame margin, and the wider filing edge
/// on the left [SPEC 15.8].
pub(crate) const SHEET_MARGIN: f64 = 10.0;
pub(crate) const SHEET_FILING: f64 = 20.0;

/// The absurd-rendered-extent hint threshold [SPEC 20]: a drawing wider or
/// taller than this many px almost certainly authored a magnitude into
/// `scale:` — the hint names the ratio fix.
pub const ABSURD_EXTENT_PX: f64 = 10_000.0;
