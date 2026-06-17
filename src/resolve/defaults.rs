//! Built-in defaults — the one place to tune Lini's look. These are the values
//! the lowest specificity layer provides: the visual `--lini-*` variables (live
//! at runtime) and the baked layout constants (sizes, gaps, paddings,
//! thicknesses). Theme application and variable resolution live in [`super::vars`];
//! this module is the data.
//!
//! Per-template attribute bundles and per-shape default sizes fold in here when
//! they are rewritten for v4 (see PLAN, phases 3–4).

use super::ir::{ResolvedCall, ResolvedValue, VarKind, VarTable};

/// Built-in CSS variable defaults per SPEC §12.1. Names are stored without the
/// `--lini-` prefix.
pub fn built_in_defaults() -> VarTable {
    let mut t = VarTable::new();

    // Visual vars — live at runtime.
    set_visual(&mut t, "bg", ResolvedValue::Ident("white".into()));
    set_visual(&mut t, "fg", ResolvedValue::Ident("black".into()));
    set_visual(&mut t, "fill", ResolvedValue::Ident("white".into()));
    set_visual(&mut t, "stroke", ResolvedValue::Hex("444".into()));
    set_visual(&mut t, "accent", ResolvedValue::Hex("0a84ff".into()));
    set_visual(&mut t, "on-accent", ResolvedValue::Ident("white".into()));
    set_visual(&mut t, "muted", ResolvedValue::Hex("888".into()));
    set_visual(&mut t, "danger", ResolvedValue::Ident("crimson".into()));
    set_visual(&mut t, "warn", ResolvedValue::Ident("orange".into()));
    set_visual(&mut t, "airwire", ResolvedValue::Ident("crimson".into()));
    set_visual(&mut t, "note-bg", ResolvedValue::Hex("fff9c4".into()));
    set_visual(&mut t, "group-stroke", ResolvedValue::Hex("bbb".into()));
    set_visual(
        &mut t,
        "group-fill",
        ResolvedValue::Call(ResolvedCall {
            name: "rgba".into(),
            args: vec![
                ResolvedValue::Number(0.0),
                ResolvedValue::Number(0.0),
                ResolvedValue::Number(0.0),
                ResolvedValue::Number(0.03),
            ],
        }),
    );
    set_visual(&mut t, "font", ResolvedValue::Ident("sans-serif".into()));
    // text-color defaults to var(--lini-fg).
    set_visual(
        &mut t,
        "text-color",
        ResolvedValue::LiveVar {
            name: "fg".into(),
            raw: false,
            baked: None,
        },
    );
    set_visual(
        &mut t,
        "shadow",
        ResolvedValue::Call(ResolvedCall {
            name: "rgba".into(),
            args: vec![
                ResolvedValue::Number(0.0),
                ResolvedValue::Number(0.0),
                ResolvedValue::Number(0.0),
                ResolvedValue::Number(0.2),
            ],
        }),
    );

    // Layout vars — baked at compile time. SPEC §12.3.
    // Text sizing splits three ways — body text, wire labels, and group
    // title/footer captions — so each can be tuned without touching the others.
    set_layout_n(&mut t, "text-size", 14.0);
    set_layout_n(&mut t, "wire-text-size", 13.0);
    set_layout_n(&mut t, "title-text-size", 13.0);
    set_layout_n(&mut t, "text-pad", 16.0);
    set_layout_n(&mut t, "gap", 20.0);
    set_layout_n(&mut t, "padding", 0.0);
    set_layout_n(&mut t, "thickness", 1.0);
    set_layout_n(&mut t, "radius", 0.0);
    set_layout_n(&mut t, "rect-w", 100.0);
    set_layout_n(&mut t, "rect-h", 40.0);
    set_layout_n(&mut t, "oval-w", 60.0);
    set_layout_n(&mut t, "oval-h", 40.0);
    set_layout_n(&mut t, "icon-size", 24.0);
    set_layout_n(&mut t, "canvas-pad", 20.0);
    set_layout_n(&mut t, "clearance", 16.0);

    t
}

fn set_layout_n(t: &mut VarTable, name: &str, n: f64) {
    t.set(name, VarKind::Layout, ResolvedValue::Number(n));
}

fn set_visual(t: &mut VarTable, name: &str, v: ResolvedValue) {
    t.set(name, VarKind::Visual, v);
}
