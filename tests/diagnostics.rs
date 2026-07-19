//! Structured diagnostics [ROADMAP 3.8, BETA-tooling Stage 3]: the `--json`
//! document shape, and the machine-applicable-fix round trip.

use lini::Options;

fn json(src: &str) -> String {
    let (doc, _had_error) = lini::diagnostics_json(src, &Options::default(), "test.lini");
    doc
}

/// The JSON shape for a spread of families — an unknown property (with a
/// machine-applicable fix), a malformed value, and an off-grid placement.
/// Pins codes, severities, spans, and the suggestion.
#[test]
fn json_document_shape() {
    let src = "|box#a| { colr: red; opacity: 5; }\n|box#g| [ |box#b| { cell: 1; } ]\n";
    insta::assert_snapshot!(json(src));
}

/// A clean file emits an empty diagnostics array (not an error).
#[test]
fn json_clean_file_is_empty() {
    insta::assert_snapshot!(json("|box#a| \"hi\"\n"));
}

/// The machine-applicable contract [ROADMAP 3.8]: the JSON advertises a
/// verbatim replacement over a span; applying it to the source yields a file
/// that compiles clean.
#[test]
fn machine_applicable_fix_recompiles_clean() {
    let src = "|box#a| { colr: red; }\n";
    let doc = json(src);
    assert!(
        doc.contains("\"code\": \"V001\""),
        "unknown-property code present:\n{doc}"
    );
    assert!(
        doc.contains("\"applicability\": \"machine-applicable\""),
        "fix is machine-applicable:\n{doc}"
    );
    assert!(
        doc.contains("\"replacement\": \"color\""),
        "replacement is the nearest name:\n{doc}"
    );

    // Apply the advertised edit — the misspelled name spans exactly `colr`.
    let start = src.find("colr").unwrap();
    let mut fixed = src.to_string();
    fixed.replace_range(start..start + "colr".len(), "color");
    assert_eq!(fixed, "|box#a| { color: red; }\n");
    lini::compile_str(&fixed).expect("the applied fix recompiles clean");
}

/// Every diagnostic carries a phase-prefixed code — never the unclassified
/// `E` sentinel. The boundary stamps a phase onto anything untriaged.
#[test]
fn no_diagnostic_is_unclassified() {
    for src in [
        "|box#a| { colr: red; }\n", // validate
        "|box#a| { \n",             // parse
        "a -> b -> \n",             // parse/resolve
        "|box#a| |box#a|\n",        // resolve (duplicate id)
        "|line#l| { }\n",           // layout (missing points)
    ] {
        let doc = json(src);
        assert!(
            !doc.contains("\"code\": \"E"),
            "unclassified diagnostic for {src:?}:\n{doc}"
        );
    }
}
