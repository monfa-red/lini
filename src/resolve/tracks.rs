//! Grid track lists [SPEC 12] — the one reading of a resolved `columns:` /
//! `rows:` value.
//!
//! The list **length is the column count**, and that count decides more than
//! placement: a `|table|`'s auto-header row, its per-column alignment, and an
//! `|entity|`'s full-width title all read it ([SPEC 8], `super::tables`). So it
//! is parsed once, here, from the value the cascade resolved — never counted a
//! second time from the source text, which cannot see a class rule or a folded
//! expression.

use super::ir::ResolvedValue;
use crate::error::Error;
use crate::span::Span;

/// One track: an explicit size, or `auto` (sized to its widest / tallest child).
#[derive(Clone, Copy)]
pub enum Track {
    Fixed(f64),
    Auto,
}

/// A resolved track-list value as its tracks, `repeat(N[, size])` expanded.
pub fn parse(value: &ResolvedValue, span: Span) -> Result<Vec<Track>, Error> {
    let mut out = Vec::new();
    match value {
        // The comma law [SPEC 2]: a track list is comma-separated.
        ResolvedValue::List(items) => {
            for item in items {
                push(&mut out, item, span)?;
            }
        }
        single => push(&mut out, single, span)?,
    }
    Ok(out)
}

fn push(out: &mut Vec<Track>, v: &ResolvedValue, span: Span) -> Result<(), Error> {
    match v {
        ResolvedValue::Ident(s) if s == "auto" => out.push(Track::Auto),
        ResolvedValue::Call(c) if c.name == "repeat" => {
            let n = c
                .args
                .first()
                .and_then(ResolvedValue::as_number)
                .filter(|n| *n >= 1.0 && n.fract() == 0.0)
                .ok_or_else(|| Error::at(span, "repeat() needs a positive integer count"))?
                as usize;
            let size = c.args.get(1).and_then(ResolvedValue::as_number);
            for _ in 0..n {
                out.push(size.map_or(Track::Auto, Track::Fixed));
            }
        }
        other => match other.as_number() {
            Some(n) => out.push(Track::Fixed(n)),
            None => {
                return Err(Error::at(
                    span,
                    "a track is a size, 'auto', or repeat(N[, size])",
                ));
            }
        },
    }
    Ok(())
}
