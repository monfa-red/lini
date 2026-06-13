//! Theme file parser (`--theme FILE`).
//!
//! Extracts `--lini-*: value;` declarations from a CSS file; resolve layers
//! them over the built-in defaults.
//!
//! Intentionally a line scanner, not a CSS parser — we only care about a flat
//! set of custom-property declarations. Anything else in the file is ignored.

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
}
