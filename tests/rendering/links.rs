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
    // [SPEC 18] — the card's `<g>` carries the classes and no hue paint.
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
        let to = scrape(l, "data-to=\"")[0];
        starts.push((xy.join(" "), to.to_string()));
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

// ── The schematic's wire dress [SPEC 16.5/16.6] ──

/// A sheet whose one fan is dotted, with `extra` rules in the root block and
/// `tail` appended to the statements.
fn sch_sheet(extra: &str, tail: &str) -> String {
    format!(
        "{{ layout: schematic; {extra} }}\n\
         |component#u1| [\n|pin#a|; |pin#b|; |pin#c|\n]\n\
         |component#u2| [\n|pin#a|; |pin#b|; |pin#c|\n]\n\
         u1.c - u2.a{tail}\nu1.c - u2.b\n"
    )
}

/// Every drawn wire `d=` of an SVG.
fn wire_ds(svg: &str) -> Vec<String> {
    scrape(svg, "<path d=\"")
        .into_iter()
        .map(str::to_string)
        .collect()
}

#[test]
fn a_schematic_wire_bends_square_and_every_other_wire_still_rounds() {
    // `corner-radius: 0` is the scope's own link default [SPEC 16.5], so the
    // fillet pass draws no arc at a bend — the `d` is lines end to end.
    let sch = wire_ds(&render_live(&sch_sheet("", "")));
    assert!(sch.len() >= 2, "the sheet drew: {sch:?}");
    assert!(
        sch.iter().all(|d| !d.contains(" A ")),
        "square corners: {sch:?}"
    );
    assert!(sch.iter().any(|d| d.matches(" L ").count() >= 3), "{sch:?}");
    // …and `auto` is untouched everywhere else: an ordinary scene's dogleg still
    // rounds at the clearance-derived cap.
    let flow =
        render_live("{ direction: row; gap: 100 }\n|box#a|\n|box#b| { translate: 0 60 }\na -> b\n");
    assert!(
        wire_ds(&flow).iter().any(|d| d.contains(" A ")),
        "{:?}",
        wire_ds(&flow)
    );
}

#[test]
fn an_authored_corner_radius_beats_the_scopes_square_default() {
    // The scope default rides the link **base layer**, below every rule and
    // block [SPEC 17], so both spellings of an authored radius win it back.
    for src in [
        sch_sheet("|-| { corner-radius: 6 }", ""),
        sch_sheet("", " { corner-radius: 6 }"),
    ] {
        let ds = wire_ds(&render_live(&src));
        assert!(ds.iter().any(|d| d.contains(" A ")), "{ds:?}");
    }
}

#[test]
fn the_junction_dot_paints_through_one_rule_and_a_rule_removes_it() {
    // The dot authors no `style=` of its own [SPEC 18]: its whole look is the
    // single `.lini-junction` rule, which is exactly why overriding that rule
    // reaches every dot on the sheet.
    let svg = render_live(&sch_sheet("", ""));
    let dot = svg
        .lines()
        .find(|l| l.contains("lini-junction"))
        .expect("a junction dot");
    assert!(!dot.contains("style="), "no inline diff: {dot}");
    assert!(
        svg.contains(".lini .lini-junction { fill: var(--lini-wire); stroke: none; }"),
        "one rule states the look"
    );
    // The dot is wire chrome: it draws inside the wiring group, over the lines.
    let (links_at, dot_at) = (
        svg.find("<g class=\"lini-links\">")
            .expect("the wiring group"),
        svg.find("lini-junction lini-oval").expect("the dot"),
    );
    assert!(dot_at > links_at, "the dot draws over the wires");
    // Hidden by a rule — and nothing else on the sheet changes.
    let hidden = render_live(&sch_sheet("|junction| { fill: none; stroke: none }", ""));
    assert!(hidden.contains(".lini .lini-junction { fill: none; stroke: none; }"));
    assert_eq!(wire_ds(&svg), wire_ds(&hidden), "the wires are untouched");
    // `--lini-wire` stays in the block: the wires wear the same role, so the
    // shake keeps it for them (the unit test walks all four directions).
    assert!(hidden.contains("--lini-wire:"), "the wires still wear it");
}

#[test]
fn the_scopes_wires_wear_the_wire_role_through_one_rule() {
    // [SPEC 16.6] The classic dress rides the schematic scope's **link base
    // layer**, so a root sheet states it exactly once — `.lini-link` — and no
    // wire authors a `style=` diff against it.
    let svg = render_live(&sch_sheet("", ""));
    assert!(
        svg.contains(
            ".lini .lini-link { fill: none; stroke: var(--lini-wire); stroke-width: 1.5; \
             stroke-dasharray: none; }"
        ),
        "one rule states the wire look: {svg}"
    );
    for line in svg.lines().filter(|l| l.contains(r#"class="lini-link"#)) {
        assert!(!line.contains("style="), "no inline diff: {line}");
    }
    // …and an ordinary scene is untouched: the base layer is the scope's, not
    // every link's.
    let flow = render_live("{ direction: row }\n|box#a|\n|box#b|\na - b\n");
    assert!(
        flow.contains(
            ".lini .lini-link { fill: none; stroke: var(--lini-stroke); stroke-width: 2;"
        ),
        "{flow}"
    );
}

#[test]
fn an_authored_wire_paint_beats_the_scopes_dress() {
    // The base layer sits below every rule and block [SPEC 17] — which is why
    // the dress is a scope default and not a class: a class would out-specify
    // the author. A root `|-|` rule replaces it wholesale…
    let ruled = render_live(&sch_sheet("|-| { stroke: --accent; stroke-width: 3 }", ""));
    assert!(
        ruled.contains(
            ".lini .lini-link { fill: none; stroke: var(--lini-accent); stroke-width: 3;"
        ),
        "the rule wins the whole sheet: {ruled}"
    );
    // …and one wire's own block inlines its diff, leaving its neighbour dressed.
    let blocked = render_live(&sch_sheet("", " { stroke: --accent; stroke-width: 3 }"));
    assert!(
        blocked.contains(
            ".lini .lini-link { fill: none; stroke: var(--lini-wire); stroke-width: 1.5;"
        ),
        "the scope still dresses the rest: {blocked}"
    );
    let wire = blocked
        .lines()
        .find(|l| l.contains(r#"data-from="u1.c" data-to="u2.a""#))
        .expect("the authored wire");
    assert!(
        wire.contains(r#"style="stroke: var(--lini-accent); stroke-width: 3""#),
        "the block wins its own wire: {wire}"
    );
}

#[test]
fn the_sheet_wash_rides_a_rule_at_the_root_and_on_a_nested_scope() {
    // [SPEC 16.6] The scene takes `--lini-sheet`: a root sheet through the
    // backing plate's own rule, a nested `|schematic|` through its type rule.
    // Neither inlines — the wash is stated once per scope shape.
    let root = render_live(&sch_sheet("", ""));
    assert!(
        root.contains(".lini .lini-canvas { fill: var(--lini-sheet); }"),
        "{root}"
    );
    assert!(
        root.contains(r#"<rect class="lini-canvas" x="#) && !root.contains("--lini-sheet)\""),
        "the plate inlines nothing: {root}"
    );
    let nested = render_live(
        "|box#note| \"n\"\n|schematic#s| [\n|component#u1| [\n|pin#a|; |pin#b|; |pin#c|\n]\n]\n",
    );
    assert!(
        nested.contains(".lini .lini-schematic { fill: var(--lini-sheet); }"),
        "{nested}"
    );
    // …and the page behind a nested sheet is still the ordinary background.
    assert!(
        nested.contains(".lini .lini-canvas { fill: var(--lini-bg); }"),
        "{nested}"
    );
}

#[test]
fn a_theme_retunes_the_whole_schematic_family_from_one_place() {
    // [SPEC 10.1/16.6] Every part of the classic look is a `--lini-*` role, so
    // one theme file re-dresses the sheet — the KiCad-esque alternative needs no
    // built-in, and no colour is restated anywhere in the engine.
    let theme = "\
        :root, .lini { --lini-wire: #008484; --lini-sheet: #fffef0; \
        --lini-component-fill: #ffffc2; --lini-component-stroke: #840000; \
        --lini-label-ink: #006464; --lini-pin-number: #840000; }";
    // A sheet wearing all six roles: parts, numbered pins, wires, a net tag.
    let sheet = "{ layout: schematic }\n\
         |component#u1| [\n|pin#a| { number: 1 }; |pin#b| { number: 2 }; |pin#c| { number: 3 }\n]\n\
         |gnd#g1|\nu1.c - g1\n";
    let svg = lini::compile_str_with(
        sheet,
        &lini::Options {
            theme_css: Some(theme.to_string()),
            ..Default::default()
        },
    )
    .expect("compile");
    for (role, value) in [
        ("wire", "#008484"),
        ("sheet", "#fffef0"),
        ("component-fill", "#ffffc2"),
        ("component-stroke", "#840000"),
        ("label-ink", "#006464"),
        ("pin-number", "#840000"),
    ] {
        assert!(
            svg.contains(&format!("--lini-{role}: {value};")),
            "{role} retunes: {svg}"
        );
    }
}

#[test]
fn a_sheet_never_opens_a_trace() {
    // [SPEC 16.5] the knockout is the **diagram** convention — a label rides
    // its wire and the wire opens behind it [SPEC 9]. A schematic scope draws
    // in the other one: the net name stands beside the line, so no wire is
    // ever cut and, nothing wearing them, the mask rules go unemitted
    // [SPEC 18].
    let sheet = "{ layout: schematic }\n\
                 |component#u1| [ |pin#a| { side: right }; |pin#b|; |pin#c| ]\n\
                 |component#u2| [ |pin#d| { side: left }; |pin#e|; |pin#f| ]\n\
                 u1.a - u2.d \"VBUS\"\n";
    let svg = render_live(sheet);
    assert!(!svg.contains("lini-label-cut"), "no wire is cut: {svg}");
    assert!(!svg.contains("lini-cut"), "and no rule is worn: {svg}");
    // The same statement in a diagram still cuts — the convention is the
    // scope's, not the placement's.
    let diagram = "|box#a| { width: 60 }\n|box#b| { width: 60 }\na - b \"VBUS\"\n";
    let svg = render_live(diagram);
    assert!(svg.contains("lini-label-cut"), "a diagram cuts: {svg}");
    assert!(svg.contains(".lini-cut {"), "and states the rule: {svg}");
}
