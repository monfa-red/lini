//! Tree-shake the `@layer` variable block (SPEC §11.2 / §13): collect the `--lini-*`
//! variables a document actually references — directly or transitively — so the
//! built-in palette costs a diagram that doesn't use it nothing.
//!
//! Sources, gathered before the block is emitted:
//! 1. the structural class rules, which always state the core roles as `var(--lini-…)`;
//! 2. every output-bound `ResolvedValue` — node / wire / sheet / canvas attrs;
//! 3. the transitive closure over the var table — a kept var whose value names another
//!    (`text-color → fg`, an alias → its hue) pulls that one in too.
//!
//! **Invariant:** a variable is emitted iff it is reachable from this set. The
//! structural rules pin the roles the renderer always paints with, so nothing the
//! output references is ever dropped.

use super::rules::RuleSet;
use crate::layout::{LaidOut, PlacedNode};
use crate::resolve::{AttrMap, ResolvedValue, VarTable};
use std::collections::BTreeSet;

/// The set of `--lini-*` names (without the prefix) the document references.
pub fn referenced(laid: &LaidOut, ruleset: &RuleSet) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    for rule in &ruleset.rules {
        for (_, value) in &rule.props {
            scan_css(value, &mut names);
        }
    }

    for n in &laid.nodes {
        walk_node(n, &mut names);
    }
    for w in &laid.wires {
        walk_attrs(&w.attrs, &mut names);
        for t in &w.texts {
            walk_attrs(&t.attrs, &mut names);
        }
    }
    if !laid.airwires.is_empty() {
        names.insert("airwire".to_string());
    }
    for (_, attrs) in &laid.sheet.class_rules {
        walk_attrs(attrs, &mut names);
    }
    walk_attrs(&laid.sheet.wire_defaults, &mut names);
    walk_attrs(&laid.sheet.root_text, &mut names);
    if let Some(fill) = &laid.canvas_fill {
        collect_live(fill, &mut names);
    }
    // Gradient stops were rewritten out of the attrs into `laid.gradients`; walk
    // them so their palette vars survive the shake (SPEC §11.3).
    for g in &laid.gradients {
        for stop in &g.stops {
            collect_live(stop, &mut names);
        }
    }

    close_over_vars(&mut names, &laid.vars);
    names
}

fn walk_node(n: &PlacedNode, names: &mut BTreeSet<String>) {
    walk_attrs(&n.attrs, names);
    for c in &n.children {
        walk_node(c, names);
    }
}

fn walk_attrs(attrs: &AttrMap, names: &mut BTreeSet<String>) {
    for value in attrs.map.values() {
        collect_live(value, names);
    }
}

/// Add every non-raw `LiveVar` name reachable in `value`.
fn collect_live(value: &ResolvedValue, names: &mut BTreeSet<String>) {
    match value {
        ResolvedValue::LiveVar { name, raw: false } => {
            names.insert(name.clone());
        }
        ResolvedValue::Tuple(items) | ResolvedValue::List(items) => {
            items.iter().for_each(|v| collect_live(v, names));
        }
        ResolvedValue::Call(c) => c.args.iter().for_each(|v| collect_live(v, names)),
        _ => {}
    }
}

/// Find `var(--lini-NAME)` references in a formatted CSS string (the structural
/// rules carry their colours this way, not as `ResolvedValue`s).
fn scan_css(s: &str, names: &mut BTreeSet<String>) {
    const PREFIX: &str = "var(--lini-";
    let bytes = s.as_bytes();
    let mut i = 0;
    while let Some(off) = s[i..].find(PREFIX) {
        let start = i + off + PREFIX.len();
        let mut end = start;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'-') {
            end += 1;
        }
        if end > start {
            names.insert(s[start..end].to_string());
        }
        i = end.max(start + 1);
    }
}

/// Pull in any var a kept var's value references, to a fixpoint.
fn close_over_vars(names: &mut BTreeSet<String>, vars: &VarTable) {
    let mut frontier: Vec<String> = names.iter().cloned().collect();
    while let Some(name) = frontier.pop() {
        if let Some(value) = vars.get(&name) {
            let mut found = BTreeSet::new();
            collect_live(value, &mut found);
            for f in found {
                if names.insert(f.clone()) {
                    frontier.push(f);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn used(src: &str) -> BTreeSet<String> {
        let toks = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(&toks).expect("parse");
        let lowered = crate::desugar::desugar(&file).expect("desugar");
        let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
        let laid = crate::layout::layout(&program).expect("layout");
        let ruleset = super::super::rules::build(&laid, &crate::Options::default());
        referenced(&laid, &ruleset)
    }

    #[test]
    fn plain_diagram_keeps_core_roles_not_the_palette() {
        let names = used("x |box|\n");
        // Structural rules always paint with these.
        assert!(names.contains("fill"), "{names:?}");
        assert!(names.contains("stroke"), "{names:?}");
        assert!(names.contains("bg"), "{names:?}");
        // No palette hue is referenced.
        assert!(!names.contains("teal"), "{names:?}");
        assert!(!names.contains("rose"), "{names:?}");
    }

    #[test]
    fn a_used_hue_is_kept_others_are_not() {
        let names = used("x |box| { fill: --teal }\n");
        assert!(names.contains("teal"), "{names:?}");
        assert!(!names.contains("rose"), "{names:?}");
    }

    #[test]
    fn text_color_pulls_in_fg_transitively() {
        // `.lini` states `color: var(--lini-text-color)`, whose value is `var(--lini-fg)`.
        let names = used("x |box|\n");
        assert!(names.contains("text-color"), "{names:?}");
        assert!(names.contains("fg"), "{names:?}");
    }
}
