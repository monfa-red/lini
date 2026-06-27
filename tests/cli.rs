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
            bake_vars: true,
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
            bake_vars: true,
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
            bake_vars: true,
            ..Default::default()
        },
    )
    .expect("compile");
    assert!(svg.contains("fill: hotpink"), "{}", svg);
}

#[test]
fn theme_cannot_set_a_layout_value() {
    // Layout values (gap, padding, radius, …) bake from the global block and the
    // `.lini-*` classes, not `--lini-*` vars (SPEC §11.2, the "dumb core"): a
    // `--lini-gap` theme is inert. Gap is set with `gap:` in the stylesheet.
    let src = "{\n  layout: row;\n}\n|box| { width: 40; height: 40; }\n|box| { width: 40; height: 40; }\n";
    let default = lini::compile_str(src).expect("default compile");
    let themed = lini::compile_str_with(
        src,
        &Options {
            theme_css: Some("--lini-gap: 60;".to_string()),
            ..Default::default()
        },
    )
    .expect("themed compile");
    assert_eq!(
        extract_viewbox_w(&default),
        extract_viewbox_w(&themed),
        "a --lini-gap theme must not change layout — gap is not a themeable var",
    );
}

#[test]
fn theme_visual_var_does_not_change_layout_baking() {
    let src = "{\n  layout: row;\n}\n|box| { width: 40; height: 40; }\n|box| { width: 40; height: 40; }\n";
    let default = lini::compile_str(src).expect("default compile");
    let themed = lini::compile_str_with(
        src,
        &Options {
            theme_css: Some("--lini-accent: red;".to_string()),
            ..Default::default()
        },
    )
    .expect("themed compile");
    assert_eq!(extract_viewbox_w(&default), extract_viewbox_w(&themed));
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
