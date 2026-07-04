//! The stylesheet and cascade [SPEC 4, 4], class-only after desugar.
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
use super::value::resolve_property;
use crate::error::Error;
use crate::expr::FuncTable;
use crate::syntax::ast::{Rule, SelUnit};
use std::collections::HashSet;

/// A rule compiled to its selector units and resolved declarations, retaining
/// source order (its index in [`Stylesheet::rules`]).
struct CompiledRule {
    selector: Vec<SelUnit>,
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
    pub fn build(rules: &[&Rule], vars: &VarTable, funcs: &FuncTable) -> Result<Self, Error> {
        let mut compiled = Vec::with_capacity(rules.len());
        let mut classes = HashSet::new();
        for rule in rules {
            for unit in &rule.selector.units {
                if let SelUnit::Class(c) = unit {
                    classes.insert(c.clone());
                }
            }
            let mut decls = Vec::with_capacity(rule.decls.len());
            for d in &rule.decls {
                decls.push((
                    d.name.clone(),
                    resolve_property(&d.name, &d.groups, d.span, vars, funcs)?,
                ));
            }
            compiled.push(CompiledRule {
                selector: rule.selector.units.clone(),
                decls,
            });
        }
        Ok(Self {
            rules: compiled,
            classes,
        })
    }

    /// Whether `name` is a class any rule references — a node may apply only
    /// these ([SPEC 19] `unknown class`). Generated `.lini-*` classes are always
    /// known and never validated this way; this gates only user classes.
    pub fn defines_class(&self, name: &str) -> bool {
        self.classes.contains(name)
    }

    /// The single-class rule declarations for `class` — the tier-1 lookup for a
    /// worn `.lini-*` type class, merged across rules in source order.
    pub fn class_decls(&self, class: &str) -> Vec<(String, ResolvedValue)> {
        let mut out = Vec::new();
        for rule in &self.rules {
            if let [SelUnit::Class(c)] = rule.selector.as_slice()
                && c == class
            {
                out.extend(rule.decls.iter().cloned());
            }
        }
        out
    }

    /// The descendant (tier 2), user-class (tier 3), then id (tier 4) declaration
    /// layers matching a node, flattened in cascade order — most-specific last
    /// [SPEC 4]. A worn `.lini-*` class is the type tier (1), looked up via
    /// [`Self::class_decls`], and excluded here; the instance's own block (tier 5)
    /// is appended by the caller.
    pub fn node_layers(
        &self,
        ancestors: &[NodeFacts],
        node: &NodeFacts,
    ) -> Vec<(String, ResolvedValue)> {
        let mut out = Vec::new();
        // Tier 2: descendant rules (2+ units), source order.
        for rule in &self.rules {
            if rule.selector.len() > 1 && selector_matches(&rule.selector, ancestors, node) {
                out.extend(rule.decls.iter().cloned());
            }
        }
        // Tier 3: single user-class rules the node wears, definition order.
        for rule in &self.rules {
            if let [SelUnit::Class(c)] = rule.selector.as_slice()
                && !c.starts_with("lini-")
                && node.classes.iter().any(|x| x == c)
            {
                out.extend(rule.decls.iter().cloned());
            }
        }
        // Tier 4: single id rules (`#hero`, `|table#main|`), source order.
        for rule in &self.rules {
            if selector_is_id(&rule.selector) && selector_matches(&rule.selector, ancestors, node) {
                out.extend(rule.decls.iter().cloned());
            }
        }
        out
    }
}

/// A single-unit selector targeting one id — `#hero` or an id-pinned type
/// `|table#main|` — the cascade's id tier [SPEC 4].
fn selector_is_id(sel: &[SelUnit]) -> bool {
    matches!(sel, [SelUnit::Id(_)] | [SelUnit::Type { id: Some(_), .. }])
}

/// The identity a selector matches against: the node's worn classes (the `.lini-*`
/// type chain and its user classes) and its id.
pub struct NodeFacts {
    pub classes: Vec<String>,
    pub id: Option<String>,
}

/// Does `parts` match `node`, given its `ancestors` (root → parent)? The last
/// part must match the node itself; the earlier parts must match a subsequence
/// of the ancestor chain, in order but not necessarily adjacent — exactly the
/// CSS descendant combinator.
pub fn selector_matches(units: &[SelUnit], ancestors: &[NodeFacts], node: &NodeFacts) -> bool {
    let Some((last, prefix)) = units.split_last() else {
        return false;
    };
    if !unit_matches(last, node) {
        return false;
    }
    let mut next = 0;
    for unit in prefix {
        match ancestors[next..].iter().position(|a| unit_matches(unit, a)) {
            Some(offset) => next += offset + 1,
            None => return false,
        }
    }
    true
}

/// Does one selector unit hold on a node? A type unit (post-desugar `name` is the
/// `.lini-*` class) needs the class and, when id-pinned, the id; a class needs the
/// worn class; an id needs the node's id.
fn unit_matches(unit: &SelUnit, facts: &NodeFacts) -> bool {
    match unit {
        SelUnit::Class(c) => facts.classes.iter().any(|x| x == c),
        SelUnit::Id(i) => facts.id.as_deref() == Some(i.as_str()),
        SelUnit::Type { name, id } => {
            facts.classes.iter().any(|x| x == name)
                && id.as_deref().is_none_or(|i| facts.id.as_deref() == Some(i))
        }
        // `|-|` lowers to `.lini-link` in desugar [SPEC 9], so the cascade — which
        // runs on the lowered stylesheet — never sees it.
        SelUnit::Link => unreachable!("'|-|' is lowered to '.lini-link' before resolve"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(classes: &[&str]) -> NodeFacts {
        NodeFacts {
            classes: classes.iter().map(|s| s.to_string()).collect(),
            id: None,
        }
    }
    fn facts_id(classes: &[&str], id: &str) -> NodeFacts {
        NodeFacts {
            classes: classes.iter().map(|s| s.to_string()).collect(),
            id: Some(id.to_string()),
        }
    }
    fn cls(name: &str) -> SelUnit {
        SelUnit::Class(name.into())
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

    #[test]
    fn id_selector_matches_the_node_id() {
        let node = facts_id(&["lini-box"], "hero");
        assert!(selector_matches(&[SelUnit::Id("hero".into())], &[], &node));
        assert!(!selector_matches(
            &[SelUnit::Id("other".into())],
            &[],
            &node
        ));
    }

    #[test]
    fn id_pinned_type_needs_both_class_and_id() {
        // `|table#main|` — a table with id main; lowered name is the lini class.
        let unit = SelUnit::Type {
            name: "lini-table".into(),
            id: Some("main".into()),
        };
        assert!(selector_matches(
            std::slice::from_ref(&unit),
            &[],
            &facts_id(&["lini-table"], "main")
        ));
        assert!(!selector_matches(
            std::slice::from_ref(&unit),
            &[],
            &facts_id(&["lini-table"], "other")
        ));
        assert!(!selector_matches(
            std::slice::from_ref(&unit),
            &[],
            &facts_id(&["lini-box"], "main")
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
        Stylesheet::build(&rules, &VarTable::new(), &FuncTable::new()).expect("build")
    }

    fn names(decls: &[(String, ResolvedValue)]) -> Vec<&str> {
        decls.iter().map(|(n, _)| n.as_str()).collect()
    }

    #[test]
    fn class_decls_returns_the_generated_type_class() {
        // The lowered `.lini-box` carries the box bundle; tier 1 reads it here.
        let s = sheet("|box#x|\n");
        assert!(names(&s.class_decls("lini-box")).contains(&"radius"));
        assert!(s.class_decls("lini-oval").is_empty());
    }

    #[test]
    fn node_layers_order_descendant_then_class_then_id() {
        let s = sheet(
            "{ .hot { stroke: red; }\n|group| |box| { fill: gray; }\n#a { opacity: 0.5; } }\n\
             |group#g| .hot [\n  |box#a| .hot\n]\n",
        );
        let node = facts_id(&["lini-box", "hot"], "a");
        let ancestors = [facts(&["lini-group", "lini-box", "hot"])];
        // descendant `fill` (tier 2), user-class `stroke` (tier 3), then the id
        // `opacity` (tier 4); the worn `.lini-box` is tier 1 and never appears here.
        assert_eq!(
            names(&s.node_layers(&ancestors, &node)),
            vec!["fill", "stroke", "opacity"]
        );
    }

    #[test]
    fn defines_class_covers_every_referenced_user_class() {
        let s = sheet("{ .hot { stroke: red; } }\n|box#x| .hot\n");
        assert!(s.defines_class("hot"));
        assert!(!s.defines_class("cold"));
    }
}
