//! Every fixture symbol, drawn on the millimetre grid [SPEC 15.11].
//!
//! **The taste rule is minimal**: the fewest strokes that still read at 1:50,
//! settled against two real condominium plans rather than the symbol charts —
//! a stove is a square and four circles, a fridge / washer / dishwasher are
//! near-plain boxes whose *letter is the smart label*, never baked in here.
//! No upholstery lines, no faucet handles, no burner grates.
//!
//! Each family draws around the fixture's own origin, so the body is centred
//! and `rotate:` turns it about its middle. The **body outline comes first**:
//! it is the shape whose `--bg` fill masks the floor under the furniture.

use super::shape::{Shape, Sym, box_at};

/// The variant's drawing. `size` is its SPEC body in millimetres — a tabletop
/// for `|dining|`, the piece itself everywhere else.
pub(super) fn symbol(ty: &str, variant: &str, size: (f64, f64)) -> Sym {
    match ty {
        "bed" => bed(size, variant == "single"),
        "sofa" if variant == "corner" => corner_sofa(size),
        "sofa" => sofa(size),
        "dining" if variant == "round" => round_table(size),
        "dining" => dining(size, if variant == "four" { 2 } else { 3 }),
        "bath" => bath(variant, size),
        _ => appliance(variant, size),
    }
}

/// A bed: the mattress, its pillow(s) at the head, and the turned-down sheet
/// across it — the read every plan uses, in three strokes.
fn bed((w, h): (f64, f64), single: bool) -> Sym {
    const MARGIN: f64 = 80.0;
    const PILLOW: f64 = 350.0;
    const TURNDOWN: f64 = 400.0;
    let (top, cy) = (-h / 2.0 + MARGIN, -h / 2.0 + MARGIN + PILLOW / 2.0);
    let mut shapes = vec![Shape::Rect(-w / 2.0, -h / 2.0, w / 2.0, h / 2.0)];
    if single {
        shapes.push(box_at(0.0, cy, w - 2.0 * MARGIN, PILLOW));
    } else {
        let pw = (w - 3.0 * MARGIN) / 2.0;
        let off = (pw + MARGIN) / 2.0;
        shapes.push(box_at(-off, cy, pw, PILLOW));
        shapes.push(box_at(off, cy, pw, PILLOW));
    }
    let fold = top + PILLOW + TURNDOWN;
    shapes.push(Shape::Line(vec![(-w / 2.0, fold), (w / 2.0, fold)]));
    Sym::new((w, h), shapes)
}

/// A straight sofa: the outline, and one inner run tracing arm → back → arm.
/// Two strokes; the cushion divisions the charts draw read as clutter at 1:50.
const SOFA_ARM: f64 = 200.0;

fn sofa((w, h): (f64, f64)) -> Sym {
    let (x, y) = (w / 2.0 - SOFA_ARM, -h / 2.0 + SOFA_ARM);
    Sym::new(
        (w, h),
        vec![
            Shape::Rect(-w / 2.0, -h / 2.0, w / 2.0, h / 2.0),
            Shape::Line(vec![(-x, h / 2.0), (-x, y), (x, y), (x, h / 2.0)]),
        ],
    )
}

/// The corner sofa: an L of the stated depth filling the square, its seat
/// facing the open quadrant. Same two strokes, folded round the corner.
fn corner_sofa((w, h): (f64, f64)) -> Sym {
    const DEPTH: f64 = 900.0;
    let (x0, y0, x1, y1) = (-w / 2.0, -h / 2.0, w / 2.0, h / 2.0);
    let (bx, by) = (x0 + DEPTH, y0 + DEPTH);
    let (ix, iy) = (x0 + SOFA_ARM, y0 + SOFA_ARM);
    Sym::new(
        (w, h),
        vec![
            Shape::Poly(vec![
                (x0, y0),
                (x1, y0),
                (x1, by),
                (bx, by),
                (bx, y1),
                (x0, y1),
            ]),
            Shape::Line(vec![
                (x1 - SOFA_ARM, by),
                (x1 - SOFA_ARM, iy),
                (ix, iy),
                (ix, y1 - SOFA_ARM),
                (bx, y1 - SOFA_ARM),
            ]),
        ],
    )
}

/// A chair, plan-side: a plain square — the seat's own outline is the symbol.
const CHAIR: f64 = 450.0;

/// A dining set: the tabletop with `per` chairs on each long side, seated
/// flush against it, so the pair of rows is exactly what extends the bbox.
fn dining((w, h): (f64, f64), per: usize) -> Sym {
    let mut shapes = vec![Shape::Rect(-w / 2.0, -h / 2.0, w / 2.0, h / 2.0)];
    let step = w / per as f64;
    for i in 0..per {
        let x = -w / 2.0 + step * (i as f64 + 0.5);
        for side in [-1.0, 1.0] {
            shapes.push(box_at(x, side * (h + CHAIR) / 2.0, CHAIR, CHAIR));
        }
    }
    Sym::new((w, h + 2.0 * CHAIR), shapes)
}

/// The round table: the top, and one chair at each quadrant.
fn round_table((w, h): (f64, f64)) -> Sym {
    let mut shapes = vec![Shape::Oval(0.0, 0.0, w / 2.0, h / 2.0)];
    for side in [-1.0, 1.0] {
        shapes.push(box_at(0.0, side * (h + CHAIR) / 2.0, CHAIR, CHAIR));
        shapes.push(box_at(side * (w + CHAIR) / 2.0, 0.0, CHAIR, CHAIR));
    }
    Sym::new((w + 2.0 * CHAIR, h + 2.0 * CHAIR), shapes)
}

/// The bathroom set. Each is the piece's footprint plus the one detail that
/// names it: a tub's basin and drain, a shower's cross and drain, a toilet's
/// tank and bowl, a sink's basin.
fn bath(variant: &str, (w, h): (f64, f64)) -> Sym {
    let (x0, y0, x1, y1) = (-w / 2.0, -h / 2.0, w / 2.0, h / 2.0);
    let body = Shape::Rect(x0, y0, x1, y1);
    let shapes = match variant {
        "shower" => vec![
            body,
            Shape::Line(vec![(x0, y0), (x1, y1)]),
            Shape::Line(vec![(x1, y0), (x0, y1)]),
            Shape::Oval(0.0, 0.0, 70.0, 70.0),
        ],
        // The tank across the back and the bowl filling the rest of the
        // footprint, running **into** the tank — tangent, the pair reads as
        // two pieces that happen to touch.
        "toilet" => {
            const TANK: f64 = 180.0;
            const LAP: f64 = 120.0;
            let rx = (w - TANK + LAP) / 2.0;
            vec![
                Shape::Rect(x0, y0, x0 + TANK, y1),
                Shape::Oval(x1 - rx, 0.0, rx, h / 2.0 - 25.0),
            ]
        }
        "sink" => vec![body, Shape::Oval(0.0, 0.0, w / 2.0 - 80.0, h / 2.0 - 70.0)],
        // The tub: the rim, the rounded basin inside it, and the drain at the
        // tap end — the one detail that says which way it lies.
        _ => {
            const RIM: f64 = 90.0;
            vec![
                body,
                Shape::Round(x0 + RIM, y0 + RIM, x1 - RIM, y1 - RIM, 150.0),
                Shape::Oval(x1 - 280.0, 0.0, 45.0, 45.0),
            ]
        }
    };
    Sym::new((w, h), shapes)
}

/// The kitchen / laundry boxes. The real plans write the appliance's letter
/// **inside** a near-plain box — which is exactly what an `|appliance|`'s smart
/// label does [SPEC 15.11] — so only the hob earns a drawing of its own.
fn appliance(variant: &str, (w, h): (f64, f64)) -> Sym {
    let (x0, y0, x1, y1) = (-w / 2.0, -h / 2.0, w / 2.0, h / 2.0);
    let mut shapes = vec![Shape::Rect(x0, y0, x1, y1)];
    match variant {
        "stove" => {
            let (bx, by, r) = (w / 4.0, h / 4.0, w.min(h) * 0.18);
            for sy in [-1.0, 1.0] {
                for sx in [-1.0, 1.0] {
                    shapes.push(Shape::Oval(sx * bx, sy * by, r, r));
                }
            }
        }
        // The fridge's door, across the front — the one line the charts and
        // both condo plans agree on.
        "fridge" => shapes.push(Shape::Line(vec![(x0, y1 - 120.0), (x1, y1 - 120.0)])),
        // The washer's drum door, a panel inside the box: the label sits in it.
        "washer" => shapes.push(Shape::Rect(x0 + 70.0, y0 + 70.0, x1 - 70.0, y1 - 70.0)),
        // …and the dishwasher stays the plain box its letter names.
        _ => {}
    }
    Sym::new((w, h), shapes)
}

/// A straight flight [SPEC 15.11]: 900 mm wide, `steps` × 250 mm of run. Only
/// the outline is the body — the treads and the up arrow are generated chrome,
/// filled once the flight is sized.
pub(super) fn stairs(steps: f64) -> Sym {
    const WIDTH: f64 = 900.0;
    const GOING: f64 = 250.0;
    let (w, h) = (WIDTH, steps * GOING);
    Sym::new(
        (w, h),
        vec![Shape::Rect(-w / 2.0, -h / 2.0, w / 2.0, h / 2.0)],
    )
}
