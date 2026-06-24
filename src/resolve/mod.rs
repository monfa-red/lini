//! Resolve: a parsed file → a layout-ready [`Program`] (SPEC §17).
//!
//! The work splits by concept: [`value`] maps declaration values into
//! `ResolvedValue`s, [`cascade`] is the stylesheet plus selector matching,
//! [`types`] the define/template/primitive chain, [`scene`] the node tree, and
//! [`links`] the link pass. [`program`] orchestrates them over the [`defaults`]
//! table; [`merge`] folds resolved declarations and extracts markers.

mod cascade;
mod defaults;
mod ir;
mod links;
mod merge;
mod program;
mod scene;
mod value;

pub use defaults::built_in_defaults;
pub use ir::*;
pub use program::resolve as resolve_with_theme;
