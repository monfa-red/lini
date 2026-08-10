//! The one compile pipeline the in-crate suites drive.
//!
//! Every `#[cfg(test)]` module that needs a lowered file, a resolved program,
//! a laid-out scene or the error one of those stages raises calls in here —
//! the four-line `lex → parse → desugar → resolve` incantation is written
//! once. Each stage has a fallible form (for the suites that assert on the
//! error) and an unwrapping form (for the suites that assert on the output);
//! the `_in_samples` pair resolves image assets against `samples/` [SPEC 7],
//! which the drawing sweeps need and inline sources never notice.

use crate::error::Error;
use crate::layout::LaidOut;
use crate::resolve::{AssetEnv, Program};
use crate::syntax::ast::File;

pub use crate::testing::{Pred, all_placed, find_placed, placed_by_id, placed_by_type};

/// `lex → parse → desugar`. The stage the sugar tests and the desugar-time
/// diagnostics stop at.
pub fn try_lowered(src: &str) -> Result<File, Error> {
    let toks = crate::lexer::lex(src)?;
    let file = crate::syntax::parser::parse(src, &toks)?;
    crate::desugar::desugar(&file)
}

pub fn lowered(src: &str) -> File {
    try_lowered(src).expect("desugar")
}

/// The message desugar raises, for the sugar diagnostics.
#[track_caller]
pub fn desugar_err(src: &str) -> String {
    match try_lowered(src) {
        Ok(_) => panic!("expected a desugar error for {src:?}"),
        Err(e) => e.message,
    }
}

/// … `→ resolve`, with no theme and no asset base.
pub fn try_program(src: &str) -> Result<Program, Error> {
    crate::resolve::resolve_with_theme(&try_lowered(src)?, &[])
}

pub fn program(src: &str) -> Program {
    try_program(src).expect("resolve")
}

/// The message the front half raises. Unknown types and define cycles surface
/// in desugar, cascade and reference faults in resolve; a caller asserting on
/// a resolve diagnostic should not have to know which.
#[track_caller]
pub fn resolve_err(src: &str) -> String {
    match try_program(src) {
        Ok(_) => panic!("expected a resolve error for {src:?}"),
        Err(e) => e.message,
    }
}

/// … `→ layout`.
pub fn try_laid(src: &str) -> Result<LaidOut, Error> {
    crate::layout::layout(&try_program(src)?)
}

pub fn laid(src: &str) -> LaidOut {
    try_laid(src).expect("layout")
}

#[track_caller]
pub fn layout_err(src: &str) -> String {
    match try_laid(src) {
        Ok(_) => panic!("expected a layout error for {src:?}"),
        Err(e) => e.message,
    }
}

/// [`program`] with `samples/` as the asset base [SPEC 7], so a suite may
/// compile a sheet that embeds a committed image.
pub fn program_in_samples(src: &str) -> Program {
    let env = AssetEnv {
        base_dir: Some("samples".into()),
        root: None,
    };
    crate::resolve::resolve_with_env(&lowered(src), &[], env).expect("resolve")
}

/// [`laid`] with `samples/` as the asset base [SPEC 7].
pub fn laid_in_samples(src: &str) -> LaidOut {
    crate::layout::layout(&program_in_samples(src)).expect("layout")
}

#[track_caller]
pub fn layout_err_in_samples(src: &str) -> String {
    match crate::layout::layout(&program_in_samples(src)) {
        Ok(_) => panic!("expected a layout error for {src:?}"),
        Err(e) => e.message,
    }
}
