//! Built-in defaults — the one place to tune Lini's look (SPEC §11). The lowest
//! specificity layer: the visual `--lini-*` variables (live at runtime) and the
//! baked layout constants (sizes, gaps, paddings, thicknesses). Theme and
//! `--name` application live in [`super::program`]; value resolution in
//! [`super::value`]. This module is the data.

use super::ir::{ResolvedCall, ResolvedValue, VarKind, VarTable};

/// The built-in defaults (SPEC §11). Visual `--lini-*` vars stay live; layout
/// constants bake. Names are stored without the `--lini-` prefix.
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

    // Layout constants — baked at compile time (SPEC §11.3).
    set_layout_n(&mut t, "font-size", 15.0);
    set_layout_n(&mut t, "wire-font-size", 11.0);
    set_layout_n(&mut t, "caption-font-size", 12.0);
    set_layout_n(&mut t, "stroke-width", 2.0);
    set_layout_n(&mut t, "radius", 6.0);
    set_layout_n(&mut t, "gap", 20.0);
    set_layout_n(&mut t, "padding", 20.0);
    set_layout_n(&mut t, "clearance", 16.0);
    set_layout_n(&mut t, "icon-size", 24.0);
    set_layout_n(&mut t, "canvas-pad", 20.0);

    t
}

fn set_layout_n(t: &mut VarTable, name: &str, n: f64) {
    t.set(name, VarKind::Layout, ResolvedValue::Number(n));
}

fn set_visual(t: &mut VarTable, name: &str, v: ResolvedValue) {
    t.set(name, VarKind::Visual, v);
}
