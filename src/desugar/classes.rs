//! `.lini-*` class generation and the reserved-prefix split. A type chain becomes
//! worn classes in **render order** — derived→base, then the primitive last —
//! matching the pre-desugar `lini-shape-*` class order so the SVG class attribute
//! is byte-identical; the tier-1 cascade folds them in reverse (primitive first,
//! derived last wins). Each present type name gets one
//! `.lini-<name> { bundle + element-rule decls }` stylesheet rule.

use super::types::{TEMPLATES, TypeInfo};
use crate::ledger::defaults::{primitive_bundle, template_bundle};
use crate::resolve::NodeKind;
use crate::span::Span;
use crate::syntax::ast::{Decl, Rule, SelUnit, Selector, Value};
use std::collections::{BTreeSet, HashMap, HashSet};

/// The `.lini-<name>` class for a type/primitive name.
pub fn lini_class(name: &str) -> String {
    format!("lini-{name}")
}

/// Whether a worn class is a generated type class (the reserved prefix) rather
/// than a user style class.
pub fn is_lini_class(name: &str) -> bool {
    name.starts_with("lini-")
}

/// The classes a typed instance wears, in render order — derived→base, then the
/// primitive last — matching the pre-desugar SVG so the class attribute is
/// byte-identical. The tier-1 cascade folds them in reverse.
pub fn worn_classes(info: &TypeInfo) -> Vec<String> {
    let mut out: Vec<String> = info.chain.iter().rev().map(|n| lini_class(n)).collect();
    out.push(lini_class(info.kind.as_str()));
    out
}

/// One class def per present type name, ordered primitives → templates → extras
/// (defines / lowered define-classes, base before derived), each = its bundle
/// merged with its element-rule decls. A name with no decls (e.g. `image`) is
/// skipped — it is still worn for the render class, just carries no rule.
/// `present` holds bare type names (e.g. "box", "group", a define name);
/// `extra_order` is the source order of non-primitive/template type names.
pub fn class_defs(
    present: &BTreeSet<String>,
    element_rules: &HashMap<String, Vec<Decl>>,
    extra_order: &[String],
    synthesizes_shapes: bool,
) -> Vec<Rule> {
    let mut rules = Vec::new();
    let mut emit = |name: &str, bundle: Vec<Decl>, force: bool| {
        if !force && !present.contains(name) {
            return;
        }
        let decls = match element_rules.get(name) {
            Some(extra) => merge_decls(bundle, extra),
            None => bundle,
        };
        if decls.is_empty() {
            return;
        }
        rules.push(class_rule(name, decls));
    };
    for kind in NodeKind::ALL {
        // `line` / `block` are lowered by the chart / sequence engines (gridlines,
        // lifelines, frames, bars) with no source `|line|` / `|block|`, so when the
        // scene runs one of those engines they always carry their primitive defaults —
        // otherwise a lowered shape would inline `fill` / `stroke-width` a class rule
        // should state. A plain scene synthesizes nothing, so it emits neither dead rule.
        let synthesized = synthesizes_shapes && matches!(kind, NodeKind::Line | NodeKind::Block);
        emit(kind.as_str(), primitive_bundle(kind), synthesized);
    }
    for (name, _) in TEMPLATES {
        // `halo` never has desugar-visible instances — its knockout shapes are
        // render-generated mask cuts [SPEC 15.7] — so an authored `|halo|`
        // rule alone forces the class def; the render still emits it only
        // when a crossing actually baked.
        let generated_only = *name == "halo" && element_rules.contains_key(*name);
        emit(name, template_bundle(name), generated_only);
    }
    let mut seen = HashSet::new();
    for name in extra_order {
        if seen.insert(name.as_str()) {
            emit(name, Vec::new(), false);
        }
    }
    // The per-column cell-alignment classes a table distributes [SPEC 8]: each sets
    // one `align`/`justify` keyword, worn by the cells of a start/end-aligned column.
    // Emitted only when actually worn, like any type class.
    for (name, prop, value) in ALIGN_CLASSES {
        if present.contains(name) {
            rules.push(align_class_rule(name, prop, value));
        }
    }
    rules
}

/// The cell-alignment classes and the single declaration each carries.
pub(super) const ALIGN_CLASSES: [(&str, &str, &str); 4] = [
    ("align-start", "align", "start"),
    ("align-end", "align", "end"),
    ("justify-start", "justify", "start"),
    ("justify-end", "justify", "end"),
];

fn align_class_rule(name: &str, prop: &str, value: &str) -> Rule {
    Rule {
        selector: Selector {
            units: vec![SelUnit::Class(lini_class(name))],
        },
        decls: vec![Decl {
            name: prop.to_string(),
            groups: vec![vec![Value::Ident(value.to_string())]],
            span: Span::empty(),
        }],
        span: Span::empty(),
    }
}

/// Merge `extra` decls into `base`: an extra whose property name already exists
/// overrides it **in place** (so the class stays a single clean rule, and
/// re-merging is a fixed point — desugar idempotency); a new property appends.
pub(super) fn merge_decls(mut base: Vec<Decl>, extra: &[Decl]) -> Vec<Decl> {
    for d in extra {
        if let Some(slot) = base.iter_mut().find(|x| x.name == d.name) {
            *slot = d.clone();
        } else {
            base.push(d.clone());
        }
    }
    base
}

/// The engine-supplied note-card compaction [SPEC 8/13/15.7]: a `|note|`
/// inside a `|sequence|` / `|drawing|` scope tightens to drafting proportions
/// (`padding: 6 10; font-size: 13`). Generated descendant rules — `lini
/// desugar` shows the engine's true input — positioned before the user rules,
/// so any user rule of equal specificity wins by source order. A rule the
/// (re-desugared) file already carries is not re-generated: desugar stays a
/// fixed point.
pub fn scoped_note_rules(present: &BTreeSet<String>, user_rules: &[Rule]) -> Vec<Rule> {
    if !present.contains("note") {
        return Vec::new();
    }
    let number = |name: &str, ns: &[f64]| Decl {
        name: name.to_string(),
        groups: vec![ns.iter().map(|n| Value::Number(*n)).collect()],
        span: Span::empty(),
    };
    ["sequence", "drawing"]
        .iter()
        .map(|scope| Rule {
            selector: Selector {
                units: vec![
                    SelUnit::Class(format!("lini-{scope}")),
                    SelUnit::Class("lini-note".to_string()),
                ],
            },
            decls: vec![
                number("padding", &[6.0, 10.0]),
                number("font-size", &[13.0]),
            ],
            span: Span::empty(),
        })
        .filter(|r| match r.selector.units.as_slice() {
            [SelUnit::Class(a), SelUnit::Class(b)] => !has_two_class_rule(user_rules, a, b),
            _ => true,
        })
        .collect()
}

/// Whether `user_rules` already carries a rule whose selector is exactly
/// `.a .b` — the fixed-point guard for generated two-class descendant rules
/// (a re-desugared file is not re-decorated). `SelUnit` has no `PartialEq`
/// to lean on, so the shape is matched by hand.
pub(super) fn has_two_class_rule(user_rules: &[Rule], a: &str, b: &str) -> bool {
    user_rules.iter().any(|u| {
        matches!(u.selector.units.as_slice(),
            [SelUnit::Class(x), SelUnit::Class(y)] if x == a && y == b)
    })
}

fn class_rule(name: &str, decls: Vec<Decl>) -> Rule {
    Rule {
        selector: Selector {
            units: vec![SelUnit::Class(lini_class(name))],
        },
        decls,
        span: Span::empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::ast::Value;

    fn decl(name: &str, v: f64) -> Decl {
        Decl {
            name: name.into(),
            groups: vec![vec![Value::Number(v)]],
            span: Span::empty(),
        }
    }
    fn sel_class(sel: &Selector) -> &str {
        match sel.units.as_slice() {
            [SelUnit::Class(c)] => c,
            _ => "",
        }
    }

    #[test]
    fn worn_chain_is_render_order_derived_to_base_then_primitive() {
        let info = TypeInfo {
            kind: NodeKind::Block,
            chain: vec!["group".into(), "table".into()],
        };
        assert_eq!(
            worn_classes(&info),
            vec!["lini-table", "lini-group", "lini-block"]
        );
    }

    #[test]
    fn reserved_prefix_test() {
        assert!(is_lini_class("lini-box"));
        assert!(!is_lini_class("hot"));
    }

    #[test]
    fn class_def_merges_bundle_then_element_rule() {
        let mut present = BTreeSet::new();
        present.insert("box".to_string());
        let mut el: HashMap<String, Vec<Decl>> = HashMap::new();
        el.insert("box".to_string(), vec![decl("radius", 4.0)]);
        let defs = class_defs(&present, &el, &[], false);
        let boxdef = defs
            .iter()
            .find(|r| sel_class(&r.selector) == "lini-box")
            .expect("box def");
        // element-rule radius:4 comes after the bundle radius:6, so it wins the fold.
        let last_radius = boxdef
            .decls
            .iter()
            .rev()
            .find(|d| d.name == "radius")
            .unwrap();
        assert!(matches!(last_radius.groups[0][0], Value::Number(n) if n == 4.0));
    }

    #[test]
    fn empty_bundle_type_emits_no_rule() {
        let mut present = BTreeSet::new();
        present.insert("image".to_string());
        let defs = class_defs(&present, &HashMap::new(), &[], false);
        assert!(defs.iter().all(|r| sel_class(&r.selector) != "lini-image"));
    }
}
