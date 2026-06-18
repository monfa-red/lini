//! The stylesheet and cascade (SPEC §4, §12).
//!
//! A rule is `selector { decls }`. Selectors are CSS-shaped: a single element
//! (`box`), a single class (`.hot`), or a whitespace-separated descendant
//! chain (`table box`, `.sidebar box`). The cascade layers, most-specific
//! last:
//!
//! 1. the **type cascade** — element rules + define defaults, base→derived
//!    (built in [`super::types`], which reads element rules from here),
//! 2. **descendant rules** matched against the node's ancestor chain,
//! 3. **class rules** for the node's applied classes, in definition order,
//! 4. the **instance's own block**.
//!
//! Ties within a layer break by source order; later layers win. This module
//! owns rule compilation and matching; the merge into an `AttrMap` is the
//! caller's (it interleaves the type cascade and the block, which live
//! elsewhere).

use super::ir::{ResolvedValue, VarTable};
use super::value::resolve_groups;
use crate::error::Error;
use crate::syntax::ast::{Rule, SelPart};
use std::collections::HashSet;

/// A rule compiled to its selector parts and resolved declarations, retaining
/// source order (its index in [`Stylesheet::rules`]).
struct CompiledRule {
    selector: Vec<SelPart>,
    decls: Vec<(String, ResolvedValue)>,
}

/// The whole-file stylesheet — every `selector { … }` rule, in source order.
pub struct Stylesheet {
    rules: Vec<CompiledRule>,
    classes: HashSet<String>,
}

impl Stylesheet {
    /// Compile the file's rules: resolve each rule's declarations against the
    /// variable table and record its selector. Source order is preserved as the
    /// vector order — every cascade tie breaks on it.
    pub fn build(rules: &[&Rule], vars: &VarTable) -> Result<Self, Error> {
        let mut compiled = Vec::with_capacity(rules.len());
        let mut classes = HashSet::new();
        for rule in rules {
            for part in &rule.selector.parts {
                if let SelPart::Class(c) = part {
                    classes.insert(c.clone());
                }
            }
            let mut decls = Vec::with_capacity(rule.decls.len());
            for d in &rule.decls {
                decls.push((d.name.clone(), resolve_groups(&d.groups, d.span, vars)?));
            }
            compiled.push(CompiledRule {
                selector: rule.selector.parts.clone(),
                decls,
            });
        }
        Ok(Self {
            rules: compiled,
            classes,
        })
    }

    /// Whether `name` is a class any rule references — a node may apply only
    /// these (SPEC §15 `unknown class`).
    pub fn defines_class(&self, name: &str) -> bool {
        self.classes.contains(name)
    }

    /// Every type name a selector references, for the orchestrator's
    /// known-type validation (SPEC §17 step 1).
    pub fn referenced_types(&self) -> Vec<&str> {
        let mut out = Vec::new();
        for rule in &self.rules {
            for part in &rule.selector {
                if let SelPart::Type(t) = part {
                    out.push(t.as_str());
                }
            }
        }
        out
    }

    /// The element rule (`type { … }`, single type part) declarations for one
    /// type, in source order and merged across rules — the type cascade's
    /// per-type layer (SPEC §12.1). `wire { … }` reaches its defaults this way.
    pub fn element_decls(&self, type_name: &str) -> Vec<(String, ResolvedValue)> {
        let mut out = Vec::new();
        for rule in &self.rules {
            if let [SelPart::Type(t)] = rule.selector.as_slice()
                && t == type_name
            {
                out.extend(rule.decls.iter().cloned());
            }
        }
        out
    }

    /// The descendant (tier 2) then class (tier 3) declaration layers matching a
    /// node, flattened in cascade order — descendant rules first, then single
    /// class rules, each set in source order (SPEC §12). Single element rules
    /// are excluded: they belong to the type cascade.
    pub fn node_layers(
        &self,
        ancestors: &[NodeFacts],
        node: &NodeFacts,
    ) -> Vec<(String, ResolvedValue)> {
        let mut out = Vec::new();
        // Tier 2: descendant rules (2+ parts), source order.
        for rule in &self.rules {
            if rule.selector.len() > 1 && selector_matches(&rule.selector, ancestors, node) {
                out.extend(rule.decls.iter().cloned());
            }
        }
        // Tier 3: single class rules the node carries, definition order.
        for rule in &self.rules {
            if let [SelPart::Class(c)] = rule.selector.as_slice()
                && node.classes.iter().any(|x| x == c)
            {
                out.extend(rule.decls.iter().cloned());
            }
        }
        out
    }
}

/// The identity a selector matches against: every type name in the node's
/// chain (its declared type back to the primitive) plus its applied classes.
pub struct NodeFacts {
    pub types: Vec<String>,
    pub classes: Vec<String>,
}

/// Does `parts` match `node`, given its `ancestors` (root → parent)? The last
/// part must match the node itself; the earlier parts must match a subsequence
/// of the ancestor chain, in order but not necessarily adjacent — exactly the
/// CSS descendant combinator.
pub fn selector_matches(parts: &[SelPart], ancestors: &[NodeFacts], node: &NodeFacts) -> bool {
    let Some((last, prefix)) = parts.split_last() else {
        return false;
    };
    if !part_matches(last, node) {
        return false;
    }
    // The prefix matches an ordered subsequence of the ancestor chain.
    let mut next = 0;
    for part in prefix {
        match ancestors[next..].iter().position(|a| part_matches(part, a)) {
            Some(offset) => next += offset + 1,
            None => return false,
        }
    }
    true
}

fn part_matches(part: &SelPart, facts: &NodeFacts) -> bool {
    match part {
        SelPart::Type(t) => facts.types.iter().any(|x| x == t),
        SelPart::Class(c) => facts.classes.iter().any(|x| x == c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(types: &[&str], classes: &[&str]) -> NodeFacts {
        NodeFacts {
            types: types.iter().map(|s| s.to_string()).collect(),
            classes: classes.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn ty(name: &str) -> SelPart {
        SelPart::Type(name.into())
    }
    fn cls(name: &str) -> SelPart {
        SelPart::Class(name.into())
    }

    #[test]
    fn element_selector_matches_a_type_in_the_chain() {
        // `treat` resolves to a box, so a `box {}` rule still matches it.
        let node = facts(&["treat", "box"], &[]);
        assert!(selector_matches(&[ty("box")], &[], &node));
        assert!(selector_matches(&[ty("treat")], &[], &node));
        assert!(!selector_matches(&[ty("oval")], &[], &node));
    }

    #[test]
    fn class_selector_matches_an_applied_class() {
        let node = facts(&["box"], &["hot"]);
        assert!(selector_matches(&[cls("hot")], &[], &node));
        assert!(!selector_matches(&[cls("cold")], &[], &node));
    }

    #[test]
    fn descendant_selector_needs_a_matching_ancestor() {
        // `table box` — a box with a table ancestor.
        let node = facts(&["box"], &[]);
        let ancestors = [facts(&["table", "group", "box"], &[]), facts(&["row"], &[])];
        assert!(selector_matches(
            &[ty("table"), ty("box")],
            &ancestors,
            &node
        ));
    }

    #[test]
    fn descendant_selector_fails_without_the_ancestor() {
        let node = facts(&["box"], &[]);
        let ancestors = [facts(&["group", "box"], &[])];
        assert!(!selector_matches(
            &[ty("table"), ty("box")],
            &ancestors,
            &node
        ));
    }

    #[test]
    fn descendant_combinator_is_not_adjacency() {
        // `.sidebar box` matches a box even with an intervening container.
        let node = facts(&["box"], &[]);
        let ancestors = [facts(&["group", "box"], &["sidebar"]), facts(&["row"], &[])];
        assert!(selector_matches(
            &[cls("sidebar"), ty("box")],
            &ancestors,
            &node
        ));
    }

    #[test]
    fn descendant_prefix_order_is_enforced() {
        // `a c` requires an `a` ancestor *before* the node — order along the
        // chain matters; the parts match as an ordered subsequence.
        let node = facts(&["c"], &[]);
        let good = [facts(&["a"], &[]), facts(&["b"], &[])];
        assert!(selector_matches(&[ty("a"), ty("c")], &good, &node));
        // `b a` (b then a) cannot match ancestors ordered [a, b].
        assert!(!selector_matches(
            &[ty("b"), ty("a"), ty("c")],
            &good,
            &node
        ));
    }

    #[test]
    fn last_part_must_match_the_node_not_an_ancestor() {
        let node = facts(&["box"], &[]);
        let ancestors = [facts(&["table"], &[])];
        // `box table` — last part `table` must be the node, but the node is a box.
        assert!(!selector_matches(
            &[ty("box"), ty("table")],
            &ancestors,
            &node
        ));
    }

    fn sheet(src: &str) -> Stylesheet {
        use crate::syntax::ast::{Rule, StyleItem};
        let toks = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(&toks).expect("parse");
        let rules: Vec<&Rule> = file
            .stylesheet
            .iter()
            .filter_map(|it| match it {
                StyleItem::Rule(r) => Some(r),
                _ => None,
            })
            .collect();
        Stylesheet::build(&rules, &VarTable::new()).expect("build")
    }

    fn names(decls: &[(String, ResolvedValue)]) -> Vec<&str> {
        decls.iter().map(|(n, _)| n.as_str()).collect()
    }

    #[test]
    fn element_decls_merge_matching_rules_in_source_order() {
        let s = sheet("box { fill: red; }\nbox { stroke: blue; }\n");
        assert_eq!(names(&s.element_decls("box")), vec!["fill", "stroke"]);
        assert!(s.element_decls("oval").is_empty());
    }

    #[test]
    fn node_layers_put_descendant_rules_before_class_rules() {
        // Source order is class-then-descendant; the cascade still applies the
        // descendant first (tier 2), the class last (tier 3).
        let s = sheet(".hot { stroke: red; }\ntable box { fill: gray; }\n");
        let node = facts(&["box"], &["hot"]);
        let ancestors = [facts(&["table", "group", "box"], &[])];
        assert_eq!(
            names(&s.node_layers(&ancestors, &node)),
            vec!["fill", "stroke"]
        );
    }

    #[test]
    fn class_layers_follow_definition_order_not_application_order() {
        // Applied `.b .a`, but defined `.a` then `.b`; `.b` is later in source
        // so it wins the tie.
        let s = sheet(".a { fill: red; }\n.b { fill: blue; }\n");
        let node = facts(&["box"], &["b", "a"]);
        let layers = s.node_layers(&[], &node);
        assert_eq!(layers.len(), 2);
        assert!(matches!(&layers.last().unwrap().1, ResolvedValue::Ident(s) if s == "blue"));
    }

    #[test]
    fn defines_class_covers_every_referenced_class() {
        let s = sheet(".hot { stroke: red; }\n.sidebar box { fill: gray; }\n");
        assert!(s.defines_class("hot"));
        assert!(s.defines_class("sidebar"));
        assert!(!s.defines_class("cold"));
    }
}
