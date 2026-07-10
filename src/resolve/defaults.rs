//! Built-in visual variables — the live `--lini-*` palette (colours, fonts, the
//! shadow tint) that themes Lini's look [SPEC 10.1], the lowest specificity layer.
//! Theme and `--name` application live in [`super::program`], value resolution in
//! [`super::value`]. Layout constants are no longer here: desugar materializes every
//! one into the `.lini-*` classes and the global block. This module is the data.

use super::ir::{ResolvedCall, ResolvedValue, VarTable};

/// The built-in visual `--lini-*` variables [SPEC 10.1], stored without the
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
    // element's `color-scheme`, so one SVG carries both palettes [SPEC 10.1].
    let light_dark = |l: ResolvedValue, d: ResolvedValue| {
        ResolvedValue::Call(ResolvedCall {
            name: "light-dark".into(),
            args: vec![l, d],
        })
    };

    // Visual vars — live at runtime, each colour a light-dark() pair [SPEC 10.1].
    t.set("bg", light_dark(ident("white"), hex("1b1b1f")));
    t.set("fg", light_dark(ident("black"), hex("e8e8ea")));
    t.set("fill", light_dark(ident("white"), hex("26262b")));
    t.set("stroke", light_dark(hex("444"), hex("9aa0a6")));
    // The primary drafting line tone [SPEC 10.1]: pen geometry, dimension
    // lines, and their arrowheads read full black on white (white on black
    // in dark) — the ISO print look; support lines stay a step lighter.
    t.set("stroke-dark", light_dark(ident("black"), ident("white")));
    // The secondary line tone [SPEC 10.1] — drafting's thin support lines
    // (centerlines, break lines, dimension extension lines) sit a step
    // lighter than the geometry. Full black/white at reduced **alpha**
    // (matching the old grey's value), so a support line crossing dark
    // geometry blends toward black instead of greying it.
    t.set(
        "stroke-light",
        light_dark(rgba(0.0, 0.0, 0.0, 0.545), rgba(255.0, 255.0, 255.0, 0.64)),
    );
    t.set("accent", light_dark(hex("0a84ff"), hex("4aa3ff")));
    t.set("accent-text", ident("white"));
    t.set("muted", light_dark(hex("888"), hex("9aa0a6")));
    t.set("danger", light_dark(ident("crimson"), hex("ff6b6b")));
    t.set("warn", light_dark(ident("orange"), hex("ffb454")));
    t.set("stray", light_dark(ident("crimson"), hex("ff6b6b")));
    t.set(
        "group-stroke",
        light_dark(rgba(0.0, 0.0, 0.0, 0.4), rgba(255.0, 255.0, 255.0, 0.4)),
    );
    t.set(
        "group-fill",
        light_dark(rgba(0.0, 0.0, 0.0, 0.03), rgba(255.0, 255.0, 255.0, 0.05)),
    );
    // The table / entity header band [SPEC 8]: a touch stronger than group-fill so a
    // header row reads as a band; dark/light aware, tree-shaken in only when referenced.
    t.set(
        "header-fill",
        light_dark(rgba(0.0, 0.0, 0.0, 0.06), rgba(255.0, 255.0, 255.0, 0.08)),
    );
    // A soft but visible grey so a duotone icon reads as two-tone by default
    // (heavier than the near-invisible group-fill).
    t.set(
        "icon-fill",
        light_dark(rgba(0.0, 0.0, 0.0, 0.16), rgba(255.0, 255.0, 255.0, 0.18)),
    );
    // A faint line for chart gridlines [SPEC 14.4] — themeable and dark/light
    // aware like every role var, tree-shaken in only when a chart references it.
    t.set(
        "grid",
        light_dark(rgba(0.0, 0.0, 0.0, 0.10), rgba(255.0, 255.0, 255.0, 0.14)),
    );
    // The rich chart tooltip card [SPEC 14.8]: a solid contrasting surface and its
    // text — inverted from the page so the card pops in either theme.
    t.set("tip-bg", light_dark(hex("333"), hex("e8e8ea")));
    t.set("tip-fg", light_dark(ident("white"), hex("1a1a1f")));
    t.set(
        "caption-color",
        light_dark(rgba(0.0, 0.0, 0.0, 0.5), rgba(255.0, 255.0, 255.0, 0.55)),
    );
    t.set(
        "footer-color",
        light_dark(rgba(0.0, 0.0, 0.0, 0.5), rgba(255.0, 255.0, 255.0, 0.55)),
    );
    t.set("font-family",
        ResolvedValue::RawCss(
            "ui-monospace, \"SF Mono\", \"Cascadia Code\", \"JetBrains Mono\", Menlo, Consolas, \"Liberation Mono\", monospace"
                .into(),
        ),
    );
    t.set("font-weight", ident("normal"));
    t.set("caption-font-weight", ident("normal"));
    t.set("link-font-weight", ident("normal"));
    t.set(
        "text-color",
        ResolvedValue::LiveVar {
            name: "fg".into(),
            raw: false,
        },
    );
    t.set(
        "shadow-color",
        light_dark(rgba(0.0, 0.0, 0.0, 0.2), rgba(0.0, 0.0, 0.0, 0.5)),
    );

    // Layout constants (radius, padding, font-size, clearance, …) are no longer
    // here: desugar materializes every one into the `.lini-*` class defs, the
    // global block, or the cascaded link defaults (the "dumb core").

    // The named-hue palette [SPEC 10.2]: 11 hues × 4 tiers + aliases, OKLCH-derived.
    // Tree-shaken at render (only referenced vars emit), so this never bloats output.
    for (name, value) in crate::palette::palette_vars() {
        t.set(name, value);
    }

    t
}
