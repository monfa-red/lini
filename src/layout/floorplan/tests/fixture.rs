//! The furniture library [SPEC 15.11]: the true-size bodies and their
//! `symbol:` variants, the `width:` / `height:` stretch, the smart label's two
//! seats, a flight's generated chrome, and where a fixture lands when it is
//! placed and turned.

use crate::layout::PlacedNode;
use crate::layout::drawing::testutil::{by_id, text_at};
use crate::resolve::{NodeKind, ResolvedValue};
use crate::testutil::{laid, layout_err};

/// A floorplan drafting in metres at 1 px per millimetre, so a body's box
/// reads its physical size straight off the placed node.
fn plan(body: &str) -> String {
    format!("{{ layout: floorplan; unit: m; density: 1; scale: 1 }}\n{body}")
}

/// The fixture's drawn box, millimetres (the stroke's half is trimmed back off
/// — `stroke-width: 1` inflates every fixture alike).
fn body(n: &PlacedNode) -> (f64, f64) {
    let b = n.bbox.inflate(-n.attrs.half_stroke());
    (
        (b.w() * 1000.0).round() / 1000.0,
        (b.h() * 1000.0).round() / 1000.0,
    )
}

fn path(n: &PlacedNode) -> String {
    match n.attrs.get("path") {
        Some(ResolvedValue::String(d)) => d.clone(),
        other => panic!("no drawn body: {other:?}"),
    }
}

/// The true-size law [SPEC 15.11]: every family's default body is its first
/// table row, in physical millimetres, whatever the scope drafts in.
#[test]
fn each_family_defaults_to_its_first_variant_at_true_size() {
    for (ty, w, h) in [
        ("bed", 1500.0, 2000.0),
        ("sofa", 2200.0, 900.0),
        ("bath", 1700.0, 750.0),
        ("appliance", 600.0, 600.0),
    ] {
        let l = laid(&plan(&format!("|{ty}#f|\n")));
        assert_eq!(body(by_id(&l.nodes, "f")), (w, h), "{ty}");
    }
    // A dining set is sized by its **tabletop**, and its chairs extend the
    // bbox: 1800 × 900 plus a 450 row on each long side.
    let l = laid(&plan("|dining#f|\n"));
    assert_eq!(body(by_id(&l.nodes, "f")), (1800.0, 1800.0));
}

/// `symbol:` picks the variant [SPEC 15.11] — including through the cascade,
/// which is why the body is read at layout and never baked at desugar.
#[test]
fn a_symbol_picks_the_variant_inline_or_from_a_rule() {
    let l = laid(&plan("|bed#f| { symbol: single }\n"));
    assert_eq!(body(by_id(&l.nodes, "f")), (900.0, 2000.0));
    let l = laid(
        "{ layout: floorplan; unit: m; density: 1; scale: 1;\n  |bath| { symbol: toilet } }\n\
         |bath#f|\n",
    );
    assert_eq!(body(by_id(&l.nodes, "f")), (700.0, 400.0));
    // The round table's four quadrant chairs push ⌀1200 out to 2100 square.
    let l = laid(&plan("|dining#f| { symbol: round }\n"));
    assert_eq!(body(by_id(&l.nodes, "f")), (2100.0, 2100.0));
}

/// The mattress sizes [SPEC 15.11]. `queen` is the default a bare `|bed|`
/// draws — the same body, stroke for stroke — and only `single` sleeps alone,
/// so only `single` takes one pillow.
#[test]
fn the_bed_family_is_four_sizes_defaulting_to_queen() {
    for (sym, w, h) in [
        ("queen", 1500.0, 2000.0),
        ("king", 1800.0, 2000.0),
        ("double", 1350.0, 1900.0),
        ("single", 900.0, 2000.0),
    ] {
        let l = laid(&plan(&format!("|bed#f| {{ symbol: {sym} }}\n")));
        assert_eq!(body(by_id(&l.nodes, "f")), (w, h), "{sym}");
    }
    let drawn = |src: &str| path(by_id(&laid(&plan(src)).nodes, "f"));
    assert_eq!(drawn("|bed#f|\n"), drawn("|bed#f| { symbol: queen }\n"));
    assert_eq!(
        drawn("|bed#f|\n").matches('M').count(),
        4,
        "mattress, two pillows, turndown"
    );
    assert_eq!(
        drawn("|bed#f| { symbol: single }\n").matches('M').count(),
        3,
        "one pillow"
    );
}

/// The armchair is the sofa family's one-seater [SPEC 15.11] — the same two
/// strokes at a 900 mm square, never a symbol of its own.
#[test]
fn the_armchair_is_the_sofa_anatomy_at_one_seat() {
    let l = laid(&plan("|sofa#f| { symbol: one }\n"));
    assert_eq!(body(by_id(&l.nodes, "f")), (900.0, 900.0));
    let d = path(by_id(&l.nodes, "f"));
    assert_eq!(d.matches('M').count(), 2, "outline + seat run: {d}");
}

/// One fillet mechanism rounds every corner an upholstered body turns, and it
/// reads the corner it is rounding: the corner sofa's **inside** vertex sweeps
/// the other way, so the L stays soft on both faces instead of bulging.
#[test]
fn a_fillet_turns_with_the_corner_it_rounds() {
    let d = path(by_id(
        &laid(&plan("|sofa#f| { symbol: corner }\n")).nodes,
        "f",
    ));
    let outline = d.split(" M ").next().expect("the outline run");
    let sweeps: Vec<&str> = outline
        .split('A')
        .skip(1)
        .map(|a| a.split_whitespace().nth(4).expect("the sweep flag"))
        .collect();
    assert_eq!(sweeps, ["1", "1", "0", "1", "1", "1"], "{outline}");
}

/// An unknown variant names its family's whole set — the discretes' wording,
/// through the one shared builder.
#[test]
fn an_unknown_symbol_names_the_variants() {
    assert_eq!(
        layout_err(&plan("|sofa#f| { symbol: sectional }\n")),
        "unknown symbol 'sectional' on '|sofa|' — its variants are three, two, one, corner"
    );
    assert_eq!(
        layout_err(&plan("|appliance#f| { symbol: oven }\n")),
        "unknown symbol 'oven' on '|appliance|' — its variants are stove, fridge, washer, dishwasher"
    );
}

/// `width` / `height` are floors [SPEC 5] and the body **stretches** to the
/// box they resolve — one factor per axis, no aspect kept anywhere.
#[test]
fn width_and_height_are_floors_and_stretch_the_body() {
    // An authored value is drawing units — metres here, against a 1500 mm body.
    let l = laid(&plan("|bed#f| { width: 2 }\n"));
    assert_eq!(
        body(by_id(&l.nodes, "f")),
        (2000.0, 2000.0),
        "a floor, not a set"
    );
    let l = laid(&plan("|bed#f| { width: 0.9 }\n"));
    assert_eq!(
        body(by_id(&l.nodes, "f")),
        (1500.0, 2000.0),
        "under the body — inert"
    );
    // The stretch carries the detail with it: at 3 m square the mattress
    // corner sits at ±1500, its pillows scaled to match.
    let l = laid(&plan("|bed#f| { width: 3; height: 3 }\n"));
    let d = path(by_id(&l.nodes, "f"));
    assert!(d.starts_with("M -1500 -1500 L 1500 -1500"), "{d}");
}

/// A fixture's smart label reads **beside** the body, like a discrete's value
/// — except an `|appliance|`'s, which centres in it (the labelled-box
/// convention, [SPEC 15.11]).
#[test]
fn the_smart_label_reads_beside_the_body_and_inside_an_appliance() {
    let l = laid(&plan("|bed#f| \"BED\"\n"));
    let (_, y, _) = text_at(&l.nodes, "BED");
    let f = by_id(&l.nodes, "f");
    assert!(
        y > f.cy + f.bbox.h() / 2.0,
        "the label clears the body: {y} vs {}",
        f.cy + f.bbox.h() / 2.0
    );
    let l = laid(&plan("|appliance#f| \"DW\" { symbol: dishwasher }\n"));
    let (x, y, _) = text_at(&l.nodes, "DW");
    let f = by_id(&l.nodes, "f");
    assert_eq!((x, y), (f.cx, f.cy), "centred in the box it names");
}

/// A fixture datum-places and turns like any drawing geometry [SPEC 15.4] —
/// no exemption of its own; only an opening has one.
#[test]
fn a_fixture_places_and_turns_on_the_datum() {
    let l = laid(&plan(
        "|wall#w| { draw: move(0, 0) right(4):run; thickness: 0.2 }\n\
         |bed#f| { translate: 1.2 1.2; rotate: 90 }\n",
    ));
    let f = by_id(&l.nodes, "f");
    let w = by_id(&l.nodes, "w");
    assert_eq!((f.cx - w.cx, f.cy - w.cy), (1200.0, 1200.0));
    assert_eq!(f.rotation, 90.0);
    // The body is drawn unturned — `rotate:` is the node's, so the bbox is
    // still 1500 × 2000 and the render turns it.
    assert_eq!(body(f), (1500.0, 2000.0));
}

/// A flight [SPEC 15.11]: 900 mm wide × `steps` × 250 mm of run, its risers
/// and up arrow generated chrome filled from the sized body.
#[test]
fn a_flight_sizes_from_its_steps_and_draws_its_chrome() {
    let l = laid(&plan("|stairs#f| { steps: 4 }\n"));
    let f = by_id(&l.nodes, "f");
    assert_eq!(body(f), (900.0, 1000.0));
    let chrome: Vec<(&str, String)> = f
        .children
        .iter()
        .filter_map(|c| {
            let ty = c
                .type_chain
                .iter()
                .find(|t| matches!(t.as_str(), "stair-tread" | "stair-arrow"))?;
            let geo = match (c.attrs.get("path"), c.attrs.get("points")) {
                (Some(ResolvedValue::String(d)), _) => d.clone(),
                (_, Some(ResolvedValue::List(pts))) => pts
                    .iter()
                    .map(|p| match p {
                        ResolvedValue::Tuple(xy) => format!(
                            "{} {}",
                            xy[0].as_number().expect("x"),
                            xy[1].as_number().expect("y")
                        ),
                        _ => String::new(),
                    })
                    .collect::<Vec<_>>()
                    .join(" → "),
                _ => "unfilled".into(),
            };
            Some((ty.as_str(), geo))
        })
        .collect();
    // Three risers divide four treads — the outline draws the two ends — and
    // the arrow climbs the middle from the first tread onto the far edge.
    assert_eq!(
        chrome,
        vec![
            ("stair-tread", "-450 250 → 450 250".to_string()),
            ("stair-tread", "-450 0 → 450 0".to_string()),
            ("stair-tread", "-450 -250 → 450 -250".to_string()),
            (
                "stair-arrow",
                "M 0 375 L 0 -500 M -110 -390 L 0 -500 L 110 -390".to_string()
            ),
        ]
    );
}

/// The body is **one path on the fixture's own node** [SPEC 15.11] — the type
/// rule paints it, so `fill: --bg` masks what it overlaps with no generated
/// child and no class of its own.
#[test]
fn the_body_is_one_path_the_type_rule_paints() {
    let l = laid(&plan("|bath#f| { symbol: shower }\n"));
    let f = by_id(&l.nodes, "f");
    assert_eq!(f.kind, NodeKind::Path);
    assert!(f.children.is_empty());
    assert_eq!(
        path(f),
        "M -450 -450 L 450 -450 L 450 450 L -450 450 Z \
         M -450 -450 L 450 450 M 450 -450 L -450 450 \
         M -70 0 A 70 70 0 1 1 70 0 A 70 70 0 1 1 -70 0 Z"
    );
}

/// Every subpath of a body winds the **same way** [SPEC 15.11]: nonzero fill
/// is what makes furniture mask, and two runs winding against each other would
/// punch a hole exactly where the mask is wanted. The corner sofa is the case
/// — its seat line is authored the way it reads, then wound to match.
#[test]
fn a_bodys_detail_lines_wind_with_its_outline() {
    let d = path(by_id(
        &laid(&plan("|sofa#f| { symbol: corner }\n")).nodes,
        "f",
    ));
    let seat = d.split('M').nth(2).expect("the seat run");
    assert!(
        seat.trim_start().starts_with("-300 1000"),
        "the seat run is reversed to wind with the L: {seat}"
    );
}
