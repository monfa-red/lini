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
//!
//! Where a piece is upholstered or a seat, its corners take a small fillet —
//! finish, not detail: enough that the furniture reads soft against the square
//! casework and the poché around it, never enough to draw the eye. Masonry,
//! appliances and the sanitaryware keep the hard edges every plan draws them
//! with.

use super::shape::{Shape, Sym, box_at, rect};

/// A rectangle drawn with the corners it was authored with.
const SHARP: f64 = 0.0;

/// The upholstery fillet — a sofa's or an armchair's outline and the seat run
/// inside it.
const SEAT_R: f64 = 80.0;

/// A tabletop's corner, and a chair's.
const TOP_R: f64 = 60.0;
const CHAIR_R: f64 = 50.0;

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
/// across it — the read every plan uses, in three strokes. Every size but
/// `single` sleeps two, so it splits its pillows about the centre.
fn bed((w, h): (f64, f64), single: bool) -> Sym {
    const MARGIN: f64 = 80.0;
    const PILLOW: f64 = 350.0;
    const TURNDOWN: f64 = 400.0;
    let (top, cy) = (-h / 2.0 + MARGIN, -h / 2.0 + MARGIN + PILLOW / 2.0);
    let mut shapes = vec![rect(-w / 2.0, -h / 2.0, w / 2.0, h / 2.0, SHARP)];
    if single {
        shapes.push(box_at(0.0, cy, w - 2.0 * MARGIN, PILLOW, SHARP));
    } else {
        let pw = (w - 3.0 * MARGIN) / 2.0;
        let off = (pw + MARGIN) / 2.0;
        shapes.push(box_at(-off, cy, pw, PILLOW, SHARP));
        shapes.push(box_at(off, cy, pw, PILLOW, SHARP));
    }
    let fold = top + PILLOW + TURNDOWN;
    shapes.push(Shape::Line(vec![(-w / 2.0, fold), (w / 2.0, fold)], SHARP));
    Sym::new((w, h), shapes)
}

/// A straight sofa — three-seat, two-seat, or the armchair, one anatomy at
/// three widths: the outline, and one inner run tracing arm → back → arm. Two
/// strokes; the cushion divisions the charts draw read as clutter at 1:50.
const SOFA_ARM: f64 = 200.0;

fn sofa((w, h): (f64, f64)) -> Sym {
    let (x, y) = (w / 2.0 - SOFA_ARM, -h / 2.0 + SOFA_ARM);
    Sym::new(
        (w, h),
        vec![
            rect(-w / 2.0, -h / 2.0, w / 2.0, h / 2.0, SEAT_R),
            Shape::Line(vec![(-x, h / 2.0), (-x, y), (x, y), (x, h / 2.0)], SEAT_R),
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
            Shape::Poly(
                vec![(x0, y0), (x1, y0), (x1, by), (bx, by), (bx, y1), (x0, y1)],
                SEAT_R,
            ),
            Shape::Line(
                vec![
                    (x1 - SOFA_ARM, by),
                    (x1 - SOFA_ARM, iy),
                    (ix, iy),
                    (ix, y1 - SOFA_ARM),
                    (bx, y1 - SOFA_ARM),
                ],
                SEAT_R,
            ),
        ],
    )
}

/// A chair, plan-side: a plain square — the seat's own outline is the symbol.
const CHAIR: f64 = 450.0;

/// A dining set: the tabletop with `per` chairs on each long side, seated
/// flush against it, so the pair of rows is exactly what extends the bbox.
fn dining((w, h): (f64, f64), per: usize) -> Sym {
    let mut shapes = vec![rect(-w / 2.0, -h / 2.0, w / 2.0, h / 2.0, TOP_R)];
    let step = w / per as f64;
    for i in 0..per {
        let x = -w / 2.0 + step * (i as f64 + 0.5);
        for side in [-1.0, 1.0] {
            shapes.push(box_at(x, side * (h + CHAIR) / 2.0, CHAIR, CHAIR, CHAIR_R));
        }
    }
    Sym::new((w, h + 2.0 * CHAIR), shapes)
}

/// The round table: the top, and one chair at each quadrant.
fn round_table((w, h): (f64, f64)) -> Sym {
    let mut shapes = vec![Shape::Oval(0.0, 0.0, w / 2.0, h / 2.0)];
    for side in [-1.0, 1.0] {
        shapes.push(box_at(0.0, side * (h + CHAIR) / 2.0, CHAIR, CHAIR, CHAIR_R));
        shapes.push(box_at(side * (w + CHAIR) / 2.0, 0.0, CHAIR, CHAIR, CHAIR_R));
    }
    Sym::new((w + 2.0 * CHAIR, h + 2.0 * CHAIR), shapes)
}

/// The bathroom set. Each is the piece's footprint plus the one detail that
/// names it: a tub's basin and drain, a shower's cross and drain, a toilet's
/// tank and bowl, a sink's basin.
fn bath(variant: &str, (w, h): (f64, f64)) -> Sym {
    let (x0, y0, x1, y1) = (-w / 2.0, -h / 2.0, w / 2.0, h / 2.0);
    let body = rect(x0, y0, x1, y1, SHARP);
    let shapes = match variant {
        "shower" => vec![
            body,
            Shape::Line(vec![(x0, y0), (x1, y1)], SHARP),
            Shape::Line(vec![(x1, y0), (x0, y1)], SHARP),
            Shape::Oval(0.0, 0.0, 70.0, 70.0),
        ],
        // The cistern across the back and the bowl filling the rest of the
        // footprint, running **into** it. The bowl is narrower than the pan is
        // wide, so the tank keeps a shoulder either side — what makes the pair
        // read as a toilet at 1:50 rather than an oval on a sliver.
        "toilet" => {
            const TANK: f64 = 220.0;
            const LAP: f64 = 80.0;
            const SHOULDER: f64 = 55.0;
            let (back, wide) = (x0 + TANK - LAP, h / 2.0 - SHOULDER);
            vec![
                rect(x0, y0, x0 + TANK, y1, SHARP),
                rect(back, -wide, x1, wide, wide * 0.8),
            ]
        }
        // The basin **is** the sink: the counter or vanity it drops into is the
        // author's own `|rect|` [SPEC 15.11], so drawing a second rim here
        // would nest three outlines where every real plan draws two.
        "sink" => vec![
            rect(x0, y0, x1, y1, 60.0),
            Shape::Oval(0.0, 0.0, 45.0, 45.0),
        ],
        // The tub: the rim, the rounded basin inside it, and the drain at the
        // tap end — the one detail that says which way it lies.
        _ => {
            const RIM: f64 = 90.0;
            vec![
                body,
                rect(x0 + RIM, y0 + RIM, x1 - RIM, y1 - RIM, 150.0),
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
    let mut shapes = vec![rect(x0, y0, x1, y1, SHARP)];
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
        "fridge" => shapes.push(Shape::Line(vec![(x0, y1 - 120.0), (x1, y1 - 120.0)], SHARP)),
        // The washer's drum door, a panel inside the box: the label sits in it.
        "washer" => shapes.push(rect(x0 + 70.0, y0 + 70.0, x1 - 70.0, y1 - 70.0, SHARP)),
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
        vec![rect(-w / 2.0, -h / 2.0, w / 2.0, h / 2.0, SHARP)],
    )
}
