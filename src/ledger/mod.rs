//! The property ledger — the single source of truth for Lini's property
//! surface [AUDIT D1]. Three files, pure data: [`properties`] (one row per
//! property: owners, value shape, default ref, inheritance, gate),
//! [`defaults`] (every built-in default bundle — the one place the look is
//! tuned), and [`consts`] (the shared chrome constants, filled in Stage R3).
//!
//! Consumers, in order: the resolve classifiers, the 0.21 validation pass,
//! schema generation, fmt, and generated SPEC tables. The `.get()`/`.number()`
//! read-sites stay direct — the ledger describes and validates; reads never
//! hop through it.

pub(crate) mod consts;
pub(crate) mod defaults;
pub(crate) mod properties;
