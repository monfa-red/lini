//! Resolve: a parsed file → a layout-ready [`Program`] [SPEC 18].
//!
//! The work splits by concept: [`value`] maps declaration values into
//! `ResolvedValue`s, [`cascade`] is the stylesheet plus selector matching,
//! [`scene`] the node tree, and [`links`] the link pass (types, templates, and
//! defines were already lowered by desugar, so resolve sees only primitives).
//! [`program`] orchestrates them over the [`defaults`] table; [`merge`] folds
//! resolved declarations and extracts markers; [`ir`] is the resolved form.

pub(crate) mod assets;
mod cascade;
mod defaults;
mod ir;
mod links;
mod merge;
mod program;
pub(crate) mod scene;
pub(crate) mod value;

pub use assets::AssetEnv;
pub use defaults::built_in_defaults;
pub use ir::*;
#[cfg_attr(not(test), allow(unused_imports))]
pub use program::resolve as resolve_with_theme;
pub use program::resolve_with_env;
