use lini::testing::{read_sample, samples};

/// Every sweep sample must lex + parse without error.
/// Resolve / layout / render correctness is enforced by sprint-specific tests.
#[test]
fn all_samples_parse() {
    let mut failures = Vec::new();

    for path in samples() {
        if let Err(e) = lini::check_parse(&read_sample(&path)) {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            failures.push(format!("{}: {}", name, e));
        }
    }

    assert!(
        failures.is_empty(),
        "the following samples failed to parse:\n  {}",
        failures.join("\n  ")
    );
}

// ─────────────────────────── Invalid-input cases ───────────────────────────

#[track_caller]
fn assert_parse_error(src: &str, expect_msg_substr: &str) {
    let err = lini::check_parse(src).expect_err("expected parse error");
    let msg = err.to_string();
    assert!(
        msg.contains(expect_msg_substr),
        "expected error containing {:?}, got {:?}",
        expect_msg_substr,
        msg
    );
}

#[test]
fn err_link_chain_mixes_operators() {
    // Wire hops may differ (`cat -> dog --> bird` parses, each hop its own op
    // [SPEC 9]); mixing operator *kinds* — a wire with a measure — errors.
    assert_parse_error("cat -> dog (-) bird\n", "link chain mixes operators");
}

#[test]
fn err_unterminated_string() {
    assert_parse_error("|box#cat| \"oops\n", "unterminated string");
}

#[test]
fn err_bad_escape_sequence() {
    assert_parse_error("|box#cat| \"\\x\"\n", "invalid escape sequence");
}

#[test]
fn err_invalid_hex_color() {
    assert_parse_error("{ --c: #ff; }\n|box#cat|\n", "invalid hex color");
}

#[test]
fn err_link_body_holds_only_labels() {
    // A link's `{ }` holds only declarations (along:, stroke, …); a nested link
    // is not a declaration, so the block rejects it.
    assert_parse_error(
        "|box#a|\n|box#b|\na -> b { c -> d }\n",
        "style block holds only declarations",
    );
}

#[test]
fn err_text_carries_children() {
    // [SPEC 3/21] a string is a leaf: it wears classes and takes a style block,
    // but children need a box.
    assert_parse_error(
        "\"hello\" [ \"x\" ]\n",
        "text content takes no '[ ]' — wrap it in '|block|' to give it children",
    );
    assert_parse_error(
        "|box#b| [ \"hello\" [ \"x\" ] ]\n",
        "text content takes no '[ ]' — wrap it in '|block|' to give it children",
    );
    // The tail a text leaf *does* take.
    lini::check_parse("|box#b| [ \"hello\" .quiet { color: red } ]\n").expect("a styled leaf");
}

#[test]
fn err_spaced_call_paren() {
    // [SPEC 2/21] a call's '(' glues to its name — the rule that keeps
    // `move(-2, 5)`, `(8 * 2)`, and `pin (o)` apart.
    assert_parse_error(
        "|box#a| { fill: rgb (1, 2, 3) }\n",
        "a call's '(' glues to its name — write 'rgb(…)'",
    );
    assert_parse_error(
        "|box#a| { width: min (3, 4) }\n",
        "a call's '(' glues to its name — write 'rgb(…)'",
    );
    // A free-standing group is still a math group.
    lini::check_parse("|box#a| { width: (8 * 2) }\n").expect("a math group");
}

#[test]
fn lini_var_value_parses_anywhere() {
    // SPEC §11.2: `--name` is a first-class value form.
    lini::check_parse("{ --gap: --my-gap; }\n|box#cat|\n").expect("--gap parses");
    lini::check_parse("|box#cat| { fill: --accent; }\n").expect("--accent parses");
}

#[test]
fn endpoint_dotpath_navigates_into_groups() {
    lini::check_parse("|group#garden| [ |box#frog| ]\ngarden.frog -> outside\n")
        .expect("dot-path endpoint");
}

#[test]
fn endpoint_side_suffix_parses() {
    lini::check_parse("|box#cat|\n|box#dog|\ncat:right -> dog:left\n").expect("side suffix");
}

#[test]
fn fan_out_with_ampersand_parses() {
    lini::check_parse("cat -> dog & bird\n").expect("fan-out");
    lini::check_parse("fox & owl -> mouse\n").expect("fan-in");
    lini::check_parse("a & b -> c & d\n").expect("cartesian fan");
}

#[test]
fn capsule_endpoints_parse_in_every_position() {
    // [SPEC 9/22]: bars open an endpoint after an op, at statement head, in
    // fans, mid-chain, and glued to the op; a statement-head capsule with a
    // tail stays the node it always was.
    for src in [
        "cat -> |cyl#db|\n",
        "|cyl#db| -> cat\n",
        "a -> |box| -> c\n",
        "a & b -> |gnd|\n",
        "a - |gnd| - b\n",
        "a -|gnd|- b\n",
        "x - |component#U9|.p4\n",
        "|a| || |b|\n",
    ] {
        lini::check_parse(src).unwrap_or_else(|e| panic!("{src:?}: {e}"));
    }
}
