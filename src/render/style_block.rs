//! Emit the `<style>` block: the `@layer lini.defaults` variable defaults
//! (host CSS wins automatically per [SPEC 10.1]) plus the unlayered structural
//! rules ([SPEC 18] — paint rides CSS, geometry bakes; unlayered so renderers
//! that skip `@layer` still parse them).

use super::fonts::{self, FontSink};
use super::rules::RuleSet;
use super::values::format_value;
use crate::Options;
use crate::resolve::VarTable;
use std::collections::BTreeSet;
use std::fmt::Write;

/// The figure's scope class — the identity of its own stylesheet, so two
/// figures inlined in one document share a scope only when their CSS is
/// byte-identical and sharing is therefore a no-op [`crate::name`]. Rendering
/// the body once under the bare `.lini` head is what it hashes; the `@font-face`
/// payload stays out (it is unscoped, and megabytes of base64 under
/// `--embed-font`).
pub fn scope_class(
    vars: &VarTable,
    rules: &RuleSet,
    used: &BTreeSet<String>,
    opts: &Options,
    tooltip_cards: usize,
) -> String {
    let mut probe = String::with_capacity(1024);
    emit_scoped(&mut probe, "lini", vars, rules, used, opts, tooltip_cards);
    crate::name::scope_class(&probe)
}

#[allow(clippy::too_many_arguments)] // the `<style>` block's full emission context
pub fn emit(
    out: &mut String,
    scope: &str,
    vars: &VarTable,
    rules: &RuleSet,
    used: &BTreeSet<String>,
    opts: &Options,
    tooltip_cards: usize,
    embed: Option<&FontSink>,
) {
    out.push_str("  <style>\n");

    // `--embed-font` [SPEC 18]: the used faces inline first, so the rules
    // below can already resolve against them.
    if let Some(sink) = embed {
        fonts::emit_font_faces(out, sink);
    }

    emit_scoped(out, scope, vars, rules, used, opts, tooltip_cards);
    out.push_str("  </style>\n");
}

/// Everything in the `<style>` whose selectors are headed by the figure's
/// scope class — the whole body bar the `@font-face` payload.
fn emit_scoped(
    out: &mut String,
    scope: &str,
    vars: &VarTable,
    rules: &RuleSet,
    used: &BTreeSet<String>,
    opts: &Options,
    tooltip_cards: usize,
) {
    // `--static` inlines every value (the rules below carry literals), so the
    // themeable `@layer` block is only emitted when vars stay live.
    if !opts.static_mode {
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
                .any(|n| vars.entries.get(*n).unwrap().is_light_dark());
            write!(out, "    @layer lini.defaults {{\n      :root, .{scope} {{").unwrap();
            if adaptive {
                out.push_str(" color-scheme: light dark;");
            }
            for name in &names {
                let value = vars.entries.get(*name).unwrap();
                let mut css = format_value(value, vars, opts);
                // Under `--embed-font` the default stack leads with the
                // embedded face's Lini-scoped name [SPEC 18].
                if opts.embed_font && *name == "font-family" {
                    css = fonts::lead_with_scoped(&css);
                }
                write!(out, " --lini-{}: {};", name, css).unwrap();
            }
            out.push_str(" }\n");
            if adaptive {
                for mode in ["dark", "light"] {
                    writeln!(
                        out,
                        "      .{scope}[data-theme=\"{mode}\"], [data-theme=\"{mode}\"] .{scope} {{ color-scheme: {mode}; }}"
                    )
                    .unwrap();
                }
            }
            out.push_str("    }\n");
        }
    }

    rules.emit(out, scope);
    // The rich chart tooltip [SPEC 14.8]: cards are hidden in a top layer; hovering
    // a mark (`.lini-hit-N`) reveals its `.lini-tip-N` card, a later sibling, so no other
    // mark can paint over it. Live-only — `--static` drops the cards and these rules.
    if tooltip_cards > 0 {
        writeln!(
            out,
            "    .{scope} .lini-chart-tip {{ visibility: hidden; pointer-events: none; }}"
        )
        .unwrap();
        for i in 0..tooltip_cards {
            writeln!(
                out,
                "    .{scope} .lini-hit-{i}:hover ~ .lini-tip-{i} {{ visibility: visible; }}"
            )
            .unwrap();
        }
    }
}
