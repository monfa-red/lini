//! Attribute extraction + ResolvedValue → CSS-formatted string helpers used by
//! every render submodule.

use crate::Options;
use crate::resolve::{AttrMap, ResolvedCall, ResolvedValue, VarTable};

/// Format a value for use inline in SVG/CSS (e.g. an attribute value).
///
/// Live mode emits `var(--lini-name)`; bake mode follows the var indirectly
/// to a concrete CSS literal so renderers without custom-property support can
/// still display the diagram.
pub fn format_value(value: &ResolvedValue, vars: &VarTable, opts: &Options) -> String {
    match value {
        ResolvedValue::Number(n) => num(*n),
        ResolvedValue::Percent(n) => format!("{}%", num(*n)),
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
        ResolvedValue::Call(c) => {
            // Baked output carries no `light-dark()`; freeze its light arm so a
            // standalone file renders correctly [SPEC 10.6].
            if opts.static_mode && c.name == "light-dark" && !c.args.is_empty() {
                format_value(&c.args[0], vars, opts)
            } else {
                format_call(c, vars, opts)
            }
        }
        // A `fn:` formula is sampled to baked numbers at chart layout [SPEC 14.3],
        // and a `draw:` pen item folds to a path there [SPEC 15.3] — neither ever
        // reaches render.
        ResolvedValue::Deferred(_) => {
            unreachable!("a deferred fn: is sampled at chart layout, never rendered")
        }
        ResolvedValue::PenCall { .. } => {
            unreachable!("a draw: pen item folds to a path at layout, never rendered")
        }
        ResolvedValue::LiveVar { name, raw } => {
            // `--static` inlines each var to a literal so renderers without
            // `var()` support (and standalone SVG files) draw correctly;
            // otherwise the reference stays live.
            if opts.static_mode {
                if *raw {
                    // Raw CSS vars cannot be baked — we don't know their value.
                    format!("var(--{})", name)
                } else if let Some(value) = vars.get(name) {
                    format_value(value, vars, opts)
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

/// Format a paint/text property value for CSS output. lini numbers are unitless
/// [SPEC 2], so `font-size` and the length parts of `text-shadow` get `px`
/// added; every other property formats plainly.
pub fn css_value(prop: &str, value: &ResolvedValue, vars: &VarTable, opts: &Options) -> String {
    match prop {
        "font-size" => format!("{}px", format_value(value, vars, opts)),
        "text-shadow" => px_lengths(value, vars, opts),
        // The widened weight names [SPEC 6] are lini vocabulary, not CSS —
        // emit their numeric equivalents (`normal` / `bold` are valid CSS).
        "font-weight" => match value {
            ResolvedValue::Ident(w) if w == "medium" => "500".into(),
            ResolvedValue::Ident(w) if w == "semibold" => "600".into(),
            _ => format_value(value, vars, opts),
        },
        // Under `--embed-font`, a bundled family leads with its embedded
        // Lini-scoped twin; the authored name stays as the fallback [SPEC 17].
        "font-family" if opts.embed_font => {
            super::fonts::lead_with_scoped(&format_value(value, vars, opts))
        }
        _ => format_value(value, vars, opts),
    }
}

/// `text-shadow` formatting: every numeric offset/blur gains `px`; a tuple joins
/// with spaces, a comma-list with commas (multiple shadows). Colours and idents
/// pass through (a `--var` colour stays themeable).
fn px_lengths(value: &ResolvedValue, vars: &VarTable, opts: &Options) -> String {
    match value {
        ResolvedValue::Number(n) => format!("{}px", num(*n)),
        ResolvedValue::Tuple(items) => items
            .iter()
            .map(|i| px_lengths(i, vars, opts))
            .collect::<Vec<_>>()
            .join(" "),
        ResolvedValue::List(items) => items
            .iter()
            .map(|i| px_lengths(i, vars, opts))
            .collect::<Vec<_>>()
            .join(", "),
        other => format_value(other, vars, opts),
    }
}

/// An attribute formatted for SVG, falling back to the lini CSS variable named
/// `var_name` when unset. Going through `format_value` means `--static`
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
/// stroke is solid, the wave living in the link geometry ([`super::wavy`]); on a
/// shape it stays solid (deferred, [SPEC 23]). Shared by shapes, links, and the
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
/// `lini-link-*` class rules so both speak the same pattern.
pub fn dash_pattern(style: &str, width: f64) -> String {
    match style {
        // Dash and gap grow with the stroke, but **gently**: a flat `4×width`
        // made thick strokes gappy (16,16 at width 4 reads as dots adrift). Scale
        // a `width + 1` unit instead — dash 2 units, gap 1.5 (a 4:3 dash:gap) — so
        // the pattern runs 4,3 at 1px and 10,7.5 at 4px, staying tight as it thickens.
        "dashed" => {
            let unit = width + 1.0;
            format!("{},{}", num(unit * 2.0), num(unit * 1.5))
        }
        "dotted" => format!("{},{}", num(width), num(width * 2.0)),
        // The drafting conventions [SPEC 15.7]: `center` is dash-dot (axes,
        // symmetry), `phantom` dash-dot-dot (alternate positions) — a long run,
        // then dot(s) sized to the stroke, gapped like `dashed`.
        "center" => {
            let unit = width + 1.0;
            let (long, gap, dot) = (num(unit * 5.0), num(unit * 1.5), num(width));
            format!("{long},{gap},{dot},{gap}")
        }
        "phantom" => {
            let unit = width + 1.0;
            let (long, gap, dot) = (num(unit * 5.0), num(unit * 1.5), num(width));
            format!("{long},{gap},{dot},{gap},{dot},{gap}")
        }
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
        classes.push(format!("lini-{}", name));
    }
    classes.push(format!("lini-{}", primitive_kind));
    for name in applied_styles {
        classes.push(format!("lini-style-{}", name));
    }
    classes
}
