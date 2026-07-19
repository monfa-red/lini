//! The routing CI gate (ROUTING.md, ROUTING-LOG.md stage 6): every sample is
//! re-judged by the independent law checker — semantically, never on SVG
//! snapshots. A snapshot pins one router's coordinates; the validator pins
//! the contract: the four laws hold on everything drawn, every declared edge
//! is drawn or honestly reported, the same input compiles byte-identically,
//! and turning the one routing knob (`clearance`) can shrink the drawable
//! set but never produce illegal geometry.

use lini::testing::{declared_edges_with, drawn_edges, laws, route_sample_with, routes_str_with};
use lini::{Options, Rule, Severity};
use std::path::PathBuf;

/// Samples resolve their image assets against their own directory [SPEC 7].
fn sample_opts() -> Options {
    Options {
        base_dir: Some(PathBuf::from("samples")),
        ..Default::default()
    }
}

/// The clearance sweep: the knob's native span, dense enough to cross every
/// sample's capacity boundaries.
const CLEARANCES: [f64; 7] = [6.0, 8.0, 9.0, 10.0, 12.0, 13.0, 16.0];

fn sample_paths() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir("samples")
        .expect("read samples/")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "lini"))
        // Icons need the `icons` feature; drop icon-using samples when it's off.
        .filter(|p| {
            cfg!(feature = "icons")
                || !std::fs::read_to_string(p)
                    .unwrap_or_default()
                    .contains("|icon|")
        })
        .collect();
    paths.sort();
    paths
}

fn read(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Law breaches: everything the checker or engine flags above counted output
/// (`Info` crossings) and honest strays (`Impossible` — pinned separately).
fn breaches(report: Vec<lini::Violation>) -> Vec<lini::Violation> {
    report
        .into_iter()
        .filter(|v| v.severity != Severity::Info && v.rule != Rule::Impossible)
        .collect()
}

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
/// gap 30; the sample now ships at gap 32 so the showcase always renders
/// complete.)
#[test]
fn impossible_links_are_exactly_the_known_capacity_truths() {
    for path in sample_paths() {
        let src = read(&path);
        let impossible = lini::validate_str_with(&src, &sample_opts())
            .unwrap_or_else(|e| panic!("validate {}: {e}", path.display()))
            .into_iter()
            .filter(|v| v.rule == Rule::Impossible)
            .count();
        assert_eq!(impossible, 0, "{}: stray count moved", path.display());
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
            let impossible = report.iter().filter(|v| v.rule == Rule::Impossible).count();
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
