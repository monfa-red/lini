//! Shared chrome / look constants [SPEC 10.5] — the drawing chrome set, the
//! look tunables, and the cross-file baked fallbacks, in one home so the
//! whole look is tuned from this module.

// ── The dimension / leader anatomy [SPEC 15.6/15.7] — baked sheet constants,
// never scaled by the view.
/// The drawing scope's `clearance` default for its dimensions [SPEC 15.6] —
/// pushed into the link base beside the thin stroke, below every user rule.
/// Row offsets derive from painted bounds + clearance; 5 stands a first
/// bottom row's value text 5 off the geometry — a quarter more air than the
/// 4 this started at, which drafted too cramped by eye.
pub(crate) const DIM_CLEARANCE: f64 = 5.0;
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
/// A crossing halo's clearance each side of the crossed geometry line
/// [SPEC 15.7] — the sheet-space knockout that breaks annotation linework
/// where it crosses geometry (2 = the drawing linework width doubled).
pub(crate) const HALO_MARGIN: f64 = 2.0;

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
/// Chrome text scales with the inherited body size [SPEC 6]: a link label
/// reads 11 and a caption 12 at the default 15, each derived as
/// `inherited × N / 15` (multiply before divide — exact at the default).
pub(crate) const LINK_FONT_AT_ROOT: f64 = 11.0;
pub(crate) const CAPTION_FONT_AT_ROOT: f64 = 12.0;
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
/// ISO 5457 sheet furniture, mm: the frame's margin from the trimmed edge
/// [SPEC 15.8].
pub(crate) const SHEET_MARGIN: f64 = 10.0;

// ── The schematic chrome [SPEC 10.5/16] — sheet-space baked constants;
// Phase 6's visual pass tunes them against the reference sheet.
/// Pin centre-to-centre spacing — a pin row's height, so rows stacking at
/// gap 0 land on exact pitch centres. Must stay ≥ the router's min pitch at
/// the scope's clearance [SPEC 16.5].
pub(crate) const PIN_PITCH: f64 = 20.0;
/// The stub — the short lead a pin extends outward; the wire lands on its tip.
pub(crate) const PIN_STUB: f64 = 20.0;
/// The junction dot's radius [SPEC 16.5].
pub(crate) const JUNCTION_RADIUS: f64 = 4.0;
/// The room a shaped tag's point is given [SPEC 16.4] — the flag's nose,
/// reserved by the class rule on the pointed side alone. The nose itself
/// draws at half the tag's height (a 45° point at any text size) and clamps
/// here, so the text never rides it.
pub(crate) const TAG_POINT: f64 = 8.0;
/// The clear space a **net label**'s text keeps off the trace it names
/// [SPEC 16.4]: a sheet writes the net name *beside* the line, never on it, so
/// this is the daylight between the wire's centreline and the nearest edge of
/// the text. It holds the text off the run's two ends as well — one constant,
/// one meaning: a net label's clear space.
pub(crate) const NET_LABEL_OFFSET: f64 = 4.0;
/// The floor on a plain net label's **run** [SPEC 16.4] — the length of trace
/// its text names, two pin pitches, so a short name still gets a readable
/// stretch of wire under it. A longer name grows it, and `width:` raises the
/// floor, through SPEC 5's ordinary width law.
pub(crate) const NET_LABEL_RUN: f64 = 2.0 * PIN_PITCH;
/// The satellite seat gap [SPEC 16.1/10.5] — the clear run between a pin's
/// stub tip and the satellite seated off it, and between stacked satellites.
///
/// It is a **routing corridor**, not just daylight: the lead between a pin
/// and its satellite is an ordinary routed wire, and the channel model gives
/// it a cell only where the two keep-outs do not overlap — so the seat must
/// clear `2 × SCH_CLEARANCE`. SPEC 10.5's 10 leaves no channel at the
/// schematic's own clearance 10 and every lead strays; one pin pitch does,
/// which also puts satellites on the sheet grid. (The same reasoning
/// widened the tree's `gap` past SPEC's plain 36.)
pub(crate) const LABEL_SEAT: f64 = 25.0;
/// The schematic scope's link `clearance` [SPEC 10.5/16.6] — tighter than the
/// routing 16, so a sheet's short leads and pin pitch have room. Cascades
/// from the scope's own block, so a user's `clearance:` still wins.
pub(crate) const SCH_CLEARANCE: f64 = 10.0;
/// Part linework weight — symbol bodies, stubs, tag outlines [SPEC 16.6].
pub(crate) const SCH_STROKE_WIDTH: f64 = 1.5;
/// The pin-number readout, outside beside the stub [SPEC 16.2].
pub(crate) const PIN_NUMBER_FONT: f64 = 10.0;
/// How far the pin number sits off its lead, across the stub: above a
/// horizontal one, beside a vertical one [SPEC 16.2].
pub(crate) const PIN_NUMBER_OFFSET: f64 = 9.0;
/// The ref / value readout text size [SPEC 16.2] — also the line height a
/// readout's seat adds back, since `pin:` aligns edges (a single line measures
/// one em, [`crate::layout::text::approx_height`]).
pub(crate) const REF_FONT: f64 = 12.0;
/// The clear gap between a part's drawing and the readout naming it, and
/// between the two readouts where they stack [SPEC 16.2].
pub(crate) const READOUT_GAP: f64 = 8.0;
pub(crate) const READOUT_STACK: f64 = 4.0;
/// How far a **turned** part's ref / value readouts sit beside its axis
/// [SPEC 16.2] — clear of the symbol *and* of the wire's own corridor running
/// down through it, so a typical value (up to ~5 characters) never blocks a
/// landing. A longer one is `translate:`'s to move.
pub(crate) const READOUT_OFFSET: f64 = 40.0;

/// The absurd-rendered-extent hint threshold [SPEC 21]: a drawing wider or
/// taller than this many px almost certainly authored a magnitude into
/// `scale:` — the hint names the ratio fix.
pub const ABSURD_EXTENT_PX: f64 = 10_000.0;
