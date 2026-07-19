//! Stage 2 drift guard [BETA-tooling]. The schema and its compact reference are
//! generated from the property ledger and committed in `schema/`; here we
//! regenerate them in memory and assert byte-equality, so a stale checkout
//! fails CI and never ships. The per-property examples the schema embeds are
//! compiled end-to-end — an example can never rot into invalid syntax.

use std::path::{Path, PathBuf};

fn schema_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("schema")
}

#[test]
fn schema_json_matches_committed_byte_for_byte() {
    let committed = std::fs::read_to_string(schema_dir().join("lini.schema.json"))
        .expect("schema/lini.schema.json — run `cargo xtask gen-schema`");
    assert_eq!(
        committed,
        lini::schema_json(),
        "schema drift — regenerate with `cargo xtask gen-schema` and commit"
    );
}

#[test]
fn reference_md_matches_committed_byte_for_byte() {
    let committed = std::fs::read_to_string(schema_dir().join("reference.md"))
        .expect("schema/reference.md — run `cargo xtask gen-schema`");
    assert_eq!(
        committed,
        lini::reference_md(),
        "reference drift — regenerate with `cargo xtask gen-schema` and commit"
    );
}

#[test]
fn every_property_example_compiles() {
    // Two examples reference a sample asset (`assets/logo.svg`); resolve it
    // against the samples dir, exactly as the conformance suite does [SPEC 7].
    let opts = lini::Options {
        base_dir: Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("samples")),
        ..Default::default()
    };
    for &(name, src) in lini::schema_examples {
        lini::compile_str_with(src, &opts)
            .unwrap_or_else(|e| panic!("the '{name}' example must compile: {}", e.message));
    }
}
