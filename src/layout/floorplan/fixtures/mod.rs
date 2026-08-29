//! The furniture library [SPEC 15.11] — the six symbol-bodied fixture types.
//!
//! A fixture is **geometry**, not annotation: its body is authored on a
//! physical-millimetre grid ([`shape`]) and carried into the view at the
//! scope's own `unit:` and `scale:`, so a bed is 1500 × 2000 mm whether the
//! file drafts in `m` or `mm`. `symbol:` picks the variant (the first row of
//! its family is the default, the discretes' table shape [SPEC 16.3]);
//! `width:` / `height:` are floors and the body **stretches** to the box they
//! resolve — one anisotropic factor per axis, every family alike.
//!
//! The whole body is **one path on the fixture's own node**: the type's class
//! rule paints it (`fill: --bg` masks what it overlaps, a 1 px `--stroke-dark`
//! outline), so a symbol needs no generated children and no class of its own.
//! `|stairs|` is the exception the SPEC states — its treads and up arrow are
//! generated chrome, counted at desugar and filled here.

use super::super::ir::{Bbox, PlacedNode};
use crate::error::Error;
use crate::layout::drawing::geometry::n;
use crate::resolve::{NodeKind, ResolvedInst, ResolvedValue};
use shape::Sym;

mod draw;
mod shape;

/// A fixture's lowered body — everything the sizing pass and the fill-in pass
/// downstream of it need, measured once.
pub(in crate::layout) struct Body {
    /// The node's box: the drawn body plus its half-stroke.
    pub(in crate::layout) bbox: Bbox,
    /// The symbol as path data in the node's own frame, pixels.
    d: String,
    /// The drawn body, stroke excluded.
    size: (f64, f64),
    /// Pixels per millimetre on each axis — the stretch, for the chrome that
    /// is drawn after the body is sized.
    px: (f64, f64),
    /// `|stairs|`' tread count; `None` for every other family.
    steps: Option<f64>,
    /// Whether the smart label centres **in** the body [SPEC 15.11] — the
    /// `|appliance|` labelled-box convention; every other fixture reads its
    /// label beside the body, like a discrete's value.
    inside: bool,
    /// The fixture's own turn — what its label takes back to stay readable.
    rot: f64,
}

/// Lay out a fixture's body [SPEC 15.11], or `None` for a node that is not
/// one. `own` is the pixels per drawing unit the fixture draws at.
pub(in crate::layout) fn plan(inst: &ResolvedInst, own: f64) -> Result<Option<Body>, Error> {
    let Some(ty) = family(&inst.type_chain) else {
        return Ok(None);
    };
    // The mm grid into pixels: one drawing unit per millimetre through the
    // scope's own `unit:`, the shared true-size reader [SPEC 15.11].
    let per_mm = super::true_size(&inst.attrs, 1.0) * own;
    let steps = (ty == "stairs").then(|| inst.attrs.number("steps").unwrap_or(2.0));
    let sym = symbol(inst, ty, steps)?;
    let (w0, h0) = (sym.extent.0 * per_mm, sym.extent.1 * per_mm);
    // `width:` / `height:` are floors [SPEC 5]; the body stretches to whatever
    // box they resolve, so the two axes carry their own factor.
    let floor = |name: &str| inst.attrs.number(name).unwrap_or(0.0) * own;
    let (w, h) = (floor("width").max(w0), floor("height").max(h0));
    let px = (per_mm * w / w0, per_mm * h / h0);
    Ok(Some(Body {
        bbox: Bbox::centered(w, h).inflate(inst.attrs.half_stroke()),
        d: sym.d(px.0, px.1),
        size: (w, h),
        px,
        steps,
        inside: ty == "appliance",
        rot: inst.attrs.number("rotate").unwrap_or(0.0),
    }))
}

/// Seat what the body could not decide until it was sized: `|stairs|`' tread
/// and arrow chrome, and the smart label — beside the body like a discrete's
/// value [SPEC 16.3] (the shared seat, [`super::label`]), or centred in it for
/// an `|appliance|` [SPEC 15.11].
pub(in crate::layout) fn finish(children: &mut [PlacedNode], body: &Body) {
    if let Some(steps) = body.steps {
        flight(children, body, steps);
    }
    // An `|appliance|`'s label keeps the `|block|`'s centred seat and only
    // turns upright; every other fixture's reads beside the body.
    if body.inside {
        super::label::upright(children, body.rot);
    } else {
        super::label::seat(children, body.size.1 / 2.0, 1.0, body.rot);
    }
}

/// The node's drawn path and the kind it draws as — a fixture body is one
/// `|path|`, so the type's own class rule is all the paint it needs.
pub(in crate::layout) fn paint(node: &mut PlacedNode, body: Body) {
    node.kind = NodeKind::Path;
    node.attrs.insert("path", ResolvedValue::String(body.d));
}

/// The fixture family a chain wears — the name the variant table keys on.
fn family(chain: &[String]) -> Option<&'static str> {
    crate::desugar::types::FIXTURES
        .iter()
        .find(|f| chain.iter().any(|t| t == *f))
        .copied()
}

/// A family's variants [SPEC 15.11], **first row the default** — the discrete
/// symbol table's shape ([`crate::desugar::schematic`]). The size is the SPEC
/// body in millimetres; what a symbol actually occupies is its own to say (a
/// dining set's chairs push the extent past the tabletop).
struct Variant {
    name: &'static str,
    size: (f64, f64),
}

fn variants(ty: &str) -> &'static [Variant] {
    macro_rules! v {
        ($($n:literal $w:literal $h:literal),+ $(,)?) => {
            &[$(Variant { name: $n, size: ($w, $h) }),+]
        };
    }
    match ty {
        "bed" => {
            v!("queen" 1500.0 2000.0, "king" 1800.0 2000.0, "double" 1350.0 1900.0, "single" 900.0 2000.0)
        }
        "sofa" => {
            v!("three" 2200.0 900.0, "two" 1600.0 900.0, "one" 900.0 900.0, "corner" 2400.0 2400.0, "stool" 350.0 350.0)
        }
        "dining" => v!("six" 1800.0 900.0, "four" 1200.0 800.0, "round" 1000.0 1000.0),
        "bath" => {
            v!("tub" 1700.0 750.0, "shower" 900.0 900.0, "toilet" 700.0 400.0, "sink" 500.0 400.0, "double-sink" 800.0 450.0)
        }
        "appliance" => {
            v!("stove" 600.0 600.0, "fridge" 600.0 600.0, "washer" 600.0 600.0, "dishwasher" 600.0 600.0)
        }
        // `|stairs|` takes no `symbol:` [SPEC 17] — `steps:` sizes the flight.
        _ => &[],
    }
}

/// The symbol a fixture draws: the variant `symbol:` names, else the family
/// default. An unknown name errors with its family's variants spelled out —
/// the discretes' wording, through the one shared builder.
fn symbol(inst: &ResolvedInst, ty: &str, steps: Option<f64>) -> Result<Sym, Error> {
    if let Some(steps) = steps {
        return Ok(draw::stairs(steps));
    }
    let want = match inst.attrs.get("symbol") {
        Some(ResolvedValue::Ident(s)) => Some(s.as_str()),
        _ => None,
    };
    let set = variants(ty);
    let v = match want {
        None => set.first(),
        Some(name) => set.iter().find(|v| v.name == name),
    }
    .ok_or_else(|| {
        Error::at(
            inst.span,
            crate::suggest::unknown_symbol(
                want.unwrap_or_default(),
                ty,
                set.iter().map(|v| v.name),
            ),
        )
    })?;
    Ok(draw::symbol(ty, v.name, v.size))
}

/// `|stairs|`' generated chrome [SPEC 15.7/15.11], filled once the flight is
/// sized: the treads across it, and the up arrow from the first tread past the
/// last. The count is desugar's ([`crate::desugar::drawing`]); the geometry is
/// the flight's, so it is drawn here.
fn flight(children: &mut [PlacedNode], body: &Body, steps: f64) {
    let (w, run) = body.size;
    let pitch = run / steps;
    let (x0, x1) = (-w / 2.0, w / 2.0);
    for c in children.iter_mut() {
        let Some((kind, i)) = crate::layout::drawing::chrome::indexed(&c.attrs) else {
            continue;
        };
        match kind.as_str() {
            // The risers between the treads — the flight's own outline draws
            // the two ends, so only the interior divisions are chrome.
            "tread" => {
                let y = run / 2.0 - (i + 1.0) * pitch;
                let pt = |p: (f64, f64)| {
                    ResolvedValue::Tuple(vec![
                        ResolvedValue::Number(p.0),
                        ResolvedValue::Number(p.1),
                    ])
                };
                c.attrs.insert(
                    "points",
                    ResolvedValue::List(vec![pt((x0, y)), pt((x1, y))]),
                );
                c.bbox = Bbox::from_points(&[(x0, y), (x1, y)]).inflate(c.attrs.half_stroke());
            }
            // The direction arrow: up the flight's middle from the first
            // tread, its head landing on the far edge.
            "arrow" => {
                let (foot, tip) = (run / 2.0 - pitch / 2.0, -run / 2.0);
                let (hw, hl) = (ARROW_HEAD_MM * body.px.0, ARROW_HEAD_MM * body.px.1);
                c.attrs.insert(
                    "path",
                    ResolvedValue::String(format!(
                        "M 0 {} L 0 {} M {} {} L 0 {} L {} {}",
                        n(foot),
                        n(tip),
                        n(-hw),
                        n(tip + hl),
                        n(tip),
                        n(hw),
                        n(tip + hl)
                    )),
                );
                c.kind = NodeKind::Path;
                c.bbox =
                    Bbox::from_points(&[(-hw, tip), (hw, foot)]).inflate(c.attrs.half_stroke());
            }
            _ => {}
        }
    }
}

/// The up arrow's head, millimetres — half its width and its length, so it
/// reads at 1:50 without ever competing with a tread line.
const ARROW_HEAD_MM: f64 = 110.0;
