//! Lint pass — stylistic warnings that are not parse/resolve errors.
//!
//! v4 deliberately makes inline paint idiomatic (an instance carries its own
//! declarations in its block), so the v3 "visual attrs belong in a style def"
//! lint is gone. The "did you mean" property-name hint (SPEC §19) is deferred.
//! The pass is intentionally empty for now — the home for future lints, kept so
//! the `--no-warn` / `--strict` machinery has somewhere to read from.

use crate::error::Diagnostic;
use crate::syntax::ast::File;

pub fn lint(_file: &File) -> Vec<Diagnostic> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    /// Inline paint in an instance block is idiomatic v4 — no warning.
    #[test]
    fn inline_paint_is_not_linted() {
        let warns =
            crate::lint_str("x |box| { fill: red; stroke: blue; }\n").expect("lint");
        assert!(warns.is_empty(), "{warns:?}");
    }
}
