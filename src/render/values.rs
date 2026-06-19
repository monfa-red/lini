//! Attribute extraction + ResolvedValue → CSS-formatted string helpers used by
//! every render submodule.

use crate::Options;
use crate::resolve::{AttrMap, ResolvedCall, ResolvedValue, VarKind, VarTable};

/// Format a value for use inline in SVG/CSS (e.g. an attribute value).
///
/// Live mode emits `var(--lini-name)`; bake mode follows the var indirectly
/// to a concrete CSS literal so renderers without custom-property support can
/// still display the diagram.
pub fn format_value(value: &ResolvedValue, vars: &VarTable, opts: &Options) -> String {
    match value {
        ResolvedValue::Number(n) => num(*n),
        ResolvedValue::String(s) => format!("\"{}\"", s),
        ResolvedValue::Hex(h) => format!("#{}", h),
        ResolvedValue::Ident(s) => s.clone(),
        ResolvedValue::RawCss(s) => s.clone(),
        ResolvedValue::Tuple(items) => {
            let parts: Vec<String> = items.iter().map(|v| format_value(v, vars, opts)).collect();
            parts.join(" ")
        }
        ResolvedValue::List(items) => {
            let parts: Vec<String> = items.iter().map(|v| format_value(v, vars, opts)).collect();
            parts.join(", ")
        }
        ResolvedValue::Call(c) => format_call(c, vars, opts),
        ResolvedValue::LiveVar { name, raw, baked } => {
            // Layout vars are language defaults, not CSS-themable: always
            // bake them to a literal so renderers without `var()` support
            // (and standalone SVG files) draw correctly. Visual vars stay
            // live in non-bake mode.
            let is_layout = !*raw && vars.get(name).is_some_and(|e| e.kind == VarKind::Layout);
            if opts.bake_vars || is_layout {
                if *raw {
                    // Raw CSS vars cannot be baked — we don't know their value.
                    format!("var(--{})", name)
                } else if let Some(entry) = vars.get(name) {
                    format_value(&entry.value, vars, opts)
                } else if let Some(b) = baked {
                    format_value(b, vars, opts)
                } else {
                    format!("var(--lini-{})", name)
                }
            } else if *raw {
                format!("var(--{})", name)
            } else {
                format!("var(--lini-{})", name)
            }
        }
    }
}

fn format_call(c: &ResolvedCall, vars: &VarTable, opts: &Options) -> String {
    let parts: Vec<String> = c.args.iter().map(|a| format_value(a, vars, opts)).collect();
    format!("{}({})", c.name, parts.join(", "))
}

/// An attribute formatted for SVG, falling back to the lini CSS variable named
/// `var_name` when unset. Going through `format_value` means `--bake-vars`
/// correctly resolves the fallback to a literal rather than leaving a `var()`
/// string.
pub fn attr_or_var(
    attrs: &AttrMap,
    name: &str,
    var_name: &str,
    vars: &VarTable,
    opts: &Options,
) -> String {
    match attrs.get(name) {
        Some(v) => format_value(v, vars, opts),
        None => format_value(
            &ResolvedValue::LiveVar {
                name: var_name.to_string(),
                raw: false,
                baked: None,
            },
            vars,
            opts,
        ),
    }
}

pub fn attr_points(attrs: &AttrMap, name: &str) -> Option<Vec<(f64, f64)>> {
    match attrs.get(name)? {
        ResolvedValue::List(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                if let ResolvedValue::Tuple(t) = item
                    && t.len() == 2
                {
                    let x = t[0].as_number()?;
                    let y = t[1].as_number()?;
                    out.push((x, y));
                    continue;
                }
                return None;
            }
            Some(out)
        }
        _ => None,
    }
}

/// The `stroke-dasharray` pattern for `stroke-style: dashed|dotted` (sized
/// against `stroke-width`), or empty for solid. `wavy` also returns empty — its
/// stroke is solid, the wave living in the wire geometry ([`super::wavy`]); on a
/// shape it stays solid (deferred, SPEC §19). Shared by shapes, wires, and the
/// rules builder.
pub fn dasharray_value(attrs: &AttrMap, width: f64) -> String {
    match attrs.get("stroke-style") {
        Some(ResolvedValue::Ident(s)) => dash_pattern(s, width),
        _ => String::new(),
    }
}

/// The `stroke-dasharray` for a named line style at `width`, empty for any style
/// without a dash (`solid`, `wavy`). Sized against `stroke-width` so the pattern
/// stays proportional as a line thickens. Shared by the inline diff and the
/// `lini-wire-*` class rules so both speak the same pattern.
pub fn dash_pattern(style: &str, width: f64) -> String {
    match style {
        "dashed" => format!("{},{}", num(width * 4.0), num(width * 4.0)),
        "dotted" => format!("{},{}", num(width), num(width * 2.0)),
        _ => String::new(),
    }
}

/// Format a number with minimal precision (integers stay integers; floats trim
/// trailing zeros). Used everywhere for compact, snapshot-stable SVG.
pub fn num(n: f64) -> String {
    if n.is_finite() && n == n.trunc() && n.abs() < 1e15 {
        return (n as i64).to_string();
    }
    let s = format!("{:.4}", n);
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

pub fn class_list(
    primitive_kind: &str,
    type_chain: &[String],
    applied_styles: &[String],
) -> Vec<String> {
    let mut classes = vec!["lini-node".to_string()];
    for name in type_chain {
        classes.push(format!("lini-shape-{}", name));
    }
    classes.push(format!("lini-shape-{}", primitive_kind));
    for name in applied_styles {
        classes.push(format!("lini-style-{}", name));
    }
    classes
}
