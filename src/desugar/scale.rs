//! The scale fold [SPEC 15.1/18]: a drawing scope's `scale:` (the drafting
//! **ratio**, default 1) × `unit:` (mm per drawing unit — `mm`/`cm`/`m`/`in`,
//! nearest-wins, default mm) × the root `density:` (px per mm, default 4)
//! become one generated internal **`px-per-unit:`** — the engine's existing
//! multiplier — so the layout core stays dumb and `lini desugar` shows the
//! number. A `|page|` folds the density alone (mm paper, ratio locked at 1 —
//! a page carries no `scale:` of its own [SPEC 15.8]). The pass recomputes
//! from the same authored inputs every time, so desugar stays idempotent.
//!
//! The fold reads **authored decls** (and the worn `.lini-*` classes of an
//! already-lowered file); a rule-borne `scale:` stays what it reaches the
//! engine as — a raw multiplier.

use super::nest::{in_drawing_scope, is_drawing_body, is_page_body, is_stack_body};
use super::schematic::lowered_chain;
use super::types;
use crate::error::Error;
use crate::span::Span;
use crate::syntax::ast::{Child, Decl, Value, decl_of};

/// The generated internal attr name [SPEC 19] — whitelisted in validation.
pub(crate) const PX_PER_UNIT: &str = "px-per-unit";

// ── The floorplan's true-size stamps [SPEC 15.11] ──
//
// A floorplan type's intrinsic sizes are physical millimetres read through the
// scope's `unit:`, and this walk is the **only** place that unit is known
// (nearest-wins, pages included) — so it stamps what the layout readers need.
// Two stamps, and the split is the one question "can the walk resolve this?":
//
// - `unit-mm:` carries the **input**, for a size the walk cannot resolve: a
//   fixture body's millimetres come from the `symbol:` variant and an opening's
//   from its type, then `width:` / `height:` stretch the result — all cascade,
//   all past this walk. The reader converts, through [`mm_to_units`].
// - `wall-thickness:` carries a **resolved value**, for the one size that is
//   not on the node at all: `thickness:` inherits nearest-wins from the scope
//   ([SPEC 17] `Inherit::Engine`, resolve carries no such channel), so only the
//   walk can say what a wall without its own value falls back to.

/// The generated internal attr carrying the scope's **millimetres per drawing
/// unit**, for every reader of a true-size default — the openings and the
/// fixtures. A wall takes the resolved stamp below instead, so it needs none.
pub(crate) const UNIT_MM: &str = "unit-mm";

/// The generated internal attr carrying a wall's **resolved fallback**
/// `thickness:` in drawing units — what the wall reads when no cascaded
/// `thickness:` reaches it.
pub(crate) const WALL_THICKNESS: &str = "wall-thickness";

/// The true-size wall defaults [SPEC 15.11] — physical millimetres, the
/// reader's (never a class-rule literal; see `ledger::defaults`).
pub(crate) const WALL_MM: f64 = 200.0;
const PARTITION_MM: f64 = 100.0;

/// What one drawing unit **is** [SPEC 15.1]. A drafting scope measures in
/// physical millimetres and converts through `density:`; a plain `stack` draws
/// in pixels, where the conversion is the identity and `density:` has no
/// millimetres to act on.
#[derive(Clone, Copy)]
pub(crate) enum Unit {
    Mm(f64),
    Px,
}

impl Unit {
    /// Pixels per drawing unit at ratio 1 — the number the fold stamps.
    fn px_per_unit(self, density: f64) -> f64 {
        match self {
            Unit::Mm(mm) => mm * density,
            Unit::Px => 1.0,
        }
    }

    /// Millimetres per drawing unit, for the floorplan's true-size defaults.
    /// Pixel space has no physical size, so it reads as the identity — those
    /// defaults only arise in a floorplan, which always measures physically.
    fn mm(self) -> f64 {
        match self {
            Unit::Mm(mm) => mm,
            Unit::Px => 1.0,
        }
    }
}

/// The unit / density context carried down the lowered tree.
struct ScaleCtx {
    density: f64,
    /// The nearest **authored** `unit:`, which inherits nearest-wins
    /// [SPEC 15.1]. Distinct from the effective one below, because a default
    /// must not inherit: a `|drawing|` that states no unit measures in
    /// millimetres even inside a pixel-space `stack`, while a `unit: cm`
    /// written above reaches both.
    authored: Option<Unit>,
    /// What one drawing unit is **in this scope** — the authored value, or
    /// the scope kind's own default.
    unit: Unit,
    in_drawing: bool,
    /// The nearest authored floorplan-scope `thickness:` (drawing units) —
    /// the inherited slot a wall without its own value falls back to
    /// [SPEC 15.11].
    thickness: Option<f64>,
}

/// Fold the whole lowered scene. `root_layout` is the root's own `layout:`: a
/// datum root [SPEC 12] — a `stack` or any of the drawing family — is itself a
/// scope and gets the stamp. A plain stack measures in pixels, a drawing in
/// millimetres; anything else keeps millimetres so a *nested* drawing inherits
/// the usual default.
pub(super) fn fold(
    instances: &mut [Child],
    user_root: &mut Vec<Decl>,
    root_layout: Option<&str>,
) -> Result<(), Error> {
    let opens = root_layout.is_some_and(crate::resolve::is_stack_layout);
    let drafts = root_layout.is_some_and(crate::resolve::is_drawing_layout);
    let density = read_density(user_root)?;
    let authored = read_unit(user_root)?;
    // A root that opens no datum scope of its own holds millimetres, so a
    // *nested* drawing inherits the usual default.
    let unit = authored.unwrap_or(default_unit(drafts || !opens));
    let ctx = ScaleCtx {
        density,
        authored,
        unit,
        in_drawing: drafts,
        thickness: read_thickness(user_root),
    };
    if opens {
        stamp(user_root, &ctx);
    }
    for c in instances.iter_mut() {
        walk(c, &ctx)?;
    }
    Ok(())
}

fn walk(child: &mut Child, ctx: &ScaleCtx) -> Result<(), Error> {
    let Child::Box(n) = child else { return Ok(()) };
    let mut ctx = ScaleCtx {
        density: ctx.density,
        authored: ctx.authored,
        unit: ctx.unit,
        in_drawing: ctx.in_drawing,
        thickness: ctx.thickness,
    };
    let chain = lowered_chain(n);
    // Any datum scope carries a scale [SPEC 12]; only the drawing half drafts.
    let opens = is_stack_body(&chain, &n.style);
    if is_page_body(&chain) {
        if let Some(d) = decl_of(&n.style, "scale") {
            return Err(Error::at(
                d.span,
                "a '|page|' carries no 'scale:' — 'density:' sets its pixels per millimetre (root), a drawing's 'scale:' its drafting ratio",
            ));
        }
        if let Some(u) = read_unit(&n.style)? {
            ctx.authored = Some(u);
            ctx.unit = u;
        }
        // Paper is millimetres: px-per-unit is the density alone.
        n.style.retain(|d| d.name != PX_PER_UNIT);
        n.style.push(number_decl(ctx.density, n.span));
    } else if opens {
        if let Some(u) = read_unit(&n.style)? {
            ctx.authored = Some(u);
        }
        // An authored unit inherits; a default does not [SPEC 15.1] — so a
        // plain `stack` measures in pixels and a drawing in millimetres,
        // each regardless of what kind of scope encloses it.
        ctx.unit = ctx
            .authored
            .unwrap_or(default_unit(is_drawing_body(&chain, &n.style)));
        if let Some(t) = read_thickness(&n.style) {
            ctx.thickness = Some(t);
        }
        stamp(&mut n.style, &ctx);
    }
    if ctx.in_drawing {
        stamp_wall_thickness(&mut n.style, &ctx, &chain, n.span);
        stamp_unit_mm(&mut n.style, &ctx, &chain, n.span);
    }
    ctx.in_drawing = in_drawing_scope(opens, ctx.in_drawing, &chain, &n.style);
    for c in &mut n.children {
        walk(c, &ctx)?;
    }
    Ok(())
}

/// Replace any prior stamp and push the scope's `px-per-unit = unit-mm ×
/// density` — its pixels per drawing unit **at ratio 1**. The drafting ratio
/// is not folded in: `scale:` is an ordinary node property and the cascade
/// owns it ([SPEC 15.1] — an element or id rule reaches a drawing exactly as
/// its own block does, and a node states it against its scope's stamp), so it
/// multiplies once, where the engine reads the pair
/// ([`crate::layout::effective_scale`]).
fn stamp(style: &mut Vec<Decl>, ctx: &ScaleCtx) {
    style.retain(|d| d.name != PX_PER_UNIT);
    style.push(number_decl(
        ctx.unit.px_per_unit(ctx.density),
        Span::empty(),
    ));
}

/// **The** physical-millimetre → drawing-unit conversion [SPEC 15.11]: a
/// floorplan type's true-size default — a wall's 200 mm thickness, a door's
/// 900 mm clear width, every fixture body — is stated in physical millimetres
/// and read through the scope's own `unit:` (`unit_mm` millimetres to the
/// drawing unit, [`read_unit`]), so a bed is 1500 × 2000 mm whether the file
/// drafts in `m` or `mm`. An **authored** value is drawing units like
/// everything else and never passes through here.
///
/// It lives beside the `unit:` reader because that is where the scope's
/// millimetres-per-unit is known; every true-size consumer calls this one
/// function.
pub(crate) fn mm_to_units(mm: f64, unit_mm: f64) -> f64 {
    mm / unit_mm
}

/// Stamp a wall's resolved fallback thickness [SPEC 15.11], nearest-wins:
/// the wall's own authored `thickness:` needs no stamp (it is the nearest
/// value, and the cascade already carries it); a `|partition|`'s 100 mm is
/// its define's value — **at** the node, so it beats the scope's inherited
/// slot exactly as the SPEC 8 bundle would ([SPEC 8]: "a define, nothing
/// more"); then the nearest scope-authored value; then the 200 mm default.
/// The mm defaults convert through the scope's `unit:` here — the true-size
/// law — while authored values pass untouched (they are drawing units).
/// Recomputed from the same authored inputs every walk, so desugar stays
/// idempotent; rule-borne `thickness:` is resolve's to cascade and wins over
/// this stamp at the read site (`layout::floorplan::wall`).
fn stamp_wall_thickness(style: &mut Vec<Decl>, ctx: &ScaleCtx, chain: &[String], span: Span) {
    style.retain(|d| d.name != WALL_THICKNESS);
    if !chain.iter().any(|t| t == "wall") || decl_of(style, "thickness").is_some() {
        return;
    }
    let units = if chain.iter().any(|t| t == "partition") {
        mm_to_units(PARTITION_MM, ctx.unit.mm())
    } else {
        ctx.thickness
            .unwrap_or_else(|| mm_to_units(WALL_MM, ctx.unit.mm()))
    };
    style.push(Decl {
        name: WALL_THICKNESS.into(),
        groups: vec![vec![Value::Number(units)]],
        span,
    });
}

/// Stamp the scope's `unit:` on every node that reads a true-size default at
/// layout [SPEC 15.11] — the openings (900 mm / 1200 mm clear by type) and the
/// fixtures (a body in millimetres picked by the cascaded `symbol:`, then
/// stretched to a cascaded `width:` / `height:`). None of those is a size this
/// walk could resolve, so what travels is the unit and
/// [`crate::layout::floorplan::true_size`] converts with it.
fn stamp_unit_mm(style: &mut Vec<Decl>, ctx: &ScaleCtx, chain: &[String], span: Span) {
    style.retain(|d| d.name != UNIT_MM);
    let reads_true_size = types::OPENINGS
        .iter()
        .chain(types::FIXTURES)
        .any(|t| chain.iter().any(|c| c == t));
    if !reads_true_size {
        return;
    }
    style.push(Decl {
        name: UNIT_MM.into(),
        groups: vec![vec![Value::Number(ctx.unit.mm())]],
        span,
    });
}

/// The nearest authored `thickness:` in a scope's own decls — drawing units.
/// A malformed value is validation's to report; the walk just declines it.
fn read_thickness(style: &[Decl]) -> Option<f64> {
    match decl_of(style, "thickness").and_then(Decl::single) {
        Some(Value::Number(n)) if *n > 0.0 => Some(*n),
        _ => None,
    }
}

/// The nearest authored `unit:` as millimetres per drawing unit [SPEC 15.1].
/// Only the fold's own scopes (root, pages, drawings) are read, so an
/// `|axis|`'s quoted tick suffix never meets this enum.
fn read_unit(style: &[Decl]) -> Result<Option<Unit>, Error> {
    let Some(d) = decl_of(style, "unit") else {
        return Ok(None);
    };
    let unit = match d.single() {
        Some(Value::Ident(u)) => match u.as_str() {
            "mm" => Some(Unit::Mm(1.0)),
            "cm" => Some(Unit::Mm(10.0)),
            "m" => Some(Unit::Mm(1000.0)),
            "in" => Some(Unit::Mm(25.4)),
            // Not a physical size — one unit is one pixel, so `density:` has
            // no millimetres to convert [SPEC 15.1].
            "px" => Some(Unit::Px),
            _ => None,
        },
        _ => None,
    };
    unit.map(Some)
        .ok_or_else(|| Error::at(d.span, "'unit' is px, mm, cm, m, or in"))
}

/// A scope kind's own unit when none was authored above it [SPEC 12/15.1]:
/// a drafting scope measures in millimetres, a plain `stack` in pixels.
fn default_unit(drafts: bool) -> Unit {
    if drafts { Unit::Mm(1.0) } else { Unit::Px }
}

/// The root `density:` — px per mm, default 4, must be positive [SPEC 15.1].
fn read_density(user_root: &[Decl]) -> Result<f64, Error> {
    let Some(d) = decl_of(user_root, "density") else {
        return Ok(4.0);
    };
    match d.single() {
        Some(Value::Number(n)) if *n > 0.0 => Ok(*n),
        _ => Err(Error::at(d.span, "'density' must be > 0")),
    }
}

fn number_decl(v: f64, span: Span) -> Decl {
    Decl {
        name: PX_PER_UNIT.into(),
        groups: vec![vec![Value::Number(v)]],
        span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The true-size law [SPEC 15.11]: a physical millimetre default reads the
    /// same size at every `unit:` — 200 mm is `200` drafting in mm, `0.2`
    /// drafting in m, `20` in cm — while an authored value stays drawing units
    /// and never meets this function.
    #[test]
    fn a_true_size_default_converts_through_the_scope_unit() {
        let mm_per_unit = |src: &str| {
            read_unit(&[Decl {
                name: "unit".into(),
                groups: vec![vec![Value::Ident(src.into())]],
                span: Span::empty(),
            }])
            .expect("a known unit")
            .expect("a value")
            .mm()
        };
        assert_eq!(mm_to_units(200.0, mm_per_unit("mm")), 200.0);
        assert_eq!(mm_to_units(200.0, mm_per_unit("cm")), 20.0);
        assert_eq!(mm_to_units(200.0, mm_per_unit("m")), 0.2);
        // The scope default: no `unit:` at all is millimetres [SPEC 15.1].
        assert_eq!(mm_to_units(900.0, 1.0), 900.0);
    }

    /// The wall's fallback stamp [SPEC 15.11], nearest-wins: an authored
    /// value on the wall suppresses it; a `|partition|`'s 100 mm define beats
    /// the scope's inherited slot; the scope's value beats the 200 mm
    /// default; the mm defaults convert through the scope's `unit:`.
    #[test]
    fn the_wall_thickness_stamp_resolves_nearest_wins() {
        let ctx = |unit_mm: f64, thickness: Option<f64>| ScaleCtx {
            density: 4.0,
            authored: Some(Unit::Mm(unit_mm)),
            unit: Unit::Mm(unit_mm),
            in_drawing: true,
            thickness,
        };
        let stamped = |style: &mut Vec<Decl>, ctx: &ScaleCtx, chain: &[&str]| {
            let chain: Vec<String> = chain.iter().map(|s| s.to_string()).collect();
            stamp_wall_thickness(style, ctx, &chain, Span::empty());
            style
                .iter()
                .find(|d| d.name == WALL_THICKNESS)
                .and_then(|d| match d.single() {
                    Some(Value::Number(n)) => Some(*n),
                    _ => None,
                })
        };
        let wall = ["wall", "sketch"];
        let partition = ["partition", "wall", "sketch"];

        let mut s = Vec::new();
        assert_eq!(
            stamped(&mut s, &ctx(1000.0, None), &wall),
            Some(0.2),
            "the 200 mm default, through unit: m"
        );
        let mut s = Vec::new();
        assert_eq!(
            stamped(&mut s, &ctx(1000.0, Some(0.5)), &wall),
            Some(0.5),
            "the scope's authored value, drawing units untouched"
        );
        let mut s = Vec::new();
        assert_eq!(
            stamped(&mut s, &ctx(1000.0, Some(0.5)), &partition),
            Some(0.1),
            "the partition define is at the node — it beats the scope"
        );
        let mut s = vec![Decl {
            name: "thickness".into(),
            groups: vec![vec![Value::Number(2.0)]],
            span: Span::empty(),
        }];
        assert_eq!(
            stamped(&mut s, &ctx(1000.0, Some(0.5)), &wall),
            None,
            "an authored wall value needs no fallback"
        );
    }
}
