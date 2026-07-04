//! Emit the `<style>` block: the `@layer lini.defaults` variable defaults
//! (host CSS wins automatically per [SPEC 10.1]) plus the unlayered structural
//! rules ([SPEC 16] — paint rides CSS, geometry bakes; unlayered so renderers
//! that skip `@layer` still parse them).

use super::rules::RuleSet;
use super::values::format_value;
use crate::Options;
use crate::resolve::{ResolvedValue, VarTable};
use std::collections::BTreeSet;
use std::fmt::Write;

pub fn emit(
    out: &mut String,
    vars: &VarTable,
    rules: &RuleSet,
    used: &BTreeSet<String>,
    opts: &Options,
    tooltip_cards: usize,
) {
    out.push_str("  <style>\n");

    // `--bake-vars` inlines every value (the rules below carry literals), so the
    // themeable `@layer` block is only emitted when vars stay live.
    if !opts.bake_vars {
        // Tree-shake: emit only the vars the document references [SPEC 10.2/16],
        // so the built-in palette never bloats a diagram that doesn't use it.
        let mut names: Vec<&String> = vars
            .entries
            .keys()
            .filter(|k| used.contains(k.as_str()))
            .collect();
        names.sort();
        if !names.is_empty() {
            // Adaptive when any emitted colour is a light-dark() pair: emit
            // `color-scheme` so `light-dark()` follows the OS, plus the `data-theme`
            // toggles that force a mode by flipping it [SPEC 10.1].
            let adaptive = names
                .iter()
                .any(|n| is_light_dark(vars.entries.get(*n).unwrap()));
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
    // The rich chart tooltip [SPEC 14.8]: cards are hidden in a top layer; hovering
    // a mark (`.lini-hit-N`) reveals its `.lini-tip-N` card, a later sibling, so no other
    // mark can paint over it. Live-only — `--bake-vars` drops the cards and these rules.
    if tooltip_cards > 0 {
        out.push_str("    .lini .lini-chart-tip { visibility: hidden; pointer-events: none; }\n");
        for i in 0..tooltip_cards {
            writeln!(
                out,
                "    .lini .lini-hit-{i}:hover ~ .lini-tip-{i} {{ visibility: visible; }}"
            )
            .unwrap();
        }
    }
    out.push_str("  </style>\n");
}

/// A colour with both light and dark arms — the signal that the document is
/// adaptive and needs `color-scheme` + the `data-theme` toggles.
fn is_light_dark(v: &ResolvedValue) -> bool {
    matches!(v, ResolvedValue::Call(c) if c.name == "light-dark")
}
