//! The clearance sweep (PLAN §Test strategy): `clearance` is the one routing
//! knob, and turning it must never break a law or lose a link silently — at
//! every swept value the laws hold on everything drawn and every declared
//! edge is drawn or reported impossible. A denser clearance may legitimately
//! shrink the drawable set; it may never produce illegal geometry. The sweep
//! runs **growth-off** (`route_sample_raw`) — it measures the router, not
//! the escape hatch; gap growth has its own gate below.

use lini::{Rule, Severity};
use std::path::PathBuf;

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

/// Completeness across the sweep (Phase 6): turning the clearance knob must
/// never starve a link while legal geometry remains. The raw router (no gap
/// growth) owns all of `links_simple` and `links_medium` through clearance
/// 10; past that, medium's corridors genuinely lack lanes — completing the
/// rest of the sweep is gap growth's job and gated by Phase 8, not here.
#[test]
fn completeness_holds_where_the_raw_router_owns_it() {
    let complete = |name: &str, c: f64| {
        let src = std::fs::read_to_string(format!("samples/{name}.lini")).expect("read sample");
        let laid = lini::testing::route_sample_raw(&src, c);
        assert_eq!(
            laid.links.len(),
            lini::testing::declared_edges(&src),
            "{name} at clearance {c}: every declared edge must draw"
        );
    };
    for c in CLEARANCES {
        complete("links_simple", c);
    }
    for c in [6.0, 8.0, 10.0] {
        complete("links_medium", c);
    }
}

/// Gap growth's gate (Phase 8, re-derived for Phase 8½): where the raw
/// router honestly runs out of corridor lanes, growth makes room — every
/// routing scene draws every declared edge at every sweep clearance.
/// (`links_medium` at clearance 16 was pinned at 13/14 + one stray until
/// Phase 9: the separation audit's best-of-round repair selection found the
/// lawful mesh the first-accept greedy walked past.)
#[test]
fn growth_completes_the_scenes_across_the_sweep() {
    for name in ["links_simple", "links_medium", "links_hard"] {
        let src = std::fs::read_to_string(format!("samples/{name}.lini")).expect("read sample");
        let declared = lini::testing::declared_edges(&src);
        for c in CLEARANCES {
            let laid = lini::testing::route_sample(&src, c);
            let breaches: Vec<_> = lini::testing::laws(&laid)
                .into_iter()
                .filter(|v| v.severity != Severity::Info && v.rule != Rule::Impossible)
                .collect();
            assert!(breaches.is_empty(), "{name} at clearance {c}: {breaches:?}");
            assert_eq!(
                laid.links.len(),
                declared,
                "{name} at clearance {c}: growth must complete the scene"
            );
            assert!(laid.strays.is_empty(), "{name} at clearance {c}");
        }
    }
}

#[test]
fn every_sample_holds_the_laws_at_every_clearance() {
    for path in sample_paths() {
        let src = std::fs::read_to_string(&path).expect("read sample");
        let declared = lini::testing::declared_edges(&src);
        for c in CLEARANCES {
            let laid = lini::testing::route_sample_raw(&src, c);
            let report = lini::testing::laws(&laid);
            let breaches: Vec<_> = report
                .iter()
                .filter(|v| v.severity != Severity::Info && v.rule != Rule::Impossible)
                .collect();
            assert!(
                breaches.is_empty(),
                "{} at clearance {c}: {breaches:?}",
                path.display()
            );
            let impossible = report.iter().filter(|v| v.rule == Rule::Impossible).count();
            assert_eq!(
                laid.links.len() + impossible,
                declared,
                "{} at clearance {c}: every edge must be drawn or reported",
                path.display()
            );
        }
    }
}
