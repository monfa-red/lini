//! Themes: the `--theme` argument and the built-in palettes [SPEC 10/16].
//!
//! A theme is a set of `--lini-*` values. Built-ins are typed palettes here;
//! `--theme FILE` reads the same shape from CSS ([`extract_lini_vars`]). Both flow
//! through the one apply path in [`super::resolve`], so a built-in and a user file
//! are the same mechanism. `builtin_css` / `pair_css` render a palette back to CSS
//! for `lini theme` — the boilerplate a user copies.

use crate::Options;
use crate::render::values::format_value;
use crate::resolve::defaults::{hex as hx, ident as idn, light_dark as ld, rgba};
use crate::resolve::{ResolvedValue, VarTable, built_in_defaults};
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
        (
            "blueprint",
            "white linework on Prussian blue — the diazo print, one look",
        ),
    ]
}

/// The CSS for a built-in theme — the `--lini-*` declarations a user can copy
/// [SPEC 18]. `None` for an unknown name.
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
        // One look in every mode: collapse to the dark arm — the paper is dark,
        // so every hue takes its dark-mode job — then paint the paper over it.
        "blueprint" => {
            collapse(&mut v, 1);
            apply(&mut v, &blueprint());
        }
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

/// Render a palette to the canonical theme CSS [SPEC 18]. `color-scheme` rides
/// the rule when adaptive; `font-family` is commented so the engine default
/// (monospace, exact text sizing) holds unless a user uncomments it.
fn to_css(vars: &VarTable) -> String {
    let opts = Options::default();
    let mut names: Vec<&String> = vars.entries.keys().collect();
    names.sort();
    let adaptive = vars.entries.values().any(ResolvedValue::is_light_dark);
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

/// The blueprint — white linework on Prussian-blue paper, the diazo print look,
/// one look in every mode. Only the black-and-white roles are repainted; the
/// named-hue palette [SPEC 10.2] passes through as the **dark arm** it was
/// collapsed to, since the paper sits at that arm's own lightness (OKLCH L 0.30
/// against the dark default's 0.23): a hue's `-ink` still reads as text straight
/// on the paper, and its `-wash` / `-soft` still read as surfaces over it — which
/// the light arm's pale tiers, pasted on as bright plates, do not. `text-color`,
/// the fonts and the weights are not colours of the paper: they pass through.
fn blueprint() -> Vec<(&'static str, ResolvedValue)> {
    // Linework is the paper's own white at strength — one pen, many pressures,
    // so a line crossing a filled shape blends toward it instead of greying it
    // (the reasoning behind `--lini-stroke-light` [SPEC 10.1]).
    let pen = |a: f64| rgba(255.0, 255.0, 255.0, a);
    vec![
        // The paper: OKLCH (0.30, 0.093, 253) — Prussian blue, the diazo ground.
        ("bg", hx("002e5b")),
        ("sheet", hx("002e5b")),
        ("fg", hx("edf5fb")),
        // A body sits one step up from the paper (L 0.355) — opaque, so it still
        // masks what it overlaps, and blue, so nothing reads as a white plate.
        ("fill", hx("0f3d69")),
        ("stroke", pen(0.78)),
        // The primary drafting tone: full white. A floorplan's poché fills with
        // it [SPEC 15.11], so walls read solid white on blue.
        ("stroke-dark", idn("white")),
        ("stroke-light", pen(0.45)),
        ("accent", hx("7adff7")),
        ("accent-text", hx("0a2c4e")),
        ("muted", pen(0.62)),
        ("danger", hx("f97770")),
        ("warn", hx("f3bd5c")),
        ("stray", hx("f97770")),
        ("group-stroke", pen(0.4)),
        ("group-fill", pen(0.05)),
        ("header-fill", pen(0.1)),
        ("icon-fill", pen(0.18)),
        ("caption-color", pen(0.62)),
        ("footer-color", pen(0.62)),
        ("grid", pen(0.16)),
        // The tooltip card inverts the paper, as it does in every theme.
        ("tip-bg", hx("edf5fb")),
        ("tip-fg", hx("002e5b")),
        // A print is flat: the shadow is a whisper, not a lift.
        ("shadow-color", rgba(0.0, 0.0, 0.0, 0.18)),
        // The schematic roles [SPEC 16.6]: wires and net tags as lighter tints
        // of the pen, part bodies the same faint card as any other fill.
        ("wire", hx("a8d8f0")),
        ("component-fill", hx("0f3d69")),
        ("component-stroke", idn("white")),
        ("label-ink", hx("7bd9de")),
        ("pin-number", pen(0.55)),
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
    fn blueprint_is_one_look_on_prussian_paper() {
        let css = builtin_css("blueprint").unwrap();
        // A blueprint is a blueprint in either mode: no arms, no color-scheme.
        assert!(!css.contains("light-dark("), "{css}");
        assert!(!css.contains("color-scheme"), "{css}");
        // The paper, and the pen a floorplan's poché fills with [SPEC 15.11].
        assert!(css.contains("--lini-bg: #002e5b;"), "{css}");
        assert!(css.contains("--lini-sheet: #002e5b;"), "{css}");
        assert!(css.contains("--lini-stroke-dark: white;"), "{css}");
    }

    #[test]
    fn blueprint_covers_the_whole_role_roster() {
        // Every `--lini-*` the defaults carry [SPEC 10.1/10.2] survives the
        // theme — a role dropped here would fall back to a light-mode default
        // and paint black on the blue.
        let names = |css: &str| -> BTreeSet<String> {
            extract_lini_vars(css).into_iter().map(|(n, _)| n).collect()
        };
        let default = names(&builtin_css("default").unwrap());
        let blueprint = names(&builtin_css("blueprint").unwrap());
        // `font-family` is emitted commented out, so it is in neither set.
        assert_eq!(default, blueprint);
    }

    #[test]
    fn blueprint_hues_are_the_dark_arm() {
        // The named-hue palette [SPEC 10.2] passes through: the paper is dark,
        // so every hue takes its dark-mode job rather than a repainted one.
        let bp = builtin_css("blueprint").unwrap();
        let dark = builtin_css("dark").unwrap();
        for line in ["--lini-teal-ink: ", "--lini-rose-soft: ", "--lini-amber: "] {
            let of = |css: &str| {
                css.lines()
                    .find(|l| l.trim_start().starts_with(line))
                    .unwrap()
                    .to_string()
            };
            assert_eq!(of(&bp), of(&dark), "{line} must pass through");
        }
    }

    #[test]
    fn pair_composes_arms() {
        // `light/dark` reconstructs the adaptive default.
        let css = pair_css("light", "dark").unwrap();
        assert!(css.contains("light-dark(white, #1b1b1f)"));
    }
}
