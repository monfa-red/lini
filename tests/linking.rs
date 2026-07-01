//! The ROUTING CI gate (see `ROUTING.md`, `PLAN.md`).
//!
//! ROUTING is gated **semantically** — the four laws, the crossing report, edge
//! completeness, determinism — never on SVG snapshots: a snapshot pins one
//! router's coordinates, the validator pins the contract. Tests marked
//! `#[ignore]` are phase gates; each phase un-ignores the ones it makes true.

use lini::{Rule, Severity};
use std::path::PathBuf;

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

/// Laws: on every sample, the independent validator reports nothing above an
/// `Info` crossing.
#[test]
fn every_sample_satisfies_the_laws() {
    for path in sample_paths() {
        let src = read(&path);
        lini::compile_str(&src).unwrap_or_else(|e| panic!("compile {}: {e}", path.display()));
        let breaches: Vec<_> = lini::validate_str(&src)
            .unwrap_or_else(|e| panic!("validate {}: {e}", path.display()))
            .into_iter()
            .filter(|v| v.severity != Severity::Info)
            .collect();
        assert!(
            breaches.is_empty(),
            "{}: the four laws must hold, got {breaches:?}",
            path.display()
        );
    }
}

/// Law 4: the same input renders byte-identically.
#[test]
fn compile_is_byte_identical_across_runs() {
    for path in sample_paths() {
        let src = read(&path);
        let a = lini::compile_str(&src).expect("compile a");
        let b = lini::compile_str(&src).expect("compile b");
        assert_eq!(a, b, "{}: compile is not deterministic", path.display());
    }
}

/// Completeness: every declared edge is drawn, or reported impossible — links
/// never silently vanish.
#[test]
fn every_declared_edge_is_drawn_or_reported() {
    for path in sample_paths() {
        let src = read(&path);
        let laid = lini::testing::route_sample(&src, 8.0);
        let drawn = laid.links.len();
        let impossible = lini::testing::laws(&laid)
            .iter()
            .filter(|v| v.rule == Rule::Impossible)
            .count();
        assert_eq!(
            drawn + impossible,
            lini::testing::declared_edges(&src),
            "{}: every edge must be drawn or reported",
            path.display()
        );
    }
}

/// Completeness under pressure (Phase 6): at the native clearance the three
/// routing scenes draw every declared edge — nothing starved by first-come
/// routing, nothing undrawn by a repair.
#[test]
fn the_scenes_draw_every_edge_at_native_clearance() {
    for name in ["links_simple", "links_medium", "links_hard"] {
        let path = format!("samples/{name}.lini");
        let src = read(std::path::Path::new(&path));
        let laid = lini::testing::route_sample(&src, 8.0);
        assert_eq!(
            laid.links.len(),
            lini::testing::declared_edges(&src),
            "{name}: every declared edge must draw at the native clearance"
        );
    }
}

/// Law-shaped Phase-2 properties, measured directly on the routed polylines.
/// Vacuous until links draw; the completeness gate above keeps them honest.
#[test]
fn links_are_orthogonal_polylines() {
    for path in sample_paths() {
        let laid = lini::testing::route_sample(&read(&path), 8.0);
        for w in &laid.links {
            assert!(w.path.len() >= 2, "{}: degenerate link", path.display());
            for s in w.path.windows(2) {
                let ((x0, y0), (x1, y1)) = (s[0], s[1]);
                assert!(
                    x0 == x1 || y0 == y1,
                    "{}: diagonal segment {s:?} on {}→{}",
                    path.display(),
                    w.seg_from,
                    w.seg_to
                );
            }
        }
    }
}

/// Law 2: each end sits on a side of its endpoint's rect (corners excluded)
/// and the adjoining segment leaves perpendicular to that side.
#[test]
fn link_ends_land_perpendicular_on_their_sides() {
    for path in sample_paths() {
        let laid = lini::testing::route_sample(&read(&path), 8.0);
        for w in &laid.links {
            let n = w.path.len();
            check_end(&laid, w.path[0], w.path[1], &w.seg_from, &path);
            check_end(&laid, w.path[n - 1], w.path[n - 2], &w.seg_to, &path);
        }
    }
}

fn check_end(
    laid: &lini::testing::LaidOut,
    port: (f64, f64),
    inward: (f64, f64),
    node: &str,
    sample: &std::path::Path,
) {
    let (x0, y0, x1, y1) = lini::testing::node_rect(laid, node)
        .unwrap_or_else(|| panic!("{}: no rect for '{node}'", sample.display()));
    let eps = 1e-6;
    let (px, py) = port;
    let on_vertical =
        ((px - x0).abs() < eps || (px - x1).abs() < eps) && py > y0 + eps && py < y1 - eps;
    let on_horizontal =
        ((py - y0).abs() < eps || (py - y1).abs() < eps) && px > x0 + eps && px < x1 - eps;
    assert!(
        on_vertical || on_horizontal,
        "{}: port {port:?} not on a side of '{node}' {:?}",
        sample.display(),
        (x0, y0, x1, y1)
    );
    let perpendicular = if on_vertical {
        (inward.1 - py).abs() < eps
    } else {
        (inward.0 - px).abs() < eps
    };
    assert!(
        perpendicular,
        "{}: oblique attachment at {port:?} on '{node}'",
        sample.display()
    );
}

/// Law 1 on the one scripted detour: `ant -> bee` must clear Wall's body by
/// ≥ clearance along its whole length.
#[test]
fn detour_keeps_clearance_from_the_blocking_node() {
    let src = read(std::path::Path::new("samples/links_simple.lini"));
    let laid = lini::testing::route_sample(&src, 10.0);
    let wall = lini::testing::node_rect(&laid, "wall").expect("wall rect");
    let link = laid
        .links
        .iter()
        .find(|w| w.seg_from == "ant" && w.seg_to == "bee")
        .expect("ant→bee drawn");
    for s in link.path.windows(2) {
        let d = seg_rect_distance(s[0], s[1], wall);
        assert!(
            d >= 10.0 - 1e-6,
            "segment {s:?} is {d} from Wall, needs ≥ 10"
        );
    }
}

/// Distance between an axis-aligned segment and a rect (both treated as boxes).
fn seg_rect_distance(a: (f64, f64), b: (f64, f64), r: (f64, f64, f64, f64)) -> f64 {
    let (sx0, sx1) = (a.0.min(b.0), a.0.max(b.0));
    let (sy0, sy1) = (a.1.min(b.1), a.1.max(b.1));
    let (x0, y0, x1, y1) = r;
    let dx = (x0 - sx1).max(sx0 - x1).max(0.0);
    let dy = (y0 - sy1).max(sy0 - y1).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

/// Law 3 pins: the three routing scenes carry a known number of forced
/// crossings — simple stays clean, the others force a handful. The compass
/// groups carry captions and stacked units, so the dense gap leaves little
/// corridor room and the scene is heavily contended. Behaviour pin, not a
/// coordinate pin — if the audit ever finds fewer, lower the pin, never the
/// engine. (The scene's gap is the density dial, but the count is
/// non-monotonic in it — these are tuned to their current geometry.)
#[test]
fn crossing_counts_are_pinned() {
    let crossings = |path: &str| {
        let src = read(std::path::Path::new(path));
        lini::validate_str(&src)
            .expect("validate")
            .into_iter()
            .filter(|v| v.rule == Rule::Crossing)
            .count()
    };
    // Behaviour pin, not a coordinate pin — re-pinned to the current geometry
    // (monospace, node stroke-width 1.6, pinned captions). The node stroke bump
    // (1.5 → 1.6) shifted links_hard's forced crossings 5 → 7; the laws still hold
    // (see `every_sample_satisfies_the_laws`), so the pin follows the audit.
    assert_eq!(crossings("samples/links_simple.lini"), 0);
    assert_eq!(crossings("samples/links_medium.lini"), 6);
    assert_eq!(crossings("samples/links_hard.lini"), 7);
}

/// Law 3 (Economy), audit accept: a crossing a longer route can remove is
/// removed. The plus layout's shortest second route would cross the first;
/// the engine must detour it instead and draw both.
#[test]
fn audit_removes_a_removable_crossing() {
    let src = "{ layout: grid; columns: repeat(3); gap: 40;\n\
               clearance: 8;\n\
               }\n\
               |box#north| { cell: 2 1; }\n\
               |box#south| { cell: 2 3; }\n\
               |box#west| { cell: 1 2; }\n\
               |box#east| { cell: 3 2; }\n\
               north -> south\n\
               west -> east\n";
    let crossings = lini::validate_str(src)
        .expect("validate")
        .into_iter()
        .filter(|v| v.rule == Rule::Crossing)
        .count();
    assert_eq!(crossings, 0, "the detour must remove the crossing");
    let laid = lini::testing::route_sample(src, 8.0);
    assert_eq!(laid.links.len(), 2, "both links must still be drawn");
}

/// Impossible layouts: a node walled in on every side (its neighbours'
/// keep-outs seal every face) is reported with its link, never drawn dirty —
/// and the report reaches the CLI's strict gate as a diagnostic.
// TODO(routing wiring): at node stroke-width 1.6 this fixture no longer walls
// `core` in — the gap-growth lever widens the grid (10 → 32) and routes
// `core -> n2` directly instead of reporting it impossible. The stray-draw /
// strict-diagnostic path it covers needs a fixture that gap-growth can't rescue
// (separate bodies, growth-proof). Deferred per owner; re-enable once rewired.
#[ignore = "stroke 1.6 geometry: gap-growth routes core->n2; fixture needs rewiring"]
#[test]
fn a_walled_in_link_is_reported_impossible() {
    let src = "{ layout: grid; columns: repeat(3); gap: 10;\n\
               clearance: 16;\n\
               }\n\
               |box#n1| { width: 40; height: 40; cell: 1 1; }\n\
               |box#n2| { width: 40; height: 40; cell: 2 1; }\n\
               |box#n3| { width: 40; height: 40; cell: 3 1; }\n\
               |box#n4| { width: 40; height: 40; cell: 1 2; }\n\
               |box#core| { width: 40; height: 40; cell: 2 2; }\n\
               |box#n5| { width: 40; height: 40; cell: 3 2; }\n\
               |box#n6| { width: 40; height: 40; cell: 1 3; }\n\
               |box#n7| { width: 40; height: 40; cell: 2 3; }\n\
               |box#n8| { width: 40; height: 40; cell: 3 3; }\n\
               core -> n2\n";
    let impossible: Vec<_> = lini::validate_str(src)
        .expect("validate")
        .into_iter()
        .filter(|v| v.rule == Rule::Impossible)
        .collect();
    assert_eq!(impossible.len(), 1, "{impossible:?}");
    assert_eq!(impossible[0].links, vec!["core -> n2".to_owned()]);

    let laid = lini::testing::route_sample(src, 16.0);
    assert!(
        laid.links.is_empty(),
        "the impossible link must not be drawn"
    );

    // The report made visible (ROUTING §Impossible layouts): the impossible
    // link renders as an stray — beside the links, never as one, so the
    // validator (which already ran clean above) never sees it.
    assert_eq!(laid.strays.len(), 1, "the report must be drawn");
    let aw = &laid.strays[0];
    assert_eq!((aw.data_from.as_str(), aw.data_to.as_str()), ("core", "n2"));
    let svg = lini::compile_str(src).expect("compile");
    assert!(svg.contains("lini-stray"), "the stray must reach the SVG");

    let (_, diags) = lini::compile_str_checked(src, &lini::Options::default()).expect("compile");
    assert!(
        !diags.is_empty(),
        "--strict must have a diagnostic to fail on"
    );
}

/// Gap growth (Phase 8): when links are impossible for lack of corridor
/// lanes, the named containers' gaps grow by exactly the deficit and the
/// scene reroutes — at most two rounds, deterministically. `links_medium`
/// at clearance 12 is the canonical starved scene: the raw router loses two
/// links to corridor deficits, one growth round completes it.
#[test]
fn gap_growth_completes_a_starved_scene() {
    let src = read(std::path::Path::new("samples/links_medium.lini"))
        .replace("clearance: 8", "clearance: 12");
    let declared = lini::testing::declared_edges(&src);

    let raw = lini::testing::route_sample_raw(&src, 12.0);
    assert!(
        raw.links.len() < declared,
        "the scene must starve the raw router for this gate to mean anything"
    );

    let grown = lini::testing::route_sample(&src, 12.0);
    assert_eq!(grown.links.len(), declared, "growth must complete it");
    assert!(grown.strays.is_empty());
    let breaches: Vec<_> = lini::testing::laws(&grown)
        .into_iter()
        .filter(|v| v.severity != Severity::Info)
        .collect();
    assert!(breaches.is_empty(), "{breaches:?}");

    // Law 4 through the growth loop: deficit classification, the growth map,
    // and the re-layout are all deterministic.
    let a = lini::compile_str(&src).expect("compile a");
    let b = lini::compile_str(&src).expect("compile b");
    assert_eq!(a, b, "growth must stay deterministic");
}

/// Gap growth is bounded and honest: a deficit no gap controls — links
/// forced into a padding-bounded corridor of a container whose gap the
/// deficit names anyway — grows nothing useful, stops after its two rounds,
/// and keeps the original layout with the starved links reported. (The
/// pair is containment, so the report stays textual — strays draw only
/// for separate bodies; the walled-in fixture above covers that path.)
#[test]
fn gap_growth_is_bounded_where_no_gap_can_help() {
    let src = "{ direction: row; gap: 40;\n\
                 clearance: 16;\n\
               }\n\
               |group#grp| {\n\
                 direction: row; gap: 24; padding: 24;\n\
               } [\n\
                 |box#aa| { width: 40; height: 40; }\n\
                 |box#bb| { width: 40; height: 40; }\n\
               ]\n\
               grp:left -> grp.aa:left\ngrp:left -> grp.aa:left\ngrp:left -> grp.aa:left\n";
    let raw = lini::testing::route_sample_raw(src, 16.0);
    let grown = lini::testing::route_sample(src, 16.0);
    assert_eq!(raw.links.len(), 1, "the ring corridor holds one lane");
    assert_eq!(
        grown.links.len(),
        raw.links.len(),
        "growth must not pretend to help"
    );
    assert_eq!(
        lini::testing::node_rect(&grown, "grp"),
        lini::testing::node_rect(&raw, "grp"),
        "a fruitless growth round must not survive into the kept layout"
    );
    let impossible = lini::testing::laws(&grown)
        .into_iter()
        .filter(|v| v.rule == Rule::Impossible)
        .count();
    assert_eq!(impossible, 2, "the starved links stay reported");
}

/// Law 2's compaction clause (Phase 8½): a small shape never turns links
/// away. Twenty links converge on a hub whose four sides hold 16 ports at
/// clearance pitch; the overflowed sides re-pitch all their ports evenly
/// below clearance — like the pins of an IC — and every law still holds on
/// everything drawn.
#[test]
fn a_full_node_compacts_port_rows_rather_than_turning_links_away() {
    // Empty labels (`""`): these are routing nodes, sized by width/height — an
    // id-as-label would float them larger via the content floor (SPEC §6).
    let mut src = String::from(
        "{ layout: grid; columns: repeat(5); gap: 60;\n\
         clearance: 8;\n\
         }\n\
         |box#hub| \"\" { width: 40; height: 40; cell: 3 3; }\n",
    );
    let cells: Vec<(usize, usize)> = (1..=5)
        .flat_map(|r| (1..=5).map(move |c| (c, r)))
        .filter(|&cell| cell != (3, 3))
        .take(20)
        .collect();
    for (i, (c, r)) in cells.iter().enumerate() {
        src.push_str(&format!(
            "|box#s{i:02}| \"\" {{ width: 30; height: 30; cell: {c} {r}; }}\n"
        ));
    }
    for i in 0..cells.len() {
        src.push_str(&format!("s{i:02} -> hub\n"));
    }

    let laid = lini::testing::route_sample(&src, 8.0);
    assert_eq!(laid.links.len(), 20, "every declared edge must draw");
    assert!(laid.strays.is_empty(), "nothing may fall back to an stray");
    let breaches: Vec<_> = lini::testing::laws(&laid)
        .into_iter()
        .filter(|v| v.severity != Severity::Info)
        .collect();
    assert!(breaches.is_empty(), "{breaches:?}");

    // Compaction is even, never a weld: every link keeps its own port, and
    // each side's pitch is uniform — at clearance while the side has slots,
    // at the side's widest sub-clearance pitch once it overflows.
    let mut ports: Vec<(u64, u64)> = laid
        .links
        .iter()
        .map(|w| {
            let p = *w.path.last().expect("hub end");
            (p.0.to_bits(), p.1.to_bits())
        })
        .collect();
    ports.sort_unstable();
    ports.dedup();
    assert_eq!(ports.len(), 20, "every link keeps a distinct port");

    let (x0, y0, x1, _y1) = lini::testing::node_rect(&laid, "hub").expect("hub rect");
    let mut rows: std::collections::BTreeMap<u8, Vec<f64>> = Default::default();
    for w in &laid.links {
        let (px, py) = *w.path.last().unwrap();
        let (side, ord) = if (px - x0).abs() < 1e-6 {
            (3, py)
        } else if (px - x1).abs() < 1e-6 {
            (1, py)
        } else if (py - y0).abs() < 1e-6 {
            (0, px)
        } else {
            (2, px)
        };
        rows.entry(side).or_default().push(ord);
    }
    let usable = (x1 - x0) - 2.0 * 8.0;
    for (side, mut ords) in rows {
        ords.sort_by(f64::total_cmp);
        let k = ords.len();
        let expected = if k > 4 {
            usable / (k as f64 - 1.0)
        } else {
            8.0
        };
        for g in ords.windows(2) {
            assert!(
                (g[1] - g[0] - expected).abs() < 1e-6,
                "side {side}: pitch {} must be the even {expected}",
                g[1] - g[0]
            );
        }
    }

    // Law 4 holds through the compaction lever.
    let a = lini::compile_str(&src).expect("compile a");
    let b = lini::compile_str(&src).expect("compile b");
    assert_eq!(a, b, "compaction must stay deterministic");
}

#[test]
fn a_spanning_chip_side_keeps_accepting_clear_ports() {
    let src = read(std::path::Path::new("samples/pcb.lini")).replacen(
        "pwr.right -> mcu\npwr.right -> mcu.left",
        "pwr.right -> mcu\npwr.right -> mcu\npwr.right -> mcu.left",
        1,
    );
    let laid = lini::testing::route_sample(&src, 10.0);
    assert_eq!(
        laid.links.len(),
        lini::testing::declared_edges(&src),
        "the large MCU side has enough clear ports and corridor lanes"
    );
}

/// Law 3, audit reject: the forced interleave's crossing is kept and named —
/// the `.hot` pair rides the report alongside the other forced crossings.
#[test]
fn the_kept_crossing_names_its_link_pair() {
    let src = read(std::path::Path::new("samples/links_hard.lini"));
    let kept: Vec<_> = lini::validate_str(&src)
        .expect("validate")
        .into_iter()
        .filter(|v| v.rule == Rule::Crossing)
        .collect();
    assert_eq!(kept.len(), 7);
    assert!(
        kept.iter().any(|v| v.links
            == vec![
                "alpha -> west.ww2".to_owned(),
                "west.ww1 -> gamma".to_owned()
            ]),
        "the .hot interleave must be named: {kept:?}"
    );
}

/// Phase 9: link labels ride their link (ROUTING §Model step 7). Every
/// declared label is placed once on its statement's drawn route — a chain's
/// label on exactly one of its segments — and a label's box overlaps no
/// leaf node body (it may slide along the link, never off it).
#[test]
fn link_labels_ride_their_links_and_dodge_nodes() {
    let leaf_clear = |laid: &lini::testing::LaidOut, leaves: &[&str], expect: usize| {
        let texts: Vec<_> = laid.links.iter().flat_map(|w| w.texts.iter()).collect();
        assert_eq!(texts.len(), expect, "every declared label must be placed");
        for t in &texts {
            let size = t.attrs.number("size").unwrap_or(11.0);
            let w = size * 0.55 * t.content.chars().count() as f64;
            let h = size * 1.2;
            let (x, y) = t.position;
            for leaf in leaves {
                let (x0, y0, x1, y1) = lini::testing::node_rect(laid, leaf).expect("leaf placed");
                let apart = x + w / 2.0 <= x0
                    || x1 <= x - w / 2.0
                    || y + h / 2.0 <= y0
                    || y1 <= y - h / 2.0;
                assert!(apart, "label '{}' overlaps {leaf}", t.content);
            }
        }
    };

    let simple = read(std::path::Path::new("samples/links_simple.lini"));
    let laid = lini::testing::route_sample(&simple, 10.0);
    leaf_clear(&laid, &["ant", "wall", "bee", "owl"], 3);

    let hard = read(std::path::Path::new("samples/links_hard.lini"));
    let laid = lini::testing::route_sample(&hard, 8.0);
    leaf_clear(
        &laid,
        &[
            "alpha",
            "beta",
            "gamma",
            "delta",
            "hub",
            "west.ww1",
            "west.ww2",
            "north.nn1",
            "north.nn2",
            "south.ss1",
            "south.ss2",
            "east.ee1",
            "east.ee2",
        ],
        5,
    );
    // The chain label sits on exactly one of the chain's two segments.
    let relay: Vec<_> = laid
        .links
        .iter()
        .filter(|w| w.data_from == "gamma" && w.data_to == "delta")
        .collect();
    assert_eq!(relay.len(), 2, "the relay chain draws two segments");
    assert_eq!(
        relay.iter().map(|w| w.texts.len()).sum::<usize>(),
        1,
        "one 'relay' label on the whole chain"
    );
}
