//! Built-in visual variables — the live `--lini-*` palette (colours, fonts, the
//! shadow tint) that themes Lini's look (SPEC §11.1), the lowest specificity layer.
//! Theme and `--name` application live in [`super::program`], value resolution in
//! [`super::value`]. Layout constants are no longer here: desugar materializes every
//! one into the `.lini-*` classes and the global block. This module is the data.

use super::ir::{ResolvedCall, ResolvedValue, VarTable};

/// The built-in visual `--lini-*` variables (SPEC §11.1), stored without the
/// `--lini-` prefix. They stay live at runtime; layout values are not vars.
pub fn built_in_defaults() -> VarTable {
    let mut t = VarTable::new();
    let ident = |s: &str| ResolvedValue::Ident(s.into());
    let hex = |s: &str| ResolvedValue::Hex(s.into());
    let rgba = |r: f64, g: f64, b: f64, a: f64| {
        ResolvedValue::Call(ResolvedCall {
            name: "rgba".into(),
            args: vec![
                ResolvedValue::Number(r),
                ResolvedValue::Number(g),
                ResolvedValue::Number(b),
                ResolvedValue::Number(a),
            ],
        })
    };
    // A `light-dark(LIGHT, DARK)` colour: the UA paints the arm matching the
    // element's `color-scheme`, so one SVG carries both palettes (SPEC §11.1).
    let light_dark = |l: ResolvedValue, d: ResolvedValue| {
        ResolvedValue::Call(ResolvedCall {
            name: "light-dark".into(),
            args: vec![l, d],
        })
    };

    // Visual vars — live at runtime, each colour a light-dark() pair (SPEC §11.1).
    set_visual(&mut t, "bg", light_dark(ident("white"), hex("1b1b1f")));
    set_visual(&mut t, "fg", light_dark(ident("black"), hex("e8e8ea")));
    set_visual(&mut t, "fill", light_dark(ident("white"), hex("26262b")));
    set_visual(&mut t, "stroke", light_dark(hex("444"), hex("9aa0a6")));
    set_visual(&mut t, "accent", light_dark(hex("0a84ff"), hex("4aa3ff")));
    set_visual(&mut t, "accent-text", ident("white"));
    set_visual(&mut t, "muted", light_dark(hex("888"), hex("9aa0a6")));
    set_visual(
        &mut t,
        "danger",
        light_dark(ident("crimson"), hex("ff6b6b")),
    );
    set_visual(&mut t, "warn", light_dark(ident("orange"), hex("ffb454")));
    set_visual(&mut t, "stray", light_dark(ident("crimson"), hex("ff6b6b")));
    set_visual(&mut t, "note-bg", light_dark(hex("fff9c4"), hex("4a4733")));
    set_visual(
        &mut t,
        "group-stroke",
        light_dark(rgba(0.0, 0.0, 0.0, 0.4), rgba(255.0, 255.0, 255.0, 0.4)),
    );
    set_visual(
        &mut t,
        "group-fill",
        light_dark(rgba(0.0, 0.0, 0.0, 0.03), rgba(255.0, 255.0, 255.0, 0.05)),
    );
    // A soft but visible grey so a duotone icon reads as two-tone by default
    // (heavier than the near-invisible group-fill).
    set_visual(
        &mut t,
        "icon-fill",
        light_dark(rgba(0.0, 0.0, 0.0, 0.16), rgba(255.0, 255.0, 255.0, 0.18)),
    );
    // A faint line for chart gridlines ([CHARTS.md] §5) — themeable and dark/light
    // aware like every role var, tree-shaken in only when a chart references it.
    set_visual(
        &mut t,
        "grid",
        light_dark(rgba(0.0, 0.0, 0.0, 0.10), rgba(255.0, 255.0, 255.0, 0.14)),
    );
    // The rich chart tooltip card ([CHARTS.md] §14): a solid contrasting surface and its
    // text — inverted from the page so the card pops in either theme.
    set_visual(&mut t, "tip-bg", light_dark(hex("333"), hex("e8e8ea")));
    set_visual(&mut t, "tip-fg", light_dark(ident("white"), hex("1a1a1f")));
    set_visual(
        &mut t,
        "caption-color",
        light_dark(rgba(0.0, 0.0, 0.0, 0.5), rgba(255.0, 255.0, 255.0, 0.55)),
    );
    set_visual(
        &mut t,
        "footer-color",
        light_dark(rgba(0.0, 0.0, 0.0, 0.5), rgba(255.0, 255.0, 255.0, 0.55)),
    );
    set_visual(
        &mut t,
        "font-family",
        ResolvedValue::RawCss(
            "ui-monospace, \"SF Mono\", \"Cascadia Code\", \"JetBrains Mono\", Menlo, Consolas, \"Liberation Mono\", monospace"
                .into(),
        ),
    );
    set_visual(&mut t, "font-weight", ident("normal"));
    set_visual(&mut t, "caption-font-weight", ident("normal"));
    set_visual(&mut t, "link-font-weight", ident("normal"));
    set_visual(
        &mut t,
        "text-color",
        ResolvedValue::LiveVar {
            name: "fg".into(),
            raw: false,
        },
    );
    set_visual(
        &mut t,
        "shadow-color",
        light_dark(rgba(0.0, 0.0, 0.0, 0.2), rgba(0.0, 0.0, 0.0, 0.5)),
    );

    // Layout constants (radius, padding, font-size, clearance, …) are no longer
    // here: desugar materializes every one into the `.lini-*` class defs, the
    // global block, or the cascaded link defaults (the "dumb core").

    // The named-hue palette (SPEC §11.2): 11 hues × 4 tiers + aliases, OKLCH-derived.
    // Tree-shaken at render (only referenced vars emit), so this never bloats output.
    for (name, value) in crate::palette::palette_vars() {
        t.set(name, value);
    }

    t
}

fn set_visual(t: &mut VarTable, name: &str, v: ResolvedValue) {
    t.set(name, v);
}
