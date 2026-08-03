//! The arity law, judged **end to end** — each case compiled from source
//! through desugar and resolve, so it pins the language's answer wherever the
//! two stages split the work: desugar lands what a scope can see (and prints
//! it), this pass lands what reaches past one. The stage each case exercises is
//! pinned by mutation, not by where its test lives —
//! `a_landing_reaching_into_another_scope_is_this_stages_own` is the one that
//! only this file can satisfy, and `tests/desugar.rs`'s fixed-point pair the
//! ones only the desugar stage can.

use super::LinkKind;
use crate::error::Code;

/// Every hop the program declares, as `(from, to)` resolved paths — one
/// entry per drawn wire, so a chain threading a part shows as two.
fn hops(src: &str) -> Vec<(String, String)> {
    let toks = crate::lexer::lex(src).expect("lex");
    let file = crate::syntax::parser::parse(src, &toks).expect("parse");
    let lowered = crate::desugar::desugar(&file).expect("desugar");
    let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
    program
        .links
        .iter()
        .flat_map(|w| {
            w.endpoints
                .windows(2)
                .map(|h| (h[0].path.clone(), h[1].path.clone()))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[track_caller]
fn err(src: &str) -> crate::error::Error {
    let toks = crate::lexer::lex(src).expect("lex");
    let file = crate::syntax::parser::parse(src, &toks).expect("parse");
    crate::desugar::desugar(&file)
        .and_then(|f| crate::resolve::resolve_with_theme(&f, &[]))
        .err()
        .expect("the wire laws report")
}

/// A root sheet holding `body` — the scope every law below is read in.
fn sheet(body: &str) -> String {
    format!("{{ layout: schematic }}\n{body}")
}

/// A component with `n` pins, named `a`, `b`, `c`.
fn part(id: &str, n: usize) -> String {
    let pins: Vec<String> = ["a", "b", "c"][..n]
        .iter()
        .map(|p| format!("|pin#{p}|"))
        .collect();
    format!("|component#{id}| [ {} ]\n", pins.join("; "))
}

#[test]
fn a_pinless_landing_lands_on_the_one_pin_of_a_one_pin_part() {
    // [SPEC 16.5] one pin, so `- u1` is `- u1.a`; two wires to it land on
    // the same pin (a one-pin part never runs out — the fan is the merge).
    let src = sheet(&(part("u1", 1) + "|gnd#g1|\n|gnd#g2|\nu1 - g1\nu1 - g2\n"));
    assert_eq!(
        hops(&src),
        vec![("u1.a".into(), "g1".into()), ("u1.a".into(), "g2".into())]
    );
}

#[test]
fn a_label_is_its_own_terminal() {
    // [SPEC 16.4] a `|label|` has no pins to choose between — the landing
    // stays the part itself, and two wires share that one connection.
    let src = sheet(&(part("u1", 2) + "|gnd#g1|\nu1.a - g1\nu1.b - g1\n"));
    assert_eq!(
        hops(&src),
        vec![("u1.a".into(), "g1".into()), ("u1.b".into(), "g1".into())]
    );
}

#[test]
fn a_two_pin_part_takes_the_next_free_pin_in_pin_order() {
    // [SPEC 16.5] p1 then p2, in the type's own pin order.
    let src = sheet(&(part("u1", 2) + "|R#r1|\nu1.a - r1\nu1.b - r1\n"));
    assert_eq!(
        hops(&src),
        vec![
            ("u1.a".into(), "r1.p1".into()),
            ("u1.b".into(), "r1.p2".into())
        ]
    );
    // …and a third landing has nowhere to go [SPEC 21].
    let e = err(&sheet(
        &(part("u1", 3) + "|R#R5|\nu1.a - R5\nu1.b - R5\nu1.c - R5\n"),
    ));
    assert_eq!(
        e.message,
        "both pins of 'R5' are wired — name one ('R5.p1')"
    );
    assert_eq!(e.code, Code::SCHEMATIC_ARITY);
}

#[test]
fn a_named_pin_is_a_reservation_wherever_it_is_written() {
    // The bookkeeping reads every statement before any pinless landing
    // chooses, so `- r1` takes p2 whether the explicit p1 came first…
    let before = sheet(&(part("u1", 2) + "|R#r1|\nu1.a - r1.p1\nu1.b - r1\n"));
    assert_eq!(hops(&before).last().expect("two wires").1, "r1.p2");
    // …or later.
    let after = sheet(&(part("u1", 2) + "|R#r1|\nu1.b - r1\nu1.a - r1.p1\n"));
    assert_eq!(hops(&after).first().expect("two wires").1, "r1.p2");
}

#[test]
fn a_dangling_pin_is_legal() {
    // [SPEC 16.5] `|R| -> a` lands p1 and p2 stays open — no warning, no
    // error, and nothing else on the sheet claims it.
    let src = sheet(&(part("u1", 1) + "|R#r1|\nu1 - r1\n"));
    assert_eq!(hops(&src), vec![("u1.a".into(), "r1.p1".into())]);
}

#[test]
fn a_pinless_landing_on_a_three_pin_part_names_one() {
    let e = err(&sheet(&(part("U7", 3) + "|gnd#g1|\nU7 - g1\n")));
    assert_eq!(e.message, "'U7' has 3 pins — name one ('U7.a')");
    assert_eq!(e.code, Code::SCHEMATIC_ARITY);
    // A three-terminal *symbol* part answers from the variant table, not
    // from lowered children — same law, same wording.
    let e = err(&sheet("|Q#q1| { symbol: npn }\n|gnd#g1|\nq1 - g1\n"));
    assert_eq!(e.message, "'q1' has 3 pins — name one ('q1.b')");
}

#[test]
fn a_chain_passes_through_a_two_pin_part() {
    // [SPEC 16.5] `vm - |R| - |gnd|` is a series circuit: entry p1, exit
    // p2 — one statement, two wires.
    let src = sheet(&(part("u1", 1) + "|R#r1|\n|gnd#g1|\nu1 - r1 - g1\n"));
    assert_eq!(
        hops(&src),
        vec![
            ("u1.a".into(), "r1.p1".into()),
            ("r1.p2".into(), "g1".into())
        ]
    );
    // …and it chains on through a second part.
    let series = sheet(&(part("u1", 1) + "|R#r1|\n|LED#d1|\n|gnd#g1|\nu1 - r1 - d1 - g1\n"));
    assert_eq!(
        hops(&series),
        vec![
            ("u1.a".into(), "r1.p1".into()),
            ("r1.p2".into(), "d1.a".into()),
            ("d1.k".into(), "g1".into())
        ]
    );
}

#[test]
fn a_polar_pass_through_enters_the_named_pin_and_leaves_by_the_other() {
    // [SPEC 16.5] `vm - |D|.k - x` enters at the cathode, exits the anode.
    let src = sheet(&(part("u1", 1) + "|D#d1|\n|gnd#g1|\nu1 - d1.k - g1\n"));
    assert_eq!(
        hops(&src),
        vec![("u1.a".into(), "d1.k".into()), ("d1.a".into(), "g1".into())]
    );
    // Both its pins are spent by the pass-through, so a later landing on
    // the part has nowhere to go.
    let e = err(&sheet(
        &(part("u1", 2) + "|D#d1|\n|gnd#g1|\nu1.a - d1.k - g1\nu1.b - d1\n"),
    ));
    assert_eq!(e.message, "both pins of 'd1' are wired — name one ('d1.a')");
}

#[test]
fn a_three_pin_part_mid_chain_is_a_shared_pin_not_a_pass_through() {
    // Only a two-pin part passes through: a named pin on a bigger part is
    // one landing that two hops share (the fan merges them at the port).
    let src = sheet(&(part("u1", 3) + "|gnd#g1|\n|gnd#g2|\ng1 - u1.b - g2\n"));
    assert_eq!(
        hops(&src),
        vec![("g1".into(), "u1.b".into()), ("u1.b".into(), "g2".into())]
    );
}

#[test]
fn a_repeated_pair_is_a_duplicate_wire() {
    // [SPEC 16.5/21] unordered and post-arity: the reversed spelling is
    // the same wire.
    for second in ["u1.a - g1", "g1 - u1.a"] {
        let e = err(&sheet(
            &(part("u1", 3) + "|gnd#g1|\nu1.a - g1\n" + second + "\n"),
        ));
        assert_eq!(
            e.message,
            format!("'{second}' is already wired — a repeated wire means nothing on a sheet"),
            "the repeat is named as its own statement spells it"
        );
        assert_eq!(e.code, Code::DUPLICATE_WIRE);
        assert!(e.related.is_some(), "it points at the first one");
    }
    // Two pinless landings on one part resolve to different pins, so they
    // are two wires, not a repeat.
    let two = sheet(&(part("u1", 2) + "|R#r1|\nu1.a - r1\nu1.a - r1\n"));
    assert_eq!(
        hops(&two),
        vec![
            ("u1.a".into(), "r1.p1".into()),
            ("u1.a".into(), "r1.p2".into())
        ]
    );
    // …while two spellings of one pin are the same wire.
    let e = err(&sheet(
        &(part("u1", 1) + "|R#r1|\nu1 - r1.p1\nu1.a - r1.p1\n"),
    ));
    assert_eq!(
        e.message,
        "'u1.a - r1.p1' is already wired — a repeated wire means nothing on a sheet"
    );
}

#[test]
fn a_repeat_inside_one_chain_is_caught_too() {
    // Desugar states a chain as a link per hop, so its hops are judged
    // like any other pair. (`u1.b` is one landing here: only a *two*-pin
    // part passes through to its other pin.)
    let e = err(&sheet(&(part("u1", 3) + "|gnd#g1|\ng1 - u1.b - g1\n")));
    assert_eq!(
        e.message,
        "'u1.b - g1' is already wired — a repeated wire means nothing on a sheet"
    );
}

#[test]
fn the_laws_reach_a_nested_container_and_stop_at_another_engine() {
    // [SPEC 16] a `|row|` reads no statement of its own, so the sheet's
    // laws reach right through it — arity resolves, duplicates error.
    let nested = "|schematic#s| [\n  |row#r| [\n".to_string()
        + &part("u1", 1)
        + "|R#r1|\n  u1 - r1\n  ]\n]\n";
    assert_eq!(hops(&nested), vec![("s.r.u1.a".into(), "s.r.r1.p1".into())]);
    let e = err(&("|schematic#s| [\n  |row#r| [\n".to_string()
        + &part("u1", 3)
        + "|gnd#g1|\n  u1.a - g1\n  g1 - u1.a\n  ]\n]\n"));
    assert_eq!(
        e.message,
        "'g1 - u1.a' is already wired — a repeated wire means nothing on a sheet"
    );
    // A nested engine that reads its own body's statements stops them: a
    // sequence's participants are not landings, and its `x -> y` twice is
    // its own business.
    let sealed = "|schematic#s| [\n  |sequence#q| [\n".to_string()
        + &part("u1", 2)
        + "  x -> u1 \"call\"\n  x -> u1 \"again\"\n  ]\n]\n";
    assert_eq!(
        hops(&sealed),
        vec![
            ("s.q.x".into(), "s.q.u1".into()),
            ("s.q.x".into(), "s.q.u1".into())
        ]
    );
}

#[test]
fn a_fan_into_a_part_is_one_landing() {
    // `&` shares one end [SPEC 9], and the router gives that end one port
    // — so both legs land on the *same* pin, not on p1 and p2. The two
    // pins of the far part stay two landings all the same: the reading is
    // keyed on the endpoint as written, not on the part.
    let src = sheet(&(part("u1", 3) + "|R#r1|\nu1.a & u1.b - r1\n"));
    assert_eq!(
        hops(&src),
        vec![
            ("u1.a".into(), "r1.p1".into()),
            ("u1.b".into(), "r1.p1".into())
        ]
    );
    // …and a fan *out of* a part is one landing too.
    let out = sheet(&(part("u1", 3) + "|R#r1|\nr1 - u1.a & u1.b\n"));
    assert_eq!(
        hops(&out),
        vec![
            ("r1.p1".into(), "u1.a".into()),
            ("r1.p1".into(), "u1.b".into())
        ]
    );
}

#[test]
fn a_self_loop_is_not_a_pass_through() {
    // Nothing threads a part to reach itself: `r1.p1 - r1.p1` stays the
    // one-side loop the router reports, not a short across the part. The
    // pinless form still spends its arity like any two landings.
    assert_eq!(
        hops(&sheet("|R#r1|\nr1.p1 - r1.p1\n")),
        vec![("r1.p1".into(), "r1.p1".into())]
    );
    assert_eq!(
        hops(&sheet("|R#r1|\nr1 - r1\n")),
        vec![("r1.p1".into(), "r1.p2".into())]
    );
}

#[test]
fn a_landing_reaching_into_another_scope_is_this_stages_own() {
    // Desugar resolves what a scope can answer for itself; a part inside a
    // nested container is not that — the sheet has no pins to read there,
    // so the lowered form still says `r.r1` and **this** pass lands it.
    let src = "{ layout: schematic }\n".to_string()
        + &part("u1", 3)
        + "|row#r| [ |R#r1| ]\nu1.a - r.r1\nu1.b - r.r1\n";
    let lowered = crate::desugar_source(&src).expect("desugar");
    assert!(
        lowered.contains("u1.a - r.r1\n"),
        "desugar leaves the deep path alone: {lowered}"
    );
    // …and lands it by the same law: next free, in pin order.
    assert_eq!(
        hops(&src),
        vec![
            ("u1.a".into(), "r.r1.p1".into()),
            ("u1.b".into(), "r.r1.p2".into())
        ]
    );
    // The law's errors reach it too, with the path spelled as written.
    let e = err(&("{ layout: schematic }\n".to_string()
        + &part("u1", 3)
        + "|row#r| [ |component#u9| [ |pin#x|; |pin#y|; |pin#z| ] ]\nu1.a - r.u9\n"));
    assert_eq!(e.message, "'r.u9' has 3 pins — name one ('r.u9.x')");
    assert_eq!(e.code, Code::SCHEMATIC_ARITY);
}

#[test]
fn a_part_inside_an_anonymous_container_is_bookkept_once() {
    // A gather lands the parts it **declares**; an anonymous container runs
    // its own, so its parts defer to resolve exactly like a named
    // container's do [SPEC 9]. Two tables spending one part's pins would
    // short two nets onto one pin — this is that regression's test.
    let sheet = |wires: &str| {
        format!(
            "{{ layout: schematic }}\n|group| [\n  |R#r1|\n  |gnd#gi|\n  r1 - gi\n]\n\
             |gnd#go|\n|gnd#go2|\n{wires}\n"
        )
    };
    assert_eq!(
        hops(&sheet("go - r1")),
        // Root statements resolve before a container's lifted ones.
        vec![("go".into(), "r1.p2".into()), ("r1.p1".into(), "gi".into())],
        "one net per pin, not two on p1"
    );
    // …and with both pins spent, the law is still there to say so.
    let e = err(&sheet("go - r1\ngo2 - r1"));
    assert_eq!(
        e.message,
        "both pins of 'r1' are wired — name one ('r1.p1')"
    );
    assert_eq!(e.code, Code::SCHEMATIC_ARITY);
    // The control: a **named** container behaves the same way, as it did
    // before — the anonymous case simply joined it.
    let named = |wires: &str| {
        format!(
            "{{ layout: schematic }}\n|group#g| [\n  |R#r1|\n  |gnd#gi|\n  r1 - gi\n]\n\
             |gnd#go|\n|gnd#go2|\n{wires}\n"
        )
    };
    assert_eq!(
        hops(&named("go - g.r1")),
        vec![
            ("go".into(), "g.r1.p2".into()),
            ("g.r1.p1".into(), "g.gi".into())
        ]
    );
    assert_eq!(
        err(&named("go - g.r1\ngo2 - g.r1")).message,
        "both pins of 'g.r1' are wired — name one ('g.r1.p1')"
    );
}

#[test]
fn a_chain_threads_a_part_this_stage_had_to_land_itself() {
    // [SPEC 16.5] The pass-through is a reading of the **chain**, so a
    // chain desugar could not resolve reaches here whole — and is read the
    // same way: entry pin in, the other pin out. Both container shapes,
    // both spellings; each also travels through its own lowered form in
    // `tests/desugar.rs::a_resolved_landing_means_the_same_program_after_lowering`.
    let sheet = |container: &str, held: &str, wire: &str| {
        format!(
            "{{ layout: schematic }}\n{}|{container}| [\n  {held}\n]\n|gnd#g1|\n{wire}\n",
            part("u1", 3)
        )
    };
    // pinless, through an anonymous container…
    assert_eq!(
        hops(&sheet("group", "|R#r1|", "u1.a - r1 - g1")),
        vec![
            ("u1.a".into(), "r1.p1".into()),
            ("r1.p2".into(), "g1".into())
        ]
    );
    // …and a named one.
    assert_eq!(
        hops(&sheet("group#gp", "|R#r1|", "u1.a - gp.r1 - g1")),
        vec![
            ("u1.a".into(), "gp.r1.p1".into()),
            ("gp.r1.p2".into(), "g1".into())
        ]
    );
    // The named-pin form enters the cathode and leaves by the anode.
    assert_eq!(
        hops(&sheet("group", "|D#d1|", "u1.a - d1.k - g1")),
        vec![("u1.a".into(), "d1.k".into()), ("d1.a".into(), "g1".into())]
    );
    assert_eq!(
        hops(&sheet("group#gp", "|D#d1|", "u1.a - gp.d1.k - g1")),
        vec![
            ("u1.a".into(), "gp.d1.k".into()),
            ("gp.d1.a".into(), "g1".into())
        ]
    );
    // Threading spends both pins, so a further landing has nowhere to go.
    let e = err(&sheet("group", "|R#r1|", "u1.a - r1 - g1\nu1.b - r1"));
    assert_eq!(
        e.message,
        "both pins of 'r1' are wired — name one ('r1.p1')"
    );
    assert_eq!(e.code, Code::SCHEMATIC_ARITY);
    // …while an `&` fan on a deferred part is still **one** landing: it
    // shares an end, it does not thread the part.
    assert_eq!(
        hops(&sheet("group", "|R#r1|", "u1.a & u1.b - r1")),
        vec![
            ("u1.a".into(), "r1.p1".into()),
            ("u1.b".into(), "r1.p1".into())
        ]
    );
}

#[test]
fn a_fan_in_a_chain_states_each_written_hop_once() {
    // [SPEC 9] `&` shares an **end**, so `a & b - x - c` is `a - x`,
    // `b - x`, `x - c` — three wires. Expanding the fan around the whole
    // chain reads every hop away from the fanned group once per leg; the
    // cut states each written hop once, and the second reading is not a
    // second wire for the duplicate law to judge.
    assert_eq!(
        hops("{ layout: schematic }\n|box#a|\n|box#b|\n|box#x|\n|box#c|\na & b - x - c\n"),
        vec![
            ("a".into(), "x".into()),
            ("x".into(), "c".into()),
            ("b".into(), "x".into())
        ]
    );
    // …and the fan's legs share the landing they share the end with,
    // while the part the chain threads still spends both its pins.
    let deferred = |wire: &str| {
        format!(
            "{{ layout: schematic }}\n{}|group| [ |R#r1| ]\n|gnd#g1|\n{wire}\n",
            part("u1", 3)
        )
    };
    assert_eq!(
        hops(&deferred("u1.a & u1.b - r1 - g1")),
        vec![
            ("u1.a".into(), "r1.p1".into()),
            ("r1.p2".into(), "g1".into()),
            ("u1.b".into(), "r1.p1".into())
        ]
    );
    // A fan at the far end of the thread reads the same way.
    assert_eq!(
        hops(&deferred("u1.a - r1 - u1.b & u1.c")),
        vec![
            ("u1.a".into(), "r1.p1".into()),
            ("r1.p2".into(), "u1.b".into()),
            ("r1.p2".into(), "u1.c".into())
        ]
    );
    // …and one at both ends fans each of the two hops, no more.
    assert_eq!(
        hops(&deferred("u1.a & u1.b - r1 - g1 & u1.c")),
        vec![
            ("u1.a".into(), "r1.p1".into()),
            ("r1.p2".into(), "g1".into()),
            ("r1.p2".into(), "u1.c".into()),
            ("u1.b".into(), "r1.p1".into())
        ]
    );
    // A fan in the **middle** is two threads, not one shared hop: each
    // written endpoint is its own landing.
    let two = "{ layout: schematic }\n".to_string()
        + &part("u1", 3)
        + "|group| [ |R#r1|; |R#r2| ]\n|gnd#g1|\nu1.a - r1 & r2 - g1\n";
    assert_eq!(
        hops(&two),
        vec![
            ("u1.a".into(), "r1.p1".into()),
            ("r1.p2".into(), "g1".into()),
            ("u1.a".into(), "r2.p1".into()),
            ("r2.p2".into(), "g1".into())
        ]
    );
    // The reading of "one written hop" is per **circuit**, not per source
    // line: a define body's statement is lifted once per host instance,
    // and every lift carries the authored spans, so two hosts are two
    // chains — four wires, not two.
    let lifted = "{\n  layout: schematic;\n  |blk::group| { layout: schematic; } [\n    \
                  |box#a|\n    |box#b|\n    |box#x|\n    a - b - x\n  ]\n}\n\
                  |blk#c1|\n|blk#c2|\n";
    assert_eq!(
        hops(lifted),
        vec![
            ("c1.a".into(), "c1.b".into()),
            ("c1.b".into(), "c1.x".into()),
            ("c2.a".into(), "c2.b".into()),
            ("c2.b".into(), "c2.x".into())
        ]
    );
    // A repeat is still a repeat: two statements naming one pair error,
    // fan or no fan.
    let e = err(&(sheet(&(part("u1", 3) + "|box#x|\n")) + "u1.a & u1.b - x\nu1.a - x\n"));
    assert_eq!(
        e.message,
        "'u1.a - x' is already wired — a repeated wire means nothing on a sheet"
    );
    assert_eq!(e.code, Code::DUPLICATE_WIRE);
}

#[test]
fn no_wire_leaves_resolve_with_more_than_two_ends() {
    // The two carriers answer different questions and may disagree:
    // desugar's cascade slice sees the `|schematic|` type's own
    // `layout: schematic` and stands `split_chain` down, while the wider
    // resolved cascade sees the worn class override it. The cut is not
    // gated on either, so the chain still becomes a wire per hop — and a
    // statement's label still rides every one of them [SPEC 9].
    let src = "{ .plain { layout: flow } }\n|schematic#s| .plain [\n  |box#a|\n  \
               |box#b|\n  |box#c|\n  a - b - c \"L\"\n]\n";
    let lowered = crate::desugar_source(src).expect("desugar");
    assert!(
        lowered.contains("a - b - c"),
        "desugar's carrier says schematic, so it left the chain whole: {lowered}"
    );
    let toks = crate::lexer::lex(src).expect("lex");
    let file = crate::syntax::parser::parse(src, &toks).expect("parse");
    let program =
        crate::resolve::resolve_with_theme(&crate::desugar::desugar(&file).expect("desugar"), &[])
            .expect("resolve");
    let ends: Vec<usize> = program.links.iter().map(|w| w.endpoints.len()).collect();
    assert_eq!(ends, vec![2, 2], "one two-ended wire per hop");
    assert_eq!(
        hops(src),
        vec![("s.a".into(), "s.b".into()), ("s.b".into(), "s.c".into())]
    );
    assert!(
        program.links.iter().all(|w| w.texts.len() == 1),
        "the label rides every hop, not one hop each"
    );
    // The one many-ended wire the law keeps: a **one-ended leader's** `&`
    // fan is one link carrying every endpoint — one note, an independent
    // leg per feature [SPEC 15.7]. It is not a chain, so it is not cut.
    let leader = "|drawing#v| [\n  |rect#p| { width: 150; height: 70 } [\n    \
                  |hole#b| { width: 10; pattern: grid(2, 2, 100, 30) }\n  ]\n  \
                  p.b.1 & p.b.2 & p.b.4 <- \"3× CSK 90°\"\n  \
                  p:left (-) p.b (-) p:right\n]\n";
    let toks = crate::lexer::lex(leader).expect("lex");
    let file = crate::syntax::parser::parse(leader, &toks).expect("parse");
    let program =
        crate::resolve::resolve_with_theme(&crate::desugar::desugar(&file).expect("desugar"), &[])
            .expect("resolve");
    let leaders: Vec<usize> = program
        .links
        .iter()
        .filter(|w| w.one_ended)
        .map(|w| w.endpoints.len())
        .collect();
    assert_eq!(leaders, vec![3], "one link, three legs");
    // …and a dimension chain is one measured row, whose hops belong to the
    // drawing engine [SPEC 15.6] — not this cut's.
    let dims: Vec<usize> = program
        .links
        .iter()
        .filter(|w| matches!(w.kind, LinkKind::Measure(_)))
        .map(|w| w.endpoints.len())
        .collect();
    assert_eq!(dims, vec![3], "one dimension chain, three anchors");
}

#[test]
fn a_wire_written_outside_a_sheet_reads_the_sheets_laws_at_its_sheet_end() {
    // The regression: the laws asked the *wire's* scope while the router lands
    // an endpoint by the *endpoint's* (`request::fixed`), so a pair written at
    // a plain root both arrived at `s.r1`'s bare port — two nets shorted onto
    // one pin, no reservation, no diagnostic. Being a part **is** being in the
    // scope, so both spellings of one circuit now land identically: the wires
    // written at the plain root, and the same wires in the sheet's own body.
    let outside = |wires: &str| {
        format!(
            "|schematic#s| [\n  |component#u1| [ |pin#a|; |pin#b|; |pin#c| ]\n  |R#r1|\n]\n{wires}"
        )
    };
    let inside = |wires: &str| {
        format!(
            "|schematic#s| [\n  |component#u1| [ |pin#a|; |pin#b|; |pin#c| ]\n  |R#r1|\n{wires}]\n"
        )
    };
    let expected = vec![
        ("s.u1.a".to_string(), "s.r1.p1".to_string()),
        ("s.u1.b".to_string(), "s.r1.p2".to_string()),
    ];
    assert_eq!(
        hops(&outside("s.u1.a - s.r1\ns.u1.b - s.r1\n")),
        expected,
        "written outside the sheet"
    );
    assert_eq!(
        hops(&inside("  u1.a - r1\n  u1.b - r1\n")),
        expected,
        "…and inside it"
    );
    // The arity law's refusal crosses the boundary with them (R021).
    let e = err(&outside("s.u1.a - s.r1\ns.u1.b - s.r1\ns.u1.c - s.r1\n"));
    assert_eq!(
        e.message,
        "both pins of 's.r1' are wired — name one ('s.r1.p1')"
    );
    assert_eq!(e.code, Code::SCHEMATIC_ARITY);
    // …and so does the duplicate law, over the resolved pair, unordered.
    let e = err(&outside("s.u1.a - s.r1.p1\ns.r1.p1 - s.u1.a\n"));
    assert_eq!(
        e.message,
        "'s.r1.p1 - s.u1.a' is already wired — a repeated wire means nothing on a sheet"
    );
    assert_eq!(e.code, Code::DUPLICATE_WIRE);
    // A pair only *one* of whose statements crosses the boundary is a repeat
    // too: the sheet's reservations are one table for the whole program.
    let e = err(
        "|schematic#s| [\n  |component#u1| [ |pin#a| ]\n  |R#r1|\n  u1.a - r1\n]\n\
         s.u1.a - s.r1.p1\n",
    );
    assert_eq!(e.code, Code::DUPLICATE_WIRE);
}

#[test]
fn a_sealed_engine_inside_a_sheet_still_owns_its_own_statements() {
    // The half the statement's own scope still answers [SPEC 12–15]: a nested
    // `|drawing|` reads its body's links itself, so a leader stays a leader —
    // one-ended, un-landed — even though the part it points at is the
    // family's and the sheet encloses it.
    let src = "{ layout: schematic }\n|drawing#d| [\n  |R#r1|\n  |R#r2|\n  \
               |rect#p| { width: 40; height: 20 }\n  r1 - r2\n  p <- \"a note\"\n]\n";
    // The parts are the family's and the sheet encloses them, but the wire is
    // the drawing's statement: no landing, so the paths stay bare.
    assert_eq!(hops(src), vec![("d.r1".into(), "d.r2".into())]);
    let toks = crate::lexer::lex(src).expect("lex");
    let file = crate::syntax::parser::parse(src, &toks).expect("parse");
    let program =
        crate::resolve::resolve_with_theme(&crate::desugar::desugar(&file).expect("desugar"), &[])
            .expect("the drawing's own statements are not the sheet's");
    assert_eq!(
        program.links.iter().filter(|w| w.one_ended).count(),
        1,
        "the leader stayed a leader"
    );
}

#[test]
fn a_wire_outside_every_sheet_is_untouched() {
    // No carrier, no laws: an ordinary document still repeats a link (the
    // parallel-rails contract) and still lands on boxes.
    let plain = "|box#a|\n|box#b|\na - b\nb - a\n";
    assert_eq!(
        hops(plain),
        vec![("a".into(), "b".into()), ("b".into(), "a".into())]
    );
}

#[test]
fn the_landings_are_deterministic() {
    let src =
        sheet(&(part("u1", 2) + "|R#r1|\n|C#c1|\n|gnd#g1|\nu1.a - r1 - g1\nu1.b - c1\nc1 - g1\n"));
    let once = hops(&src);
    assert_eq!(once.len(), 4, "one hop per wire: {once:?}");
    for _ in 0..3 {
        assert_eq!(hops(&src), once, "the same sheet lands identically");
    }
}
