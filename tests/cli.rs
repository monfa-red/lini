//! End-to-end tests for the public Options surface — exercised through the
//! library API (which is what the CLI calls). The one exception spawns the
//! binary to assert an argument the library has no surface for is rejected.

use lini::{Options, OutputFormat};
use std::process::{Command, Stdio};

#[test]
fn html_format_wraps_svg_in_html_doc() {
    let html = lini::compile_str_with(
        "|box| \"x\"\n",
        &Options {
            format: OutputFormat::Html,
            static_mode: true,
            ..Default::default()
        },
    )
    .expect("compile");
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("<svg "));
    assert!(html.contains("</body>"));
    assert!(html.ends_with("</html>\n"));
}

#[test]
fn baked_output_inlines_every_var_but_keeps_shape_rules() {
    let svg = lini::compile_str_with(
        "|box| \"x\" { fill: --accent }\n",
        &Options {
            static_mode: true,
            ..Default::default()
        },
    )
    .expect("compile");
    assert!(
        !svg.contains("var("),
        "baked output must inline every var: {}",
        svg
    );
    assert!(
        svg.contains(".lini-box"),
        "baked output keeps the structural rules: {}",
        svg
    );
}

#[test]
fn default_output_has_layered_vars_and_unlayered_rules() {
    let svg = lini::compile_str("|box| \"x\"\n").expect("compile");
    assert!(svg.contains("@layer lini.defaults"), "{}", svg);
    assert!(svg.contains(".lini .lini-box"), "{}", svg);
}

#[test]
fn no_defaults_flag_is_an_unknown_argument() {
    // The flag is gone; clap rejects it as unknown (exit 3) before it ever
    // tries to read the input, distinguishing it from an I/O failure (exit 2).
    let status = Command::new(env!("CARGO_BIN_EXE_lini"))
        .args(["--no-defaults", "/nonexistent.lini"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn lini");
    assert_eq!(status.code(), Some(3));
}

#[test]
fn theme_overrides_visual_var_visible_in_baked_output() {
    let svg = lini::compile_str_with(
        "|box| \"x\" { fill: --accent }\n",
        &Options {
            theme_css: Some("--lini-accent: hotpink;".to_string()),
            static_mode: true,
            ..Default::default()
        },
    )
    .expect("compile");
    assert!(svg.contains("fill: hotpink"), "{}", svg);
}

#[test]
fn a_theme_never_changes_layout() {
    // Layout values (gap, padding, radius, …) bake from the global block and the
    // `.lini-*` classes, not `--lini-*` vars (SPEC §11.2, the "dumb core"): a
    // `--lini-gap` theme is inert — gap is set with `gap:` in the stylesheet —
    // and a themeable *visual* var (`--lini-accent`) never reaches measurement
    // either. Neither kind of var can move a box.
    let src = "{\n  direction: row;\n}\n|box| { width: 40; height: 40; }\n|box| { width: 40; height: 40; }\n";
    let default = lini::compile_str(src).expect("default compile");
    for var in ["--lini-gap: 60;", "--lini-accent: red;"] {
        let themed = lini::compile_str_with(
            src,
            &Options {
                theme_css: Some(var.to_string()),
                ..Default::default()
            },
        )
        .expect("themed compile");
        assert_eq!(
            extract_viewbox_w(&default),
            extract_viewbox_w(&themed),
            "a '{var}' theme must not change layout",
        );
    }
}

#[test]
fn check_with_succeeds_on_valid_input() {
    let opts = Options::default();
    assert!(lini::check_with("|box| \"x\"\n", &opts).is_ok());
}

#[test]
fn check_with_propagates_resolve_errors() {
    let opts = Options::default();
    let err = lini::check_with("|nosuch| \"x\"\n", &opts).expect_err("expected error");
    assert!(
        err.to_string().contains("unknown type 'nosuch'"),
        "got: {}",
        err
    );
}

fn extract_viewbox_w(svg: &str) -> f64 {
    let vb = svg
        .lines()
        .next()
        .unwrap()
        .split("viewBox=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    vb.split_whitespace().nth(2).unwrap().parse().unwrap()
}

#[cfg(feature = "font")]
#[test]
fn static_output_outlines_text_to_glyph_uses() {
    // `--static` [SPEC 18/20]: text leaves become `<use>` references to
    // deduped glyph paths in `<defs>` — no `<text>` element survives, so the
    // file renders identically with no font installed.
    let svg = lini::compile_str_with(
        "|box| \"hi\"\n",
        &Options {
            static_mode: true,
            ..Default::default()
        },
    )
    .expect("compile");
    assert!(!svg.contains("<text"), "no live text under --static: {svg}");
    // lg1500: the proportional face (1) at the medium default weight
    // (`--lini-font-weight: 500`), and the outlines follow.
    assert!(
        svg.contains("<use href=\"#lg1500-") && svg.contains("<path id=\"lg1500-"),
        "glyph defs + uses: {svg}"
    );
}

#[cfg(feature = "font")]
#[test]
fn embed_font_inlines_used_faces_under_scoped_names() {
    // `--embed-font` [SPEC 18]: a base64 @font-face per used face, under the
    // Lini-scoped family name, and the stack leads with that name so the
    // embedded bytes win over an installed copy.
    let svg = lini::compile_str_with(
        "|box| \"hi\"\n",
        &Options {
            embed_font: true,
            ..Default::default()
        },
    )
    .expect("compile");
    assert!(
        svg.contains("@font-face { font-family: \"Lini Sans\"; font-weight: 500;"),
        "{}",
        &svg[..800]
    );
    assert!(svg.contains("src: url(data:font/ttf;base64,"), "base64 src");
    assert!(
        svg.contains("--lini-font-family: \"Lini Sans\", \"Google Sans\","),
        "the stack leads with the scoped name"
    );
    // Text stays live `<text>` — embedding never outlines.
    assert!(svg.contains("<text"), "{svg}");
}

#[test]
fn bake_vars_flag_is_gone_without_alias() {
    // `--static` renames `--bake-vars` with no alias kept [SPEC 20] — clap
    // rejects the old spelling as unknown (exit 3).
    let status = Command::new(env!("CARGO_BIN_EXE_lini"))
        .args(["--bake-vars", "/nonexistent.lini"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn lini");
    assert_eq!(status.code(), Some(3));
}

// ── The CLI contract: errors always fail; --strict promotes warnings ──

#[test]
fn strict_turns_warnings_into_exit_1_and_no_warn_silences() {
    let bin = env!("CARGO_BIN_EXE_lini");
    // A directory of this test's own: a fixed path under the shared temp dir
    // races every other run on the machine (CI matrix, a second `cargo test`),
    // so name it for the process.
    let dir = std::env::temp_dir().join(format!("lini-strict-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let file = dir.join("warns.lini");
    std::fs::write(&file, "|box#cat| \"cat\"\ncta -> bird\n").unwrap();
    let run = |args: &[&str]| {
        let out = Command::new(bin).args(args).output().expect("spawn lini");
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    };
    let f = file.to_str().unwrap();

    // A warning alone: exit 0, message on stderr.
    let (code, err) = run(&[f, "-o", "/dev/null"]);
    assert_eq!(code, Some(0), "warnings don't fail a normal run: {err}");
    assert!(err.contains("did you mean 'cat'?"), "{err}");

    // --strict: the same warning is exit 1.
    let (code, err) = run(&["--strict", f, "-o", "/dev/null"]);
    assert_eq!(code, Some(1), "--strict promotes warnings: {err}");

    // --no-warn: silent, exit 0.
    let (code, err) = run(&["--no-warn", f, "-o", "/dev/null"]);
    assert_eq!(code, Some(0));
    assert!(err.is_empty(), "--no-warn silences warnings: {err}");

    // A validation error fails even under --no-warn.
    let bad = dir.join("bad.lini");
    std::fs::write(&bad, "|box#a| { colr: red; }\n").unwrap();
    let (code, err) = run(&["--no-warn", bad.to_str().unwrap(), "-o", "/dev/null"]);
    assert_eq!(code, Some(1), "validation errors always fail: {err}");
    assert!(err.contains("unknown property 'colr'"), "{err}");

    std::fs::remove_dir_all(&dir).ok();
}
