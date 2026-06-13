use super::ir::{ResolvedAttr, VarTable};
use super::vars::resolve_value;
use crate::ast::{AttrItem, StyleDef};
use crate::error::Error;
use crate::span::Span;
use std::collections::HashMap;

/// Table of expanded styles. Each style maps to its fully-expanded ordered list
/// of attrs (style refs are flattened out, applied in source order) plus its
/// defs-block definition index — per-node application merges in that order,
/// like CSS classes (SPEC §13).
pub struct StyleTable {
    expanded: HashMap<String, (usize, Vec<ResolvedAttr>)>,
}

impl StyleTable {
    pub fn build(defs: &[&StyleDef], vars: &VarTable) -> Result<Self, Error> {
        // First pass: index by name; reject duplicates and reserved names.
        let mut by_name: HashMap<&str, &StyleDef> = HashMap::new();
        for def in defs {
            if super::is_reserved(&def.name) {
                return Err(super::reserved_error(def.span, &def.name));
            }
            if by_name.insert(def.name.as_str(), def).is_some() {
                return Err(Error::at(
                    def.span,
                    format!("duplicate style '{}'", def.name),
                ));
            }
        }

        // Second pass: expand each style with cycle detection. Results are
        // cached so a style referenced multiple times only expands once.
        let mut cache: HashMap<String, Vec<ResolvedAttr>> = HashMap::new();
        for def in defs {
            let mut visiting: Vec<String> = Vec::new();
            expand_style(
                &def.name,
                &by_name,
                &mut cache,
                &mut visiting,
                def.span,
                vars,
            )?;
        }

        let expanded = defs
            .iter()
            .enumerate()
            .map(|(i, def)| (def.name.clone(), (i, cache.remove(&def.name).unwrap())))
            .collect();
        Ok(Self { expanded })
    }

    pub fn lookup(&self, name: &str) -> Option<&[ResolvedAttr]> {
        self.expanded.get(name).map(|(_, attrs)| attrs.as_slice())
    }

    /// Defs-block definition index — the per-node merge order (SPEC §13).
    pub fn index(&self, name: &str) -> Option<usize> {
        self.expanded.get(name).map(|(i, _)| *i)
    }

    /// Every style in definition order — the output stylesheet's rule order.
    pub fn in_order(&self) -> Vec<(String, Vec<ResolvedAttr>)> {
        let mut all: Vec<(usize, String, Vec<ResolvedAttr>)> = self
            .expanded
            .iter()
            .map(|(name, (i, attrs))| (*i, name.clone(), attrs.clone()))
            .collect();
        all.sort_by_key(|(i, ..)| *i);
        all.into_iter().map(|(_, n, a)| (n, a)).collect()
    }
}

fn expand_style(
    name: &str,
    by_name: &HashMap<&str, &StyleDef>,
    expanded: &mut HashMap<String, Vec<ResolvedAttr>>,
    visiting: &mut Vec<String>,
    use_span: Span,
    vars: &VarTable,
) -> Result<Vec<ResolvedAttr>, Error> {
    if let Some(cached) = expanded.get(name) {
        return Ok(cached.clone());
    }
    if visiting.iter().any(|n| n == name) {
        let chain = format!("{} -> {}", visiting.join(" -> "), name);
        return Err(Error::at(use_span, format!("cycle in style '{}'", chain)));
    }

    let def = by_name
        .get(name)
        .ok_or_else(|| Error::at(use_span, format!("unknown style '.{}'", name)))?;

    visiting.push(name.to_string());
    let mut result: Vec<ResolvedAttr> = Vec::new();
    for item in &def.items {
        match item {
            AttrItem::Attr(a) => {
                result.push(ResolvedAttr {
                    name: a.name.clone(),
                    value: resolve_value(&a.value, vars)?,
                    span: a.span,
                });
            }
            AttrItem::Style(s) => {
                let inner = expand_style(&s.name, by_name, expanded, visiting, s.span, vars)?;
                result.extend(inner);
            }
        }
    }
    visiting.pop();

    expanded.insert(name.to_string(), result.clone());
    Ok(result)
}
