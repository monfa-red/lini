//! The stylesheet and cascade (SPEC §4, §12), class-only after desugar.
//!
//! A rule is `selector { decls }`. After desugar every type/template/define is a
//! generated `.lini-*` class, so selectors carry only classes: a single class
//! (`.hot`, `.lini-box`) or a descendant chain (`.lini-table .lini-box`,
//! `.sidebar .lini-box`). The cascade layers, most-specific last:
//!
//! 1. the **type tier** — a node's worn `.lini-*` classes, folded base→derived
//!    (the caller looks each up via [`Stylesheet::class_decls`]),
//! 2. **descendant rules** matched against the node's ancestor chain,
//! 3. **user class rules** for the node's non-`lini-` classes, in definition order,
//! 4. the **instance's own block**.
//!
//! Ties within a layer break by source order; later layers win. This module owns
//! rule compilation and matching; folding into an `AttrMap` is the caller's.

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
    /// these (SPEC §15 `unknown class`). Generated `.lini-*` classes are always
    /// known and never validated this way; this gates only user classes.
    pub fn defines_class(&self, name: &str) -> bool {
        self.classes.contains(name)
    }

    /// The single-class rule declarations for `class` — the tier-1 lookup for a
    /// worn `.lini-*` type class, merged across rules in source order.
    pub fn class_decls(&self, class: &str) -> Vec<(String, ResolvedValue)> {
        let mut out = Vec::new();
        for rule in &self.rules {
            if let [SelPart::Class(c)] = rule.selector.as_slice()
                && c == class
            {
                out.extend(rule.decls.iter().cloned());
            }
        }
        out
    }

    /// The descendant (tier 2) then user-class (tier 3) declaration layers matching
    /// a node, flattened in cascade order — descendant rules first, then single
    /// **non-`lini-`** class rules, each in source order (SPEC §12). A worn `.lini-*`
    /// class is the type tier (1) and is excluded here; descendant rules over
    /// `.lini-*` classes still match via the ancestor/node facts.
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
        // Tier 3: single user-class rules the node carries, definition order.
        for rule in &self.rules {
            if let [SelPart::Class(c)] = rule.selector.as_slice()
                && !c.starts_with("lini-")
                && node.classes.iter().any(|x| x == c)
            {
                out.extend(rule.decls.iter().cloned());
            }
        }
        out
    }
}

/// The identity a selector matches against: the node's worn classes (the `.lini-*`
/// type chain and its user classes).
pub struct NodeFacts {
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
        SelPart::Class(c) => facts.classes.iter().any(|x| x == c),
        // Post-desugar selectors are class-only; a bare type part never matches.
        SelPart::Type(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(classes: &[&str]) -> NodeFacts {
        NodeFacts {
            classes: classes.iter().map(|s| s.to_string()).collect(),
        }
    }
    fn cls(name: &str) -> SelPart {
        SelPart::Class(name.into())
    }

    #[test]
    fn class_selector_matches_a_worn_class() {
        let node = facts(&["lini-box", "hot"]);
        assert!(selector_matches(&[cls("lini-box")], &[], &node));
        assert!(selector_matches(&[cls("hot")], &[], &node));
        assert!(!selector_matches(&[cls("cold")], &[], &node));
    }

    #[test]
    fn descendant_selector_needs_a_matching_ancestor() {
        // `.lini-table .lini-box` — a box with a table ancestor.
        let node = facts(&["lini-box"]);
        let ancestors = [
            facts(&["lini-table", "lini-group", "lini-box"]),
            facts(&["lini-row"]),
        ];
        assert!(selector_matches(
            &[cls("lini-table"), cls("lini-box")],
            &ancestors,
            &node
        ));
    }

    #[test]
    fn descendant_selector_fails_without_the_ancestor() {
        let node = facts(&["lini-box"]);
        let ancestors = [facts(&["lini-group", "lini-box"])];
        assert!(!selector_matches(
            &[cls("lini-table"), cls("lini-box")],
            &ancestors,
            &node
        ));
    }

    #[test]
    fn descendant_combinator_is_not_adjacency() {
        // `.sidebar .lini-box` matches even with an intervening container.
        let node = facts(&["lini-box"]);
        let ancestors = [
            facts(&["lini-group", "lini-box", "sidebar"]),
            facts(&["lini-row"]),
        ];
        assert!(selector_matches(
            &[cls("sidebar"), cls("lini-box")],
            &ancestors,
            &node
        ));
    }

    fn sheet(src: &str) -> Stylesheet {
        use crate::syntax::ast::{Rule, StyleItem};
        let toks = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(&toks).expect("parse");
        let lowered = crate::desugar::desugar(&file).expect("desugar");
        let rules: Vec<&Rule> = lowered
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
    fn class_decls_returns_the_generated_type_class() {
        // The lowered `.lini-box` carries the box bundle; tier 1 reads it here.
        let s = sheet("x |box|\n");
        assert!(names(&s.class_decls("lini-box")).contains(&"radius"));
        assert!(s.class_decls("lini-oval").is_empty());
    }

    #[test]
    fn node_layers_put_descendant_rules_before_class_rules_and_skip_lini() {
        let s = sheet(
            "{ .hot { stroke: red; }\n|group box| { fill: gray; } }\ng |group| .hot [\n  a |box| .hot\n]\n",
        );
        let node = facts(&["lini-box", "hot"]);
        let ancestors = [facts(&["lini-group", "lini-box", "hot"])];
        // descendant `fill` (tier 2) before the user-class `stroke` (tier 3); the
        // worn `.lini-box` is tier 1 and never appears here.
        assert_eq!(
            names(&s.node_layers(&ancestors, &node)),
            vec!["fill", "stroke"]
        );
    }

    #[test]
    fn defines_class_covers_every_referenced_user_class() {
        let s = sheet("{ .hot { stroke: red; } }\nx |box| .hot\n");
        assert!(s.defines_class("hot"));
        assert!(!s.defines_class("cold"));
    }
}
