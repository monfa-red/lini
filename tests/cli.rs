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
    // Unlayered, and headed by the figure's own scope class [SPEC 18].
    assert!(svg.contains(".lini-scope-"), "{}", svg);
    assert!(svg.contains(" .lini-box {"), "{}", svg);
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
fn a_builtin_theme_round_trips_through_its_own_printed_css() {
    // `lini theme blueprint > t.css` then `--theme t.css` must land the same
    // palette: the printed file is the carrier the web path uses (a host page
    // links exactly this CSS), so it has to be a faithful copy of the built-in.
    let css = lini::builtin_css("blueprint").expect("built-in");
    let src = "{ fill: --bg; }\n|box| \"x\"\n";
    let direct = lini::compile_str_with(
        src,
        &Options {
            theme_css: Some(css.clone()),
            static_mode: true,
            ..Default::default()
        },
    )
    .expect("compile");
    assert!(direct.contains("#00509e"), "the paper bakes in: {direct}");
    assert!(!direct.contains("var("), "{direct}");
}

#[test]
fn a_themed_live_compile_stays_overridable_by_host_css() {
    // The web/WASM path [SPEC 10.6/18]: without `--static` the theme lands as
    // live `--lini-*` declarations inside `@layer lini.defaults`, and the rules
    // keep their `var()`s — so unlayered host CSS (`:root, .lini { … }`, what
    // `lini theme NAME` prints) re-themes the same SVG in the browser.
    let svg = lini::compile_str_with(
        "|box| \"x\"\n",
        &Options {
            theme_css: Some(lini::builtin_css("blueprint").expect("built-in")),
            ..Default::default()
        },
    )
    .expect("compile");
    assert!(svg.contains("@layer lini.defaults"), "{svg}");
    assert!(svg.contains("--lini-fill: #2f6199;"), "{svg}");
    assert!(svg.contains("fill: var(--lini-fill)"), "{svg}");
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

#[test]
fn every_compile_surface_rejects_validation_errors() {
    let src = "|box#a| { colr: red; }\n";
    assert!(
        lini::lint_str(src)
            .expect("lint")
            .iter()
            .any(|diag| diag.level == lini::Level::Error)
    );
    for err in [
        lini::compile_str(src).expect_err("compile must validate"),
        lini::check(src).expect_err("check must validate"),
    ] {
        assert!(err.message.contains("unknown property 'colr'"), "{err}");
    }
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
    // lini-g1500: the proportional face (1) at the medium default weight
    // (`--lini-font-weight: 500`), and the outlines follow. The `lini-` prefix
    // is the generated-id reservation [SPEC 18] — glyph defs are no exception.
    assert!(
        svg.contains("<use href=\"#lini-g1500-") && svg.contains("<path id=\"lini-g1500-"),
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

    // The cheap check path applies the same acceptance gate.
    let (code, err) = run(&["--check", bad.to_str().unwrap()]);
    assert_eq!(
        code,
        Some(1),
        "--check must reject validation errors: {err}"
    );
    assert!(err.contains("unknown property 'colr'"), "{err}");

    std::fs::remove_dir_all(&dir).ok();
}

// ── `lini highlight`: the build-time door onto the one scanner ──

/// The subcommand is a wrapper and must stay one: what it prints is what
/// `lini::highlight_html` returns, byte for byte, from a file and from stdin
/// alike — the same guarantee `tests/wasm.rs` gives the browser export
/// [SPEC 20 / 22].
#[test]
fn highlight_prints_exactly_what_the_library_returns() {
    let bin = env!("CARGO_BIN_EXE_lini");
    let source = "{ layout: sequence; --brand: #ff6600; }\n\n\
                  |box#a| \"A <&> B\" .hot { fill: --teal-wash; }\n\
                  // a comment\n\
                  a -> b \"then\"\n";
    let want = lini::highlight_html(source);

    let dir = std::env::temp_dir().join(format!("lini-highlight-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let file = dir.join("h.lini");
    std::fs::write(&file, source).unwrap();

    let from_file = Command::new(bin)
        .args(["highlight", file.to_str().unwrap()])
        .output()
        .expect("spawn lini");
    assert_eq!(from_file.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&from_file.stdout), want);

    let mut child = Command::new(bin)
        .args(["highlight", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn lini");
    std::io::Write::write_all(child.stdin.as_mut().unwrap(), source.as_bytes()).unwrap();
    let from_stdin = child.wait_with_output().expect("wait for lini");
    assert_eq!(from_stdin.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&from_stdin.stdout), want);

    std::fs::remove_dir_all(&dir).ok();
}

/// `--css` prints the palette those spans wear, from the same place — so a
/// host never hand-copies thirteen rules to colour what the markup names.
#[test]
fn highlight_css_prints_the_palette_the_markup_wears() {
    let out = Command::new(env!("CARGO_BIN_EXE_lini"))
        .args(["highlight", "--css"])
        .output()
        .expect("spawn lini");
    assert_eq!(out.status.code(), Some(0));
    let css = String::from_utf8_lossy(&out.stdout);
    assert_eq!(css, lini::highlight_css());
    for class in ["comment", "string", "type", "prop", "punct"] {
        assert!(css.contains(&format!(".lini-tok-{class}")), "{css}");
    }

    // Neither an input nor --css is a usage error, not a silent empty file.
    let bare = Command::new(env!("CARGO_BIN_EXE_lini"))
        .arg("highlight")
        .output()
        .expect("spawn lini");
    assert_eq!(bare.status.code(), Some(3));
    assert!(bare.stdout.is_empty());
}

/// Highlighting is lexical, so a file the compiler rejects still lists — which
/// is the whole point for an editor and for a docs page showing a mistake.
/// Only I/O fails.
#[test]
fn highlight_colours_a_file_that_does_not_compile() {
    let bin = env!("CARGO_BIN_EXE_lini");
    let out = Command::new(bin)
        .args(["highlight", "/nonexistent.lini"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .expect("spawn lini");
    assert_eq!(out.status.code(), Some(2), "a missing file is an I/O error");

    let mut child = Command::new(bin)
        .args(["highlight", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn lini");
    std::io::Write::write_all(child.stdin.as_mut().unwrap(), b"|box#a| { colr: red; }\n").unwrap();
    let out = child.wait_with_output().expect("wait for lini");
    assert_eq!(out.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&out.stdout)
            .contains("<span class=\"lini-tok-prop-user\">colr</span>"),
        "an unknown property still colours, weakly"
    );
}
