//! v4 syntax front end (PLAN Phase 2), built alongside the v3 front end. The v3
//! pipeline still drives compilation; Phase 3 cuts `resolve` over to this and
//! removes the v3 front end. Until then this module is exercised only by its own
//! unit tests, so its as-yet-unconsumed items are allowed dead code.
#![allow(dead_code)]

pub mod ast;
pub mod parser;
