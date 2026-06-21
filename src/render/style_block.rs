//! Emit the `<style>` block: the `@layer lini.defaults` variable defaults
//! (host CSS wins automatically per SPEC §12.1) plus the unlayered structural
//! rules (SPEC §14 — paint rides CSS, geometry bakes; unlayered so renderers
//! that skip `@layer` still parse them).

use super::rules::RuleSet;
use super::values::format_value;
use crate::Options;
use crate::resolve::{ResolvedValue, VarTable};
use std::fmt::Write;

pub fn emit(out: &mut String, vars: &VarTable, rules: &RuleSet, opts: &Options) {
    out.push_str("  <style>\n");

    // `--bake-vars` inlines every value (the rules below carry literals), so the
    // themeable `@layer` block is only emitted when vars stay live.
    if !opts.bake_vars {
        let mut names: Vec<&String> = vars.entries.keys().collect();
        names.sort();
        if !names.is_empty() {
            // Adaptive when any colour is a light-dark() pair: emit `color-scheme`
            // so `light-dark()` follows the OS, plus the `data-theme` toggles that
            // force a mode by flipping it (SPEC §11.1).
            let adaptive = vars.entries.values().any(is_light_dark);
            out.push_str("    @layer lini.defaults {\n      :root, .lini {");
            if adaptive {
                out.push_str(" color-scheme: light dark;");
            }
            for name in &names {
                let value = vars.entries.get(*name).unwrap();
                write!(
                    out,
                    " --lini-{}: {};",
                    name,
                    format_value(value, vars, opts)
                )
                .unwrap();
            }
            out.push_str(" }\n");
            if adaptive {
                out.push_str(
                    "      .lini[data-theme=\"dark\"], [data-theme=\"dark\"] .lini { color-scheme: dark; }\n",
                );
                out.push_str(
                    "      .lini[data-theme=\"light\"], [data-theme=\"light\"] .lini { color-scheme: light; }\n",
                );
            }
            out.push_str("    }\n");
        }
    }

    rules.emit(out);
    out.push_str("  </style>\n");
}

/// A colour with both light and dark arms — the signal that the document is
/// adaptive and needs `color-scheme` + the `data-theme` toggles.
fn is_light_dark(v: &ResolvedValue) -> bool {
    matches!(v, ResolvedValue::Call(c) if c.name == "light-dark")
}
