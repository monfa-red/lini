//! Desugar: lower all surface sugar (types, templates, defines, element/descendant
//! rules, labels, scene defaults) to primitive shapes + `.lini-*` classes, so the
//! core only ever sees primitives. Design:
//! `docs/superpowers/specs/2026-06-20-desugar-to-primitives-design.md`.
//!
//! Built in stages. [`bundles`] holds every built-in default as AST decls; the
//! full lowering (assembled here) is added incrementally. For now [`labels`] still
//! owns the entry point (id-as-label + auto-`along:` only — types stay as written).

mod bundles;
mod labels;
mod types;

pub use labels::desugar;
