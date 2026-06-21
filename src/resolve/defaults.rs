//! Built-in visual variables — the live `--lini-*` palette (colours, fonts, the
//! shadow tint) that themes Lini's look (SPEC §11.1), the lowest specificity layer.
//! Theme and `--name` application live in [`super::program`], value resolution in
//! [`super::value`]. Layout constants are no longer here: desugar materializes every
//! one into the `.lini-*` classes and the global block. This module is the data.

use super::ir::{ResolvedCall, ResolvedValue, VarKind, VarTable};

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

    // Visual vars — live at runtime (SPEC §11.1).
    set_visual(&mut t, "bg", ident("white"));
    set_visual(&mut t, "fg", ident("black"));
    set_visual(&mut t, "fill", ident("white"));
    set_visual(&mut t, "stroke", hex("444"));
    set_visual(&mut t, "accent", hex("0a84ff"));
    set_visual(&mut t, "on-accent", ident("white"));
    set_visual(&mut t, "muted", hex("888"));
    set_visual(&mut t, "danger", ident("crimson"));
    set_visual(&mut t, "warn", ident("orange"));
    set_visual(&mut t, "airwire", ident("crimson"));
    set_visual(&mut t, "note-bg", hex("fff9c4"));
    set_visual(&mut t, "group-stroke", rgba(0.0, 0.0, 0.0, 0.4));
    set_visual(&mut t, "group-fill", rgba(0.0, 0.0, 0.0, 0.03));
    set_visual(&mut t, "caption-color", rgba(0.0, 0.0, 0.0, 0.5));
    set_visual(&mut t, "footer-color", rgba(0.0, 0.0, 0.0, 0.5));
    set_visual(
        &mut t,
        "font-family",
        ResolvedValue::RawCss(
            "ui-monospace, \"SF Mono\", \"Cascadia Code\", \"JetBrains Mono\", Menlo, Consolas, \"Liberation Mono\", monospace"
                .into(),
        ),
    );
    set_visual(&mut t, "font-weight", ident("bold"));
    set_visual(&mut t, "caption-font-weight", ident("normal"));
    set_visual(&mut t, "wire-font-weight", ident("normal"));
    set_visual(
        &mut t,
        "text-color",
        ResolvedValue::LiveVar {
            name: "fg".into(),
            raw: false,
            baked: None,
        },
    );
    set_visual(&mut t, "shadow-color", rgba(0.0, 0.0, 0.0, 0.2));

    // Layout constants (radius, padding, font-size, clearance, …) are no longer
    // here: desugar materializes every one into the `.lini-*` class defs, the
    // global block, or the `-> { }` wire defaults (the "dumb core").

    t
}

fn set_visual(t: &mut VarTable, name: &str, v: ResolvedValue) {
    t.set(name, VarKind::Visual, v);
}
