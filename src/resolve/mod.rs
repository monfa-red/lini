//! Resolve: a parsed file → a layout-ready [`Program`] (SPEC §17).
//!
//! The work splits by concept: [`value`] maps declaration values into
//! `ResolvedValue`s, [`cascade`] is the stylesheet plus selector matching,
//! [`types`] the define/template/primitive chain, [`scene`] the node tree, and
//! [`wires`] the wire pass. [`program`] orchestrates them over the [`defaults`]
//! table; [`merge`] folds resolved declarations and extracts markers.

mod cascade;
mod defaults;
mod ir;
mod merge;
mod program;
mod scene;
mod types;
mod value;
mod wires;

pub use ir::*;
pub use program::resolve as resolve_with_theme;

use crate::syntax::ast::{File, StyleItem};
use std::collections::HashMap;

/// Whether `target` appears in `ty`'s base chain — `ty` itself, the templates
/// it builds on, or the `name::base` defines, down to the primitive. `desugar`
/// uses this to classify a type without a full resolve: `group` (its labels
/// become `|caption|` children) and `text` / `icon` (which carry their own
/// label, so it is never re-expanded). Bounded by the inheritance-depth ceiling,
/// so a cyclic define can't loop.
pub fn type_chain_contains(ty: &str, target: &str, file: &File) -> bool {
    let defines: HashMap<&str, &str> = file
        .stylesheet
        .iter()
        .filter_map(|it| match it {
            StyleItem::Define(d) => Some((d.name.as_str(), d.base.as_str())),
            _ => None,
        })
        .collect();
    let mut name = ty.to_string();
    for _ in 0..=16 {
        if name == target {
            return true;
        }
        match types::template_base(&name).or_else(|| defines.get(name.as_str()).copied()) {
            Some(base) => name = base.to_string(),
            None => return false,
        }
    }
    false
}
