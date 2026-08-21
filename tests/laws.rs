//! The routing CI gate (ROUTING.md, ROUTING-LOG.md stage 6): every sample is
//! re-judged by the independent law checker — semantically, never on SVG
//! snapshots. A snapshot pins one router's coordinates; the validator pins
//! the contract: the four laws hold on everything drawn, every declared edge
//! is drawn or honestly reported, the same input compiles byte-identically,
//! and turning the one routing knob (`clearance`) can shrink the drawable
//! set but never produce illegal geometry.

use lini::Options;
use lini::testing::{
    annotation_text_overlaps, breaches, carried_over_geometry, declared_edges_with, drawn_edges,
    laws, layout_sample, read_sample as read, route_sample_with, routes_str_with, sample_opts,
    samples as sample_paths, strays,
};

/// The clearance sweep: the knob's native span, dense enough to cross every
/// sample's capacity boundaries.
const CLEARANCES: [f64; 7] = [6.0, 8.0, 9.0, 10.0, 12.0, 13.0, 16.0];

/// Laws: on every sample at its native attributes, the independent validator
/// reports nothing above an `Info` crossing and an honest stray.
#[test]
fn every_sample_satisfies_the_laws() {
    for path in sample_paths() {
        let src = read(&path);
        let found = breaches(
            lini::validate_str_with(&src, &sample_opts())
                .unwrap_or_else(|e| panic!("validate {}: {e}", path.display())),
        );
        assert!(
            found.is_empty(),
            "{}: the four laws must hold, got {found:?}",
            path.display()
        );
    }
}

/// Strays are honest, not regressions: at native attributes every sample
/// draws whole — zero impossibles, pinned. (`links_hard` carried four at
/// gap 30; the sample ships wider so the showcase always renders complete —
/// bumped to 32, then to 38 when fan-total side pricing spent the corridor
/// beside the hub that `east -> west` used to squeeze through. Widening `gap`
/// is the contract's own lever for a scene at its capacity edge.)
#[test]
fn impossible_links_are_exactly_the_known_capacity_truths() {
    for path in sample_paths() {
        let src = read(&path);
        let report = lini::validate_str_with(&src, &sample_opts())
            .unwrap_or_else(|e| panic!("validate {}: {e}", path.display()));
        assert_eq!(strays(&report), 0, "{}: stray count moved", path.display());
    }
}

/// Law 4: the same input renders byte-identically, and routes identically.
#[test]
fn every_sample_compiles_and_routes_byte_identically() {
    for path in sample_paths() {
        let src = read(&path);
        let svg = lini::compile_str_with(&src, &sample_opts())
            .unwrap_or_else(|e| panic!("compile {}: {e}", path.display()));
        let routes = routes_str_with(&src, &sample_opts()).expect("routes");
        for _ in 0..2 {
            assert_eq!(
                lini::compile_str_with(&src, &sample_opts()).expect("recompile"),
                svg,
                "{}: compile is not deterministic",
                path.display()
            );
            assert_eq!(
                routes_str_with(&src, &sample_opts()).expect("reroute"),
                routes,
                "{}: routing is not deterministic",
                path.display()
            );
        }
    }
}

/// The clearance sweep: at every knob value the laws hold on everything
/// drawn, and every declared edge is drawn or reported impossible — links
/// never silently vanish, and a tighter diagram may only trade wires for
/// honest strays.
///
/// The admission probe (`src/routing/ortho/admit.rs`) places every route
/// beside the committed chains before it commits, so what the ledger's
/// load counting alone once over-admitted — links_medium @13, pcb @12,
/// links_hard @8, each formerly pinned here as a known limit — now routes
/// lawfully or strays honestly like every other cell.
#[test]
fn every_sample_holds_the_laws_at_every_clearance() {
    for path in sample_paths() {
        let src = read(&path);
        let declared = declared_edges_with(&src, &sample_opts());
        for c in CLEARANCES {
            let laid = route_sample_with(&src, &sample_opts(), c);
            let report = laws(&laid);
            let impossible = strays(&report);
            let found = breaches(report);
            assert!(
                found.is_empty(),
                "{} at clearance {c}: {found:?}",
                path.display()
            );
            assert_eq!(
                drawn_edges(&laid) + impossible,
                declared,
                "{} at clearance {c}: every edge must be drawn or reported",
                path.display()
            );
        }
    }
}

/// A schematic sheet is judged like any other scene [SPEC 16.5]: its wires
/// land on fixed ports (pin stub tips, a label's connection point) and its
/// parts are single obstacles, so the four laws must hold on it — at its own
/// clearance and across the knob, where a tighter sheet may only trade wires
/// for honest strays. The samples sweep above judges the shipped sheets; this
/// one pins the shapes they do not carry — a same-pin fan into a facing part,
/// and a series chain ending in a ground.
#[test]
fn a_schematic_sheet_holds_the_laws_at_every_clearance() {
    const SHEET: &str = "{ layout: schematic }\n\
        |component#u1| \"REG\" [ |pin#vin|; |pin#gnd|; |pin#vout| ]\n\
        |component#u2| \"MCU\" [ |pin#vdd|; |pin#vss|; |pin#io| ]\n\
        |gnd#g1|\n\
        |R#r1|\n\
        |gnd#g2|\n\
        u1.gnd - g1\n\
        u1.vout - u2.vdd\n\
        u1.vout - u2.vss\n\
        u2.io - r1.p1\n\
        r1.p2 - g2\n";
    let opts = Options::default();
    let report = lini::validate_str_with(SHEET, &opts).expect("validate the sheet");
    assert_eq!(
        strays(&report),
        0,
        "a seated sheet routes whole: {report:?}"
    );
    let found = breaches(report);
    assert!(found.is_empty(), "the four laws hold on a sheet: {found:?}");

    let svg = lini::compile_str_with(SHEET, &opts).expect("compile the sheet");
    let routes = routes_str_with(SHEET, &opts).expect("routes");
    for _ in 0..2 {
        assert_eq!(
            lini::compile_str_with(SHEET, &opts).expect("recompile"),
            svg
        );
        assert_eq!(routes_str_with(SHEET, &opts).expect("reroute"), routes);
    }

    let declared = declared_edges_with(SHEET, &opts);
    for c in CLEARANCES {
        let laid = route_sample_with(SHEET, &opts, c);
        let report = laws(&laid);
        let impossible = strays(&report);
        let found = breaches(report);
        assert!(found.is_empty(), "the sheet at clearance {c}: {found:?}");
        assert_eq!(
            drawn_edges(&laid) + impossible,
            declared,
            "the sheet at clearance {c}: every wire drawn or reported"
        );
    }
}

/// The perf tripwire: routing stays a counting problem — one Dijkstra per
/// bundle over tens of cells, one placement sweep per channel. Ten debug
/// compiles of the busiest sample run a few seconds on a dev laptop and
/// noticeably slower on a shared CI runner; the budget is deliberately loose —
/// it only has to catch an audit-style blowup, never machine variance.
#[test]
fn routing_pcb_ten_times_stays_fast() {
    let src = read(std::path::Path::new("samples/pcb.lini"));
    let start = std::time::Instant::now();
    for _ in 0..10 {
        lini::compile_str_with(&src, &sample_opts()).expect("compile pcb");
    }
    let took = start.elapsed();
    assert!(
        took.as_secs_f64() < 30.0,
        "10 debug compiles took {took:?}, budget 30 s"
    );
}

/// Natural's own tripwire: no channels, no search, no ledger — a mindmap is
/// spline fits, so ten debug compiles must stay well under the corridor
/// budget (the corridor-first build spent ~3 s per compile here).
#[test]
fn routing_mindmap_ten_times_stays_fast() {
    let src = read(std::path::Path::new("samples/mindmap.lini"));
    let start = std::time::Instant::now();
    for _ in 0..10 {
        lini::compile_str_with(&src, &sample_opts()).expect("compile mindmap");
    }
    let took = start.elapsed();
    assert!(
        took.as_secs_f64() < 10.0,
        "10 debug compiles took {took:?}, budget 10 s"
    );
}

// ── The drawing sheets' annotation oracles [SPEC 15.6/15.9] ──

/// The packing oracle: a dimension row stands `clearance` off everything
/// painted, so on every drawing sample no dim value may land on another
/// annotation's text — another row's, a callout's, an angle's.
#[test]
fn no_annotation_text_lands_on_another_across_the_drawing_samples() {
    let mut seen = 0;
    for path in sample_paths() {
        let src = read(&path);
        if !src.contains("drawing") {
            continue;
        }
        seen += 1;
        let found = annotation_text_overlaps(&layout_sample(&src, &sample_opts()));
        assert!(
            found.is_empty(),
            "{}: {}",
            path.display(),
            found.join("\n  ")
        );
    }
    assert!(seen >= 6, "the drawing samples compiled: {seen}");
}

/// The carrying statement's own clearing: what a statement paints below its
/// text — its carried stack — is part of its own painted band / leader block,
/// so no carried box may cross the drawn geometry in any drawing sample.
#[test]
fn no_carried_annotation_lands_on_the_drawn_geometry_across_the_samples() {
    let mut judged = 0;
    for path in sample_paths() {
        let src = read(&path);
        if !src.contains("drawing") {
            continue;
        }
        let (found, seen) = carried_over_geometry(&layout_sample(&src, &sample_opts()));
        judged += seen;
        assert!(
            found.is_empty(),
            "{}: {}",
            path.display(),
            found.join("\n  ")
        );
    }
    assert!(judged >= 2, "the carried statements compiled: {judged}");
}
