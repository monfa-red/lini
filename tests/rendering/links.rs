use super::*;

// ── Text leaves: a node's text and a link's label share one renderer ──

#[test]
fn link_label_translate_is_applied_once() {
    // Regression: `translate` used to be folded in at routing *and* re-applied at
    // render, doubling the nudge on a link label vs a node's text (SPEC §6/§9).
    // The shared text emitter applies it once. Both ends sit at y=0, so a clean
    // -10 nudge must land the label at exactly y="-10".
    let svg = render_live(
        "{ direction: row; gap: 120 }\n|box#a|\n|box#b|\na -> b [ \"L\" { translate: 0 -10 } ]\n",
    );
    let tag = svg
        .lines()
        .find(|l| l.contains(r#"<text class="lini-link-label""#))
        .expect("a link label");
    assert!(tag.contains(r#"y="-10""#), "translate once → y=-10: {tag}");
    assert!(!tag.contains(r#"y="-20""#), "not doubled: {tag}");
}

#[test]
fn link_label_supports_multiline_and_letter_spacing() {
    // A link label is an ordinary styleable text leaf (SPEC §3/§9), so the same
    // multi-line `\n` tspans and baked `letter-spacing` dx a node's text gets must
    // reach it too — the two render through one path.
    let svg = render_live("|box#a|\n|box#b|\na -> b [ \"AB\\nCD\" { letter-spacing: 5 } ]\n");
    let label = svg
        .split(r#"<text class="lini-link-label""#)
        .nth(1)
        .and_then(|s| s.split("</text>").next())
        .expect("a link label");
    assert!(label.contains("<tspan"), "multi-line tspans: {label}");
    assert!(
        label.contains(r#"dx="0 5""#),
        "baked letter-spacing: {label}"
    );
}

#[test]
fn a_scoped_link_rule_dashes_exactly_one_arm() {
    // A containment-shaped link (endpoints X and X.path) cascades as if written
    // in X [SPEC 9/12], so `#cto |-|` reaches cto's OWN spokes — the fan
    // `cto:bottom - cto.be & cto.fe` is textually written in ceo's body, but its
    // outer endpoint is cto — and no other arm. ceo's and coo's spokes stay
    // solid.
    let src = "{\n  layout: tree;\n  #cto |-| { stroke-style: dashed; }\n}\n\
        |topic#ceo| \"CEO\" [\n\
          |topic#cto| \"CTO\" [\n\
            |topic#be| \"BE\"\n\
            |topic#fe| \"FE\"\n\
          ]\n\
          |topic#coo| \"COO\" [\n\
            |topic#ops| \"Ops\"\n\
          ]\n\
        ]\n";
    let svg = render_live(src);
    let (dashed, solid) = link_targets(&svg);
    assert_eq!(
        dashed,
        ["ceo.cto.be", "ceo.cto.fe"],
        "exactly cto's two spokes dash"
    );
    assert_eq!(
        solid,
        ["ceo.cto", "ceo.coo", "ceo.coo.ops"],
        "ceo's and coo's spokes stay solid"
    );
}

#[test]
fn the_arm_rule_reaches_the_whole_subtree() {
    // With grandchildren under be, `#cto |-|` dashes the whole arm: cto's own
    // spokes AND be's fan (every chain passes through cto) [SPEC 9/12].
    let src = "{\n  layout: tree;\n  #cto |-| { stroke-style: dashed; }\n}\n\
        |topic#ceo| \"CEO\" [\n\
          |topic#cto| \"CTO\" [\n\
            |topic#be| \"BE\" [ |topic#api| \"API\" ]\n\
            |topic#fe| \"FE\"\n\
          ]\n\
          |topic#coo| \"COO\"\n\
        ]\n";
    let svg = render_live(src);
    let (dashed, solid) = link_targets(&svg);
    assert_eq!(
        dashed,
        ["ceo.cto.be", "ceo.cto.fe", "ceo.cto.be.api"],
        "the whole cto arm dashes"
    );
    assert_eq!(solid, ["ceo.cto", "ceo.coo"], "other spokes stay solid");
}

#[test]
fn natural_routing_renders_cubics_deterministically() {
    // A row tree with `routing: natural` [SPEC 9]: every branch wire draws as
    // straight stubs plus exact cubic segments — `C` commands in the link
    // path `d` — and reruns are byte-identical (ROUTING.md Law 4).
    let src = "{ layout: tree; direction: row; routing: natural }\n\
        |topic#root| \"Root\" [\n\
          |topic#a| \"Alpha\"\n\
          |topic#b| \"Beta\"\n\
          |topic#c| \"Gamma\"\n\
        ]\n";
    let svg = render_live(src);
    let wires: Vec<&str> = svg
        .lines()
        .skip_while(|l| !l.contains("lini-links"))
        .filter(|l| l.trim_start().starts_with("<path d=\""))
        .collect();
    assert_eq!(wires.len(), 3, "three branch wires");
    for w in &wires {
        assert!(w.contains(" C "), "a natural wire draws cubics: {w}");
        assert!(!w.contains(" A "), "no render-time fillet arcs: {w}");
    }
    assert_eq!(svg, render_live(src), "byte-identical rerun");
}

/// The Stage-5 mindmap scene the palette-walk render tests share: three named
/// branches (one with a subtopic), an anonymous branch, and a cross-link.
const MINDMAP: &str = "|mindmap#m| \"Plan\" [\n\
      |topic#a| \"Alpha\" [ |topic#a1| \"Deep\" ]\n\
      |topic#b| \"Beta\"\n\
      |topic#c| \"Gamma\"\n\
      |topic| \"Delta\"\n\
      a.a1 --- c\n\
    ]\n";

#[test]
fn the_palette_walk_tints_cards_and_wires_and_leaves_root_and_cross_links_neutral() {
    let svg = render_live(MINDMAP);
    // The root topic is neutral: level-0, no hue class, no hue paint.
    let root = svg
        .lines()
        .find(|l| l.contains("data-id=\"m\""))
        .expect("root node");
    assert!(root.contains("lini-level-0"), "level hook: {root}");
    assert!(!root.contains("lini-hue-"), "root neutral: {root}");
    // Branch cards tint at the tiers (wash fill, deep stroke, ink text) and
    // wear their level hook.
    let a = svg
        .lines()
        .find(|l| l.contains("data-id=\"a\""))
        .expect("branch a");
    for want in ["lini-level-1", "lini-hue-rose"] {
        assert!(a.contains(want), "{want}: {a}");
    }
    // The tint rides the emitted CSS rule, never inline on each wearer
    // [SPEC 17] — the card's `<g>` carries the classes and no hue paint.
    assert!(!a.contains("style="), "card free of inline paint: {a}");
    assert!(
        svg.contains(
            ".lini .lini-mindmap .lini-hue-rose { fill: var(--lini-rose-wash); \
             stroke: var(--lini-rose-deep); color: var(--lini-rose-ink); }"
        ),
        "the hue rule is real CSS: {svg}"
    );
    // Every branch wire tints — the root arm (written in the scene scope) and
    // the subtree wire alike, one generated rule each [SPEC 8].
    for (to, hue) in [
        ("data-to=\"m.a\"", "rose"),
        ("data-to=\"m.b\"", "orange"),
        ("data-to=\"m.c\"", "amber"),
        ("data-to=\"m.lini-topic-4\"", "lime"),
        ("data-to=\"m.a.a1\"", "rose"),
    ] {
        let wire = svg
            .lines()
            .find(|l| l.contains("lini-link") && l.contains(to))
            .unwrap_or_else(|| panic!("wire {to}"));
        assert!(wire.contains(&format!("lini-hue-{hue}")), "{to}: {wire}");
        assert!(
            !wire.contains("stroke:"),
            "the wire's tint rides the .lini-links companion rule: {wire}"
        );
        assert!(
            svg.contains(&format!(".lini .lini-links .lini-hue-{hue}")),
            "companion rule for {hue}: {svg}"
        );
    }
    // The authored cross-link keeps the neutral link default.
    let cross = svg
        .lines()
        .find(|l| l.contains("data-from=\"m.a.a1\"") && l.contains("data-to=\"m.c\""))
        .expect("cross-link");
    assert!(
        !cross.contains("lini-hue-") && !cross.contains("stroke: var(--lini-"),
        "cross-link neutral: {cross}"
    );
}

#[test]
fn authored_paint_beats_the_palette_walk() {
    // Explicit author paint wins: the generated tints are descendant rules, so
    // an inline block (and any user id/class rule) sits above them [SPEC 4/8].
    let src = "{ #b { stroke: --purple-deep; } }\n\
        |mindmap#m| \"Plan\" [\n\
          |topic#a| \"Alpha\" { fill: --amber-wash; }\n\
          |topic#b| \"Beta\"\n\
        ]\n";
    let svg = render_live(src);
    let a = svg
        .lines()
        .find(|l| l.contains("data-id=\"a\""))
        .expect("branch a");
    assert!(
        a.contains("fill: var(--lini-amber-wash)"),
        "inline fill wins over the rose wash: {a}"
    );
    // The untouched channels keep the walk *through the CSS rule* — the diff
    // inlines only the authored difference, never the rule's own values.
    assert!(
        !a.contains("stroke:"),
        "the walk's stroke rides the hue rule, not the wearer: {a}"
    );
    let b = svg
        .lines()
        .find(|l| l.contains("data-id=\"b\""))
        .expect("branch b");
    assert!(
        b.contains("stroke: var(--lini-purple-deep)"),
        "an id rule beats the generated descendant tint: {b}"
    );
}

#[test]
fn a_mindmap_compiles_transparent_to_its_desugar() {
    // The oracle law holds off-samples too: compiling the lowered mindmap —
    // seated scope, tinted per-branch arms, garnish rules — byte-matches
    // compiling the source (fan grouping included).
    let lowered = lini::desugar_source(MINDMAP).expect("desugar");
    assert_eq!(
        render_baked(MINDMAP),
        render_baked(&lowered),
        "compile(src) != compile(desugar(src))"
    );
}

#[test]
fn mindmap_root_arms_share_one_trunk_port_per_side() {
    // Per-branch root arms are separate statements so each wears its own hue,
    // yet they form one crow's-foot per side: a node's forced-port wires into
    // its own descendants fan across statements (the containment gate).
    let svg = render_live(MINDMAP);
    let mut starts: Vec<(String, String)> = Vec::new();
    for l in svg.lines() {
        if !l.contains("data-from=\"m\"") {
            continue;
        }
        let path = svg
            .lines()
            .skip_while(|x| *x != l)
            .find(|x| x.trim_start().starts_with("<path d=\""))
            .expect("wire path");
        let d = path.trim_start().strip_prefix("<path d=\"M ").unwrap();
        let xy: Vec<&str> = d.split(' ').take(2).collect();
        let to = &l[l.find("data-to=\"").unwrap() + 9..];
        starts.push((xy.join(" "), to[..to.find('"').unwrap()].to_string()));
    }
    assert_eq!(starts.len(), 4, "four root arms: {starts:?}");
    let mut ports: Vec<&str> = starts.iter().map(|(p, _)| p.as_str()).collect();
    ports.sort_unstable();
    ports.dedup();
    assert_eq!(
        ports.len(),
        2,
        "one shared port per side, not one per arm: {starts:?}"
    );
}
