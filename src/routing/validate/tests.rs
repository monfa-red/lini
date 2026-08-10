use super::*;
use crate::layout::ir::Bbox;
use crate::resolve::{AttrMap, Markers, NodeKind, ResolvedValue};

pub(super) fn sized(id: &str, cx: f64, cy: f64, w: f64, h: f64) -> PlacedNode {
    PlacedNode {
        id: Some(id.to_owned()),
        kind: NodeKind::Block,
        type_chain: Vec::new(),
        applied_styles: Vec::new(),
        label: None,
        attrs: AttrMap::default(),
        own_style: AttrMap::default(),
        markers: Markers::default(),
        cx,
        cy,
        bbox: Bbox::centered(w, h),
        rotation: 0.0,
        children: Vec::new(),
        gutters: Vec::new(),
        links: Vec::new(),
        sketch: None,
        origin: (0.0, 0.0),
        span: Span::empty(),
    }
}

pub(super) fn body(id: &str, cx: f64, cy: f64) -> PlacedNode {
    sized(id, cx, cy, 40.0, 40.0)
}

pub(super) fn link(from: &str, to: &str, path: Vec<(f64, f64)>) -> RoutedLink {
    let mut attrs = AttrMap::default();
    attrs.insert("clearance", ResolvedValue::Number(8.0));
    RoutedLink {
        path,
        curve: Vec::new(),
        strategy: Strategy::Orthogonal,
        markers: Markers::default(),
        attrs,
        applied_styles: Vec::new(),
        texts: Vec::new(),
        data_from: from.to_owned(),
        data_to: to.to_owned(),
        seg_from: from.to_owned(),
        seg_to: to.to_owned(),
        decl_span: Span::empty(),
        fan_from: None,
        fan_to: None,
        port_from: None,
        port_to: None,
    }
}

pub(super) fn rules(violations: &[Violation]) -> Vec<Rule> {
    violations.iter().map(|v| v.rule).collect()
}

/// a at origin, b to the right, both 40×40.
pub(super) fn pair() -> Vec<PlacedNode> {
    vec![body("a", 0.0, 0.0), body("b", 200.0, 0.0)]
}

#[test]
fn a_clean_straight_link_is_silent() {
    let w = link("a", "b", vec![(20.0, 0.0), (180.0, 0.0)]);
    let out = check(&pair(), &[w], &[]);
    assert_eq!(out.len(), 0, "{out:?}");
}

#[test]
fn a_straight_strategy_wire_is_exempt_from_the_laws() {
    // Oblique, corner-grazing, avoidance-free — lawful for `straight`
    // (ROUTING.md Strategies), so the orthogonal checker keeps silent.
    let mut w = link("a", "b", vec![(20.0, 20.0), (180.0, -20.0)]);
    w.strategy = Strategy::Straight;
    let out = check(&pair(), &[w], &[]);
    assert_eq!(out.len(), 0, "{out:?}");
}

#[test]
fn clearance_fires_on_a_grazing_segment() {
    // The detour passes 4 over the blocking body — clearance is 8.
    let nodes = vec![
        body("a", 0.0, 0.0),
        body("b", 200.0, 0.0),
        body("wall", 100.0, 0.0),
    ];
    let w = link(
        "a",
        "b",
        vec![
            (20.0, 0.0),
            (50.0, 0.0),
            (50.0, -24.0),
            (150.0, -24.0),
            (150.0, 0.0),
            (180.0, 0.0),
        ],
    );
    let out = check(&nodes, &[w], &[]);
    assert!(rules(&out).contains(&Rule::Clearance), "{out:?}");
}

#[test]
fn clearance_fires_inside_the_links_own_keepout() {
    // A middle segment sweeps back 4 over its own source body.
    let w = link(
        "a",
        "b",
        vec![
            (20.0, 0.0),
            (60.0, 0.0),
            (60.0, -24.0),
            (0.0, -24.0),
            (0.0, -60.0),
            (240.0, -60.0),
            (240.0, 0.0),
            (220.0, 0.0),
        ],
    );
    let out = check(&pair(), &[w], &[]);
    assert!(rules(&out).contains(&Rule::Clearance), "{out:?}");
}

#[test]
fn contact_fires_on_corner_oblique_and_diagonal_landings() {
    let corner = link("a", "b", vec![(20.0, -20.0), (180.0, -20.0)]);
    let graze = link("a", "b", vec![(20.0, -15.0), (180.0, -15.0)]);
    let oblique = link(
        "a",
        "b",
        vec![(20.0, 0.0), (20.0, -40.0), (180.0, -40.0), (180.0, 0.0)],
    );
    let diagonal = link("a", "b", vec![(20.0, 0.0), (170.0, -10.0), (180.0, 0.0)]);
    for w in [corner, graze, oblique, diagonal] {
        let out = check(&pair(), &[w], &[]);
        assert!(rules(&out).contains(&Rule::Contact), "{out:?}");
    }
}

#[test]
fn a_fixed_port_lands_exactly_and_waives_the_corner_margin() {
    // Ports 4 from the corner of a 40-tall body: a free end breaches
    // the corner margin (clearance 8); a fixed port owns its landing.
    let free = link("a", "b", vec![(20.0, -16.0), (180.0, -16.0)]);
    let out = check(&pair(), &[free], &[]);
    assert!(rules(&out).contains(&Rule::Contact), "{out:?}");
    let mut pinned = link("a", "b", vec![(20.0, -16.0), (180.0, -16.0)]);
    pinned.port_from = Some((Side::Right, -16.0));
    pinned.port_to = Some((Side::Left, -16.0));
    let out = check(&pair(), &[pinned], &[]);
    assert_eq!(out.len(), 0, "{out:?}");
    // An end drawn off its fixed port is a contact breach, exactly.
    let mut off = link("a", "b", vec![(20.0, 0.0), (180.0, 0.0)]);
    off.port_from = Some((Side::Right, -2.0));
    let out = check(&pair(), &[off], &[]);
    assert!(
        out.iter()
            .any(|v| v.detail.contains("misses its fixed port")),
        "{out:?}"
    );
}

#[test]
fn pinned_ports_excuse_a_sub_clearance_hug_free_ones_do_not() {
    // Two straights 5 apart (clearance 8, floor 4) between 100-tall
    // bodies: free ends had room to spread — a breach; ends pinned to
    // fixed ports 5 apart cannot — scarcity, excused.
    let nodes = vec![
        sized("a", 0.0, 0.0, 40.0, 100.0),
        sized("b", 200.0, 0.0, 40.0, 100.0),
    ];
    let mk = |y: f64| link("a", "b", vec![(20.0, y), (180.0, y)]);
    let (mut w1, mut w2) = (mk(0.0), mk(5.0));
    let out = check(&nodes, &[w1.clone(), w2.clone()], &[]);
    assert!(rules(&out).contains(&Rule::Separation), "{out:?}");
    w1.port_from = Some((Side::Right, 0.0));
    w1.port_to = Some((Side::Left, 0.0));
    w2.port_from = Some((Side::Right, 5.0));
    w2.port_to = Some((Side::Left, 5.0));
    let out = check(&nodes, &[w1, w2], &[]);
    assert_eq!(out.len(), 0, "{out:?}");
}

#[test]
fn separation_fires_below_the_half_clearance_floor() {
    // Two rails 3 apart: below clearance/2 = 4 — no excuse exists.
    let nodes = vec![
        sized("a", 0.0, 0.0, 40.0, 100.0),
        sized("b", 200.0, 0.0, 40.0, 100.0),
    ];
    let w1 = link("a", "b", vec![(20.0, 0.0), (180.0, 0.0)]);
    let w2 = link("a", "b", vec![(20.0, 3.0), (180.0, 3.0)]);
    let out = check(&nodes, &[w1, w2], &[]);
    assert!(
        out.iter()
            .any(|v| v.rule == Rule::Separation && v.detail.contains("floor")),
        "{out:?}"
    );
}

#[test]
fn a_squeeze_with_room_to_spare_is_flagged() {
    // Five rails at pitch 5 between 100-tall boxes: the shared window
    // (84) and the corridor both hold five wires at full clearance, so
    // the sub-clearance hug has no excuse.
    let nodes = vec![
        sized("a", 0.0, 0.0, 40.0, 100.0),
        sized("b", 200.0, 0.0, 40.0, 100.0),
    ];
    let links: Vec<RoutedLink> = (0..5)
        .map(|i| {
            let y = -10.0 + 5.0 * i as f64;
            link("a", "b", vec![(20.0, y), (180.0, y)])
        })
        .collect();
    let out = check(&nodes, &links, &[]);
    assert!(rules(&out).contains(&Rule::Separation), "{out:?}");
}

#[test]
fn a_full_side_excuses_its_compressed_ports() {
    // Four rails at the pitch floor between 28-tall boxes: the lawful
    // window is 28 − 2·8 = 12, four ports at full clearance need 24 —
    // the side cannot hold them, so the compression stands.
    let nodes = vec![
        sized("a", 0.0, 0.0, 40.0, 28.0),
        sized("b", 200.0, 0.0, 40.0, 28.0),
    ];
    let links: Vec<RoutedLink> = (0..4)
        .map(|i| {
            let y = -6.0 + 4.0 * i as f64;
            link("a", "b", vec![(20.0, y), (180.0, y)])
        })
        .collect();
    let out = check(&nodes, &links, &[]);
    assert_eq!(out.len(), 0, "{out:?}");
}

#[test]
fn a_pinched_corridor_excuses_the_compression() {
    // Two wires drop through a 4-wide slot between two tall walls —
    // their vertical legs 4 apart (the floor, exactly). The corridor's
    // usable width cannot hold two wires at clearance 8: excused.
    let mut nodes = vec![
        sized("ww", -35.0, 0.0, 50.0, 200.0),
        sized("we", 35.0, 0.0, 50.0, 200.0),
    ];
    nodes.push(sized("a1", -60.0, -150.0, 40.0, 20.0));
    nodes.push(sized("a2", 50.0, -150.0, 40.0, 20.0));
    nodes.push(sized("b1", -60.0, 150.0, 40.0, 20.0));
    nodes.push(sized("b2", 50.0, 150.0, 40.0, 20.0));
    let w1 = link(
        "a1",
        "b1",
        vec![
            (-40.0, -150.0),
            (-2.0, -150.0),
            (-2.0, 150.0),
            (-40.0, 150.0),
        ],
    );
    let w2 = link(
        "a2",
        "b2",
        vec![(30.0, -150.0), (2.0, -150.0), (2.0, 150.0), (30.0, 150.0)],
    );
    let out = check(&nodes, &[w1, w2], &[]);
    assert_eq!(out.len(), 0, "{out:?}");
}

#[test]
fn the_same_hug_in_a_roomy_corridor_is_flagged() {
    // Identical wires, walls pulled apart to a 36-wide slot: room for
    // both at clearance, so the 4-gap is an engine bug.
    let mut nodes = vec![
        sized("ww", -51.0, 0.0, 50.0, 200.0),
        sized("we", 51.0, 0.0, 50.0, 200.0),
    ];
    nodes.push(sized("a1", -60.0, -150.0, 40.0, 20.0));
    nodes.push(sized("a2", 50.0, -150.0, 40.0, 20.0));
    nodes.push(sized("b1", -60.0, 150.0, 40.0, 20.0));
    nodes.push(sized("b2", 50.0, 150.0, 40.0, 20.0));
    let w1 = link(
        "a1",
        "b1",
        vec![
            (-40.0, -150.0),
            (-2.0, -150.0),
            (-2.0, 150.0),
            (-40.0, 150.0),
        ],
    );
    let w2 = link(
        "a2",
        "b2",
        vec![(30.0, -150.0), (2.0, -150.0), (2.0, 150.0), (30.0, 150.0)],
    );
    let out = check(&nodes, &[w1, w2], &[]);
    assert!(rules(&out).contains(&Rule::Separation), "{out:?}");
}

#[test]
fn crossings_reconcile_against_the_report_both_ways() {
    let nodes = vec![
        body("a", 0.0, 0.0),
        body("b", 200.0, 0.0),
        body("c", 100.0, -100.0),
        body("d", 100.0, 100.0),
    ];
    let w1 = link("a", "b", vec![(20.0, 0.0), (180.0, 0.0)]);
    let w2 = link("c", "d", vec![(100.0, -80.0), (100.0, 80.0)]);
    let entry = |links: Vec<String>| Violation {
        rule: Rule::Crossing,
        severity: Severity::Info,
        links,
        detail: String::new(),
        span: Span::empty(),
    };

    // Drawn but unnamed: the checker flags the crossing.
    let out = check(&nodes, &[w1.clone(), w2.clone()], &[]);
    assert!(
        out.iter()
            .any(|v| v.rule == Rule::Crossing && v.severity == Severity::Warning),
        "{out:?}"
    );

    // Named exactly once: silent.
    let named = entry(vec!["a -> b".to_owned(), "c -> d".to_owned()]);
    let out = check(
        &nodes,
        &[w1.clone(), w2.clone()],
        std::slice::from_ref(&named),
    );
    assert_eq!(out.len(), 0, "{out:?}");

    // Named but not drawn: the phantom is flagged.
    let phantom = entry(vec!["a -> b".to_owned(), "x -> y".to_owned()]);
    let out = check(&nodes, &[w1, w2], &[named, phantom]);
    assert!(
        out.iter()
            .any(|v| v.detail.contains("named in the report but")),
        "{out:?}"
    );
}

#[test]
fn a_link_crossing_itself_is_flagged() {
    // A hook whose final approach sweeps back through the link's own run.
    let w = link(
        "a",
        "b",
        vec![
            (20.0, 0.0),
            (60.0, 0.0),
            (60.0, 60.0),
            (230.0, 60.0),
            (230.0, -9.0),
            (239.0, -9.0),
            (239.0, 0.0),
            (180.0, 0.0),
        ],
    );
    let out = check(&pair(), &[w], &[]);
    assert!(
        out.iter().any(|v| v.detail.contains("crosses itself")),
        "{out:?}"
    );
}

#[test]
fn fan_siblings_share_their_trunk_without_separation_noise() {
    let nodes = vec![
        body("a", 0.0, 0.0),
        body("b", 200.0, 0.0),
        body("c", 100.0, 160.0),
    ];
    let mut w1 = link("a", "b", vec![(20.0, 0.0), (180.0, 0.0)]);
    let mut w2 = link("a", "c", vec![(20.0, 0.0), (100.0, 0.0), (100.0, 140.0)]);
    // Untagged, the trunk overlap and split T-joint breach separation…
    let out = check(&nodes, &[w1.clone(), w2.clone()], &[]);
    assert!(rules(&out).contains(&Rule::Separation), "{out:?}");
    // …as fan siblings they are one drawn line.
    w1.fan_from = Some(0);
    w2.fan_from = Some(0);
    let out = check(&nodes, &[w1, w2], &[]);
    assert_eq!(out.len(), 0, "{out:?}");
}

#[test]
fn pinned_fan_legs_that_cannot_spread_are_excused() {
    // The case the fan-sibling relaxation actually flipped [Phase 4.6]:
    // the branch's own sheet fans one pin onto two pins of a header, and
    // the legs sit a **pin pitch** apart — 20 — which at clearance 24 is a
    // sub-clearance hug. They cannot spread: a fixed port grants no
    // freedom (ROUTING.md Fixed ports). While fan siblings skipped
    // contention edges the pair was unconnected in the feasibility walk,
    // so each leg "fitted" at its own pinned ordinate and the group read
    // as a breach; sharing the trunk is what made them siblings, and past
    // the split they owe pitch like any wires. EXPECTED-EXCUSED.
    //
    // The measured cost: this excuses a drawn gap of 20 where the law
    // charges 24 — the pin pitch itself — and only where **both** ends are
    // pinned. The same fan with free ends is still a breach, below.
    let nodes = vec![
        sized("a", 0.0, 0.0, 40.0, 100.0),
        sized("b", 200.0, 0.0, 40.0, 100.0),
    ];
    let fan = |split: bool| {
        let path = if split {
            vec![(20.0, 0.0), (100.0, 0.0), (100.0, 20.0), (180.0, 20.0)]
        } else {
            vec![(20.0, 0.0), (180.0, 0.0)]
        };
        let mut w = link("a", "b", path);
        w.attrs
            .insert("clearance", crate::resolve::ResolvedValue::Number(24.0));
        w.fan_from = Some(0);
        w
    };
    let (mut w1, mut w2) = (fan(false), fan(true));
    // Free ends: either leg had room to spread on b's 100-tall side.
    let out = check(&nodes, &[w1.clone(), w2.clone()], &[]);
    assert!(rules(&out).contains(&Rule::Separation), "{out:?}");
    // Pinned to two ports a pin pitch apart, neither can move.
    w1.port_from = Some((Side::Right, 0.0));
    w1.port_to = Some((Side::Left, 0.0));
    w2.port_from = Some((Side::Right, 0.0));
    w2.port_to = Some((Side::Left, 20.0));
    let out = check(&nodes, &[w1, w2], &[]);
    assert_eq!(out.len(), 0, "{out:?}");
}

#[test]
fn fan_legs_past_the_split_owe_pitch_like_any_wires() {
    // The sanctioned contact is the **trunk**, not the fan. Past the
    // split these legs run 5 apart in wide-open space, where either could
    // have spread to the full 8 — a breach, siblings or not. (Only a leg
    // that genuinely cannot spread — one pinned to a fixed port —
    // earns Law 1's scarcity excuse, ROUTING.md Fixed ports.)
    let nodes = vec![
        body("a", 0.0, 0.0),
        body("b", 300.0, 0.0),
        body("c", 300.0, 300.0),
    ];
    let mut w1 = link("a", "b", vec![(20.0, 0.0), (280.0, 0.0)]);
    let mut w2 = link(
        "a",
        "c",
        vec![
            (20.0, 0.0),
            (150.0, 0.0),
            (150.0, 5.0),
            (200.0, 5.0),
            (200.0, 300.0),
            (280.0, 300.0),
        ],
    );
    w1.fan_from = Some(0);
    w2.fan_from = Some(0);
    let out = check(&nodes, &[w1, w2], &[]);
    assert_eq!(rules(&out), vec![Rule::Separation], "{out:?}");
}
