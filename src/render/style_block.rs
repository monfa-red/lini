//! Emit the `<style>` block: the `@layer lini.defaults` variable defaults
//! (host CSS wins automatically per SPEC §12.1) plus the unlayered structural
//! rules (SPEC §14 — paint rides CSS, geometry bakes; unlayered so renderers
//! that skip `@layer` still parse them).

use super::rules::RuleSet;
use super::values::format_value;
use crate::Options;
use crate::resolve::{VarKind, VarTable};
use std::fmt::Write;

pub fn emit(out: &mut String, vars: &VarTable, rules: &RuleSet, opts: &Options) {
    out.push_str("  <style>\n");

    // `--bake-vars` inlines every value (the rules below carry literals), so the
    // themeable `@layer` block is only emitted when vars stay live.
    if !opts.bake_vars {
        let mut names: Vec<&String> = vars
            .entries
            .iter()
            .filter(|(_, e)| e.kind == VarKind::Visual)
            .map(|(n, _)| n)
            .collect();
        names.sort();
        if !names.is_empty() {
            out.push_str("    @layer lini.defaults { :root, .lini {");
            for (i, name) in names.iter().enumerate() {
                let entry = vars.entries.get(*name).unwrap();
                if i > 0 {
                    out.push(' ');
                }
                write!(
                    out,
                    " --lini-{}: {};",
                    name,
                    format_value(&entry.value, vars, opts)
                )
                .unwrap();
            }
            out.push_str(" } }\n");
        }
    }

    rules.emit(out);
    out.push_str("  </style>\n");
}
