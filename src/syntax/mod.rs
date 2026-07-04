//! The syntax front end [SPEC 20]: a token stream parsed into the [`ast`] by
//! the recursive-descent [`parser`]. `resolve` consumes the resulting `File`.

pub mod ast;
pub mod parser;
