//! `.lini-*` class generation and the reserved-prefix split. A type chain becomes
//! worn classes in **render order** — derived→base, then the primitive last —
//! matching the pre-desugar `lini-shape-*` class order so the SVG class attribute
//! is byte-identical; the tier-1 cascade folds them in reverse (primitive first,
//! derived last wins). Each present type name gets one
//! `.lini-<name> { bundle + element-rule decls }` stylesheet rule.

use super::bundles::{primitive_bundle, template_bundle};
use super::types::{TEMPLATES, TypeInfo};
use crate::resolve::NodeKind;
use crate::span::Span;
use crate::syntax::ast::{Decl, Rule, SelPart, Selector};
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
) -> Vec<Rule> {
    let mut rules = Vec::new();
    let mut emit = |name: &str, bundle: Vec<Decl>| {
        if !present.contains(name) {
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
        emit(kind.as_str(), primitive_bundle(kind));
    }
    for (name, _) in TEMPLATES {
        emit(name, template_bundle(name));
    }
    let mut seen = HashSet::new();
    for name in extra_order {
        if seen.insert(name.as_str()) {
            emit(name, Vec::new());
        }
    }
    rules
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

fn class_rule(name: &str, decls: Vec<Decl>) -> Rule {
    Rule {
        selector: Selector {
            parts: vec![SelPart::Class(lini_class(name))],
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
        match sel.parts.as_slice() {
            [SelPart::Class(c)] => c,
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
        let defs = class_defs(&present, &el, &[]);
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
        let defs = class_defs(&present, &HashMap::new(), &[]);
        assert!(defs.iter().all(|r| sel_class(&r.selector) != "lini-image"));
    }
}
