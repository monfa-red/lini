//! Themes: the `--theme` argument and the built-in palettes (SPEC §11/§14).
//!
//! A theme is a set of `--lini-*` values. Built-ins are typed palettes here;
//! `--theme FILE` reads the same shape from CSS ([`extract_lini_vars`]). Both flow
//! through the one apply path in [`super::resolve`], so a built-in and a user file
//! are the same mechanism. `builtin_css` / `pair_css` render a palette back to CSS
//! for `lini theme` — the boilerplate a user copies.

use crate::Options;
use crate::render::values::format_value;
use crate::resolve::{ResolvedCall, ResolvedValue, VarTable, built_in_defaults};
use std::collections::BTreeSet;

/// Extract `(name_without_lini_prefix, raw_value_string)` pairs from CSS-like
/// text. Names without the `--lini-` prefix are skipped — those are not
/// Lini's to own.
pub fn extract_lini_vars(src: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let cleaned = strip_block_comments(src);
    // Split on `;` to walk declarations one at a time (works whether they sit
    // on separate lines or share a line).
    for decl in cleaned.split(';') {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }
        let Some(start) = decl.find("--lini-") else {
            continue;
        };
        let rest = &decl[start + "--lini-".len()..];
        let Some(colon) = rest.find(':') else {
            continue;
        };
        let name = rest[..colon].trim();
        let value = rest[colon + 1..].trim();
        // Trim any trailing `}` that landed in this segment (e.g.,
        // `gap: 10; }` after the split).
        let value = value.trim_end_matches('}').trim();
        if name.is_empty() || value.is_empty() {
            continue;
        }
        out.push((name.to_string(), value.to_string()));
    }
    out
}

/// Remove `/* … */` block comments. Themes are simple flat files; we don't
/// support nested comments.
fn strip_block_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(open) = rest.find("/*") {
        out.push_str(&rest[..open]);
        rest = match rest[open + 2..].find("*/") {
            Some(close) => &rest[open + 2 + close + 2..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

// ─────────────────────────── Built-in themes ───────────────────────────

/// Built-in theme names + one-line descriptions, for `lini theme`.
pub fn list_themes() -> &'static [(&'static str, &'static str)] {
    &[
        (
            "default",
            "light + dark, follows the OS (the no-flag output)",
        ),
        ("light", "the light palette alone"),
        ("dark", "the dark palette alone"),
        ("high-contrast", "maximal contrast, light + dark (a11y)"),
    ]
}

/// The CSS for a built-in theme — the `--lini-*` declarations a user can copy
/// (SPEC §14). `None` for an unknown name.
pub fn builtin_css(name: &str) -> Option<String> {
    Some(to_css(&palette(name)?))
}

/// Compose two built-ins into one adaptive theme's CSS: `light`'s palette as the
/// light arm, `dark`'s as the dark arm. `None` if either name is unknown.
pub fn pair_css(light: &str, dark: &str) -> Option<String> {
    let mut l = palette(light)?;
    let mut d = palette(dark)?;
    collapse(&mut l, 0);
    collapse(&mut d, 1);
    let opts = Options::default();
    let mut out = VarTable::new();
    let mut keys: BTreeSet<&String> = l.entries.keys().collect();
    keys.extend(d.entries.keys());
    for k in keys {
        let val = match (l.get(k), d.get(k)) {
            (Some(a), Some(b)) => {
                // Identical in both arms → a single value, no light-dark().
                if format_value(a, &l, &opts) == format_value(b, &d, &opts) {
                    a.clone()
                } else {
                    ld(a.clone(), b.clone())
                }
            }
            (Some(v), None) | (None, Some(v)) => v.clone(),
            (None, None) => continue,
        };
        out.set(k.clone(), val);
    }
    Some(to_css(&out))
}

/// The fully-resolved palette for a built-in name (`None` if unknown). Single
/// themes collapse the base light-dark() pairs to one arm, then layer their look.
fn palette(name: &str) -> Option<VarTable> {
    let mut v = built_in_defaults();
    match name {
        "default" | "auto" => {}
        "light" => collapse(&mut v, 0),
        "dark" => collapse(&mut v, 1),
        "high-contrast" => apply(&mut v, &high_contrast()),
        _ => return None,
    }
    Some(v)
}

fn apply(v: &mut VarTable, overrides: &[(&str, ResolvedValue)]) {
    for (n, val) in overrides {
        v.set(*n, val.clone());
    }
}

/// Replace every `light-dark(l, d)` with its `arm` (0 = light, 1 = dark).
fn collapse(v: &mut VarTable, arm: usize) {
    for val in v.entries.values_mut() {
        if let ResolvedValue::Call(c) = val
            && c.name == "light-dark"
            && c.args.len() == 2
        {
            *val = c.args[arm].clone();
        }
    }
}

/// Render a palette to the canonical theme CSS (SPEC §14). `color-scheme` rides
/// the rule when adaptive; `font-family` is commented so the engine default
/// (monospace, exact text sizing) holds unless a user uncomments it.
fn to_css(vars: &VarTable) -> String {
    let opts = Options::default();
    let mut names: Vec<&String> = vars.entries.keys().collect();
    names.sort();
    let adaptive = vars.entries.values().any(is_light_dark);
    let mut out = String::new();
    out.push_str("/* lini theme — copy & edit. Colours; sizes are baked, not themeable. */\n");
    out.push_str(":root, .lini {\n");
    if adaptive {
        out.push_str("  color-scheme: light dark;\n");
    }
    for n in names {
        let v = vars.entries.get(n).unwrap();
        let css = format_value(v, vars, &opts);
        if n == "font-family" {
            // Optional: a host font; commented so monospace (exact sizing) holds.
            out.push_str(&format!("  /* --lini-font-family: {}; */\n", css));
        } else {
            out.push_str(&format!("  --lini-{}: {};\n", n, css));
        }
    }
    out.push_str("}\n");
    out
}

fn is_light_dark(v: &ResolvedValue) -> bool {
    matches!(v, ResolvedValue::Call(c) if c.name == "light-dark")
}

// ── Palette value constructors ──
fn idn(s: &str) -> ResolvedValue {
    ResolvedValue::Ident(s.into())
}
fn hx(s: &str) -> ResolvedValue {
    ResolvedValue::Hex(s.into())
}
fn rgba(r: f64, g: f64, b: f64, a: f64) -> ResolvedValue {
    ResolvedValue::Call(ResolvedCall {
        name: "rgba".into(),
        args: vec![
            ResolvedValue::Number(r),
            ResolvedValue::Number(g),
            ResolvedValue::Number(b),
            ResolvedValue::Number(a),
        ],
    })
}
fn ld(l: ResolvedValue, d: ResolvedValue) -> ResolvedValue {
    ResolvedValue::Call(ResolvedCall {
        name: "light-dark".into(),
        args: vec![l, d],
    })
}

/// Maximal-contrast palette, light + dark (a11y). Colour only — line weights bake.
fn high_contrast() -> Vec<(&'static str, ResolvedValue)> {
    vec![
        ("bg", ld(idn("white"), idn("black"))),
        ("fg", ld(idn("black"), idn("white"))),
        ("fill", ld(idn("white"), idn("black"))),
        ("stroke", ld(idn("black"), idn("white"))),
        ("accent", ld(hx("0033cc"), hx("66aaff"))),
        ("accent-text", idn("white")),
        ("muted", ld(hx("333333"), hx("cccccc"))),
        ("group-stroke", ld(idn("black"), idn("white"))),
        (
            "group-fill",
            ld(rgba(0.0, 0.0, 0.0, 0.0), rgba(0.0, 0.0, 0.0, 0.0)),
        ),
        ("caption-color", ld(idn("black"), idn("white"))),
        ("footer-color", ld(idn("black"), idn("white"))),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_simple_var() {
        let css = ".lini { --lini-gap: 30; }";
        let vars = extract_lini_vars(css);
        assert_eq!(vars, vec![("gap".into(), "30".into())]);
    }

    #[test]
    fn extracts_multiple_lines() {
        let css = "\
            :root, .lini {\n\
              --lini-gap: 30;\n\
              --lini-accent: hotpink;\n\
              --lini-thickness: 2;\n\
            }\n\
        ";
        let vars = extract_lini_vars(css);
        assert_eq!(
            vars,
            vec![
                ("gap".into(), "30".into()),
                ("accent".into(), "hotpink".into()),
                ("thickness".into(), "2".into()),
            ]
        );
    }

    #[test]
    fn ignores_non_lini_vars() {
        let css = "--my-var: 5; --lini-gap: 10;";
        let vars = extract_lini_vars(css);
        assert_eq!(vars, vec![("gap".into(), "10".into())]);
    }

    #[test]
    fn handles_missing_semicolon() {
        let css = "--lini-gap: 30";
        let vars = extract_lini_vars(css);
        assert_eq!(vars, vec![("gap".into(), "30".into())]);
    }

    #[test]
    fn skips_inline_block_comments() {
        let css = "--lini-gap: 30; /* a comment */";
        let vars = extract_lini_vars(css);
        assert_eq!(vars, vec![("gap".into(), "30".into())]);
    }

    #[test]
    fn survives_non_ascii_comments_and_values() {
        let css = "/* thème de l'équipe — «bleu» */ --lini-font: \"Σans\";";
        let vars = extract_lini_vars(css);
        assert_eq!(vars, vec![("font".into(), "\"Σans\"".into())]);
    }

    #[test]
    fn default_theme_is_adaptive_dark_is_not() {
        // The default carries light-dark() pairs; `dark` collapses to one arm.
        assert!(builtin_css("default").unwrap().contains("light-dark("));
        assert!(
            builtin_css("default")
                .unwrap()
                .contains("color-scheme: light dark")
        );
        let dark = builtin_css("dark").unwrap();
        assert!(!dark.contains("light-dark("));
        assert!(dark.contains("--lini-bg: #1b1b1f;"));
    }

    #[test]
    fn font_family_is_commented_in_theme_css() {
        assert!(
            builtin_css("light")
                .unwrap()
                .contains("/* --lini-font-family:")
        );
    }

    #[test]
    fn unknown_theme_is_none() {
        assert!(builtin_css("nope").is_none());
    }

    #[test]
    fn pair_composes_arms() {
        // `light/dark` reconstructs the adaptive default.
        let css = pair_css("light", "dark").unwrap();
        assert!(css.contains("light-dark(white, #1b1b1f)"));
    }
}
