use super::ir::{ResolvedCall, ResolvedValue, VarEntry, VarKind, VarTable};
use crate::ast::{FnCall, Value, VarOverride};
use crate::error::Error;

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
    set_layout_n(&mut t, "text-size", 13.0);
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

/// Apply a `--theme FILE`'s `--lini-*` overrides on top of the built-in
/// defaults. Theme entries arrive as `(name, raw_value_string)` from
/// `theme::extract_lini_vars`.
///
/// Theme values are parsed as standalone Lini values where possible — numbers,
/// hex colors, and tuple/call shapes go through the full lexer/parser path so
/// layout vars carry their numeric meaning. Anything that doesn't parse falls
/// back to a `String` value, which still emits verbatim in the SVG style block
/// and is acceptable for visual-only vars.
pub fn apply_theme(table: &mut VarTable, entries: &[(String, String)]) {
    for (name, raw) in entries {
        let value = parse_theme_value(raw);
        let kind = match table.get(name) {
            Some(VarEntry { kind, .. }) => *kind,
            None => VarKind::Visual,
        };
        table.set(name.clone(), kind, value);
    }
}

fn parse_theme_value(raw: &str) -> ResolvedValue {
    let s = raw.trim();
    // Try the standalone Lini value parser first so numbers, hexes, tuples,
    // and rgb()/rgba()/hsl() calls all round-trip with full type info.
    if let Some(v) = try_parse_via_lini(s) {
        return v;
    }
    // Anything we can't parse stays a literal CSS string. Emits verbatim in
    // the style block (e.g. `rgba(0,0,0,0.2)`, `Inter, sans-serif`) — perfect
    // for visual vars; layout vars that fall through here have no numeric
    // meaning and will trigger the visual-var error if used in a layout attr.
    ResolvedValue::String(s.to_string())
}

fn try_parse_via_lini(s: &str) -> Option<ResolvedValue> {
    let tokens = crate::lexer::lex(s).ok()?;
    // Empty input is not a value.
    if tokens.is_empty() {
        return None;
    }
    let ast_value = crate::parser::parse_value_only(&tokens).ok()?;
    convert_value(&ast_value)
}

fn convert_value(v: &crate::ast::Value) -> Option<ResolvedValue> {
    use crate::ast::Value;
    Some(match v {
        Value::Number(n) => ResolvedValue::Number(*n),
        Value::String(s) => ResolvedValue::String(s.clone()),
        Value::Hex(h) => ResolvedValue::Hex(h.clone()),
        Value::Ident(s) => ResolvedValue::Ident(s.clone()),
        Value::Tuple(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(convert_value(item)?);
            }
            ResolvedValue::Tuple(out)
        }
        Value::List(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(convert_value(item)?);
            }
            ResolvedValue::List(out)
        }
        Value::Call(c) => {
            let mut args = Vec::with_capacity(c.args.len());
            for arg in &c.args {
                args.push(convert_value(arg)?);
            }
            ResolvedValue::Call(crate::resolve::ResolvedCall {
                name: c.name.clone(),
                args,
            })
        }
        // Theme values referencing `--name` are uncommon; treat as identifier
        // for stringification purposes (the var system will resolve them when
        // referenced from Lini source).
        Value::RawCssVar(name) => ResolvedValue::Ident(format!("--{}", name)),
    })
}

/// Apply a sequence of `--name:value` defs-block overrides on top of the
/// table. Each entry overrides the previous value; unknown names are introduced
/// as Visual vars so user-defined `--lini-*` vars can be themed at runtime.
pub fn apply_var_overrides(table: &mut VarTable, entries: &[&VarOverride]) -> Result<(), Error> {
    for entry in entries {
        let value = resolve_value(&entry.value, table)?;
        let kind = match table.get(&entry.name) {
            Some(VarEntry { kind, .. }) => *kind,
            None => VarKind::Visual,
        };
        table.set(entry.name.clone(), kind, value);
    }
    Ok(())
}

/// Resolve a syntactic `Value` from the AST into a `ResolvedValue`. The only
/// transformation is `var()` → `LiveVar` with baked layout values where the
/// referenced var has VarKind::Layout.
pub fn resolve_value(value: &Value, table: &VarTable) -> Result<ResolvedValue, Error> {
    Ok(match value {
        Value::Number(n) => ResolvedValue::Number(*n),
        Value::String(s) => ResolvedValue::String(s.clone()),
        Value::Hex(h) => ResolvedValue::Hex(h.clone()),
        Value::Ident(s) => ResolvedValue::Ident(s.clone()),
        Value::Tuple(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(resolve_value(item, table)?);
            }
            ResolvedValue::Tuple(out)
        }
        Value::List(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(resolve_value(item, table)?);
            }
            ResolvedValue::List(out)
        }
        Value::Call(call) => resolve_call(call, table)?,
        Value::RawCssVar(name) => {
            // SPEC §12.2: `--name` refers to `--lini-name`. Layout vars bake
            // their value; visual vars stay live for runtime CSS.
            let baked = match table.get(name) {
                Some(VarEntry {
                    kind: VarKind::Layout,
                    value,
                }) => Some(Box::new(value.clone())),
                _ => None,
            };
            ResolvedValue::LiveVar {
                name: name.clone(),
                raw: false,
                baked,
            }
        }
    })
}

fn resolve_call(call: &FnCall, table: &VarTable) -> Result<ResolvedValue, Error> {
    // SPEC v1 drops the `var(...)` function. Authors write `--name` directly.
    if call.name == "var" {
        return Err(Error::at(
            call.span,
            "var() is no longer a function — write '--name' directly to reference a Lini CSS var",
        ));
    }
    let mut args = Vec::with_capacity(call.args.len());
    for arg in &call.args {
        args.push(resolve_value(arg, table)?);
    }
    Ok(ResolvedValue::Call(ResolvedCall {
        name: call.name.clone(),
        args,
    }))
}
