//! Resolve: a parsed file → a layout-ready [`Program`] [SPEC 19].
//!
//! The work splits by concept: [`value`] maps declaration values into
//! `ResolvedValue`s, [`cascade`] is the stylesheet plus selector matching,
//! [`scene`] the node tree, and [`links`] the link pass (types, templates, and
//! defines were already lowered by desugar, so resolve sees only primitives).
//! [`program`] orchestrates them over the [`defaults`] table; [`merge`] folds
//! resolved declarations and extracts markers; [`ir`] is the resolved form.
//! [`pattern`] states the `pattern:` call's law once, for value resolution
//! and layout alike; [`tracks`] reads a grid's track list and [`tables`] runs
//! the table / entity structure that its **resolved** column count decides
//! [SPEC 8].

pub(crate) mod assets;
mod cascade;
pub(crate) mod defaults;
mod ir;
mod links;
mod merge;
pub(crate) mod pattern;
mod program;
pub(crate) mod scene;
mod tables;
pub(crate) mod tracks;
pub(crate) mod value;

pub use assets::AssetEnv;
pub use defaults::built_in_defaults;
pub use ir::*;
#[cfg_attr(not(test), allow(unused_imports))]
pub use program::resolve as resolve_with_theme;
pub use program::resolve_with_env;
