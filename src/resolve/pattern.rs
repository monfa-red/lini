//! The `pattern:` call's one law [SPEC 15.4]: `grid(cols, rows, dx, dy)` and
//! `radial(count, radius)` — the two names, their **arities**, and their
//! ranges, read once into a typed [`Pattern`]. Resolve reads it to reject a
//! malformed call at the declaration; layout places the copies from it and a
//! dimension prefixes its `N×` count from it. Nobody re-derives an argument
//! out of the raw call, so an extra argument can't slip past one reader
//! because another didn't look.

use super::ir::{ResolvedCall, ResolvedValue};
use crate::error::Error;
use crate::span::Span;

/// A node's replication [SPEC 15.4]. The two datums match drafting practice —
/// a grid is located by its first hole, a bolt circle by its centre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pattern {
    /// `cols × rows` copies at offsets `(i·dx, j·dy)` — the **seed is copy
    /// one** and keeps the node's position.
    Grid {
        cols: usize,
        rows: usize,
        dx: f64,
        dy: f64,
    },
    /// `count` copies **on** the circle, first at bearing 0, clockwise — the
    /// node's position is the ring centre and no copy is drawn there.
    Radial { count: usize, radius: f64 },
}

/// The one usage error: a `pattern:` that is not one well-formed call.
pub fn usage(span: Span) -> Error {
    Error::at(
        span,
        "'pattern' takes grid(cols, rows, dx, dy) or radial(count, radius)",
    )
}

impl Pattern {
    /// Read a resolved call — the name picks the form, the match arm carries
    /// the **exact** arity, and the ranges are the spec's [SPEC 15.4].
    pub fn read(call: &ResolvedCall, span: Span) -> Result<Pattern, Error> {
        let args: Option<Vec<f64>> = call.args.iter().map(ResolvedValue::as_number).collect();
        let args = args.ok_or_else(|| usage(span))?;
        match (call.name.as_str(), args.as_slice()) {
            ("grid", &[cols, rows, dx, dy]) => {
                if cols < 1.0 || rows < 1.0 {
                    return Err(Error::at(span, "'grid' needs cols ≥ 1 and rows ≥ 1"));
                }
                Ok(Pattern::Grid {
                    cols: cols as usize,
                    rows: rows as usize,
                    dx,
                    dy,
                })
            }
            ("radial", &[count, radius]) => {
                if count < 2.0 || radius <= 0.0 {
                    return Err(Error::at(span, "'radial' needs count ≥ 2 and radius > 0"));
                }
                Ok(Pattern::Radial {
                    count: count as usize,
                    radius,
                })
            }
            _ => Err(usage(span)),
        }
    }

    /// The copy count — the dimension text's `N×` prefix [SPEC 15.6].
    pub fn count(self) -> usize {
        match self {
            Pattern::Grid { cols, rows, .. } => cols * rows,
            Pattern::Radial { count, .. } => count,
        }
    }

    /// A bolt circle's radius — what the generated `|pitch-circle|` is sized
    /// to [SPEC 15.7]; `None` for a grid, which draws no ring.
    pub fn ring_radius(self) -> Option<f64> {
        match self {
            Pattern::Radial { radius, .. } => Some(radius),
            Pattern::Grid { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, args: &[f64]) -> ResolvedCall {
        ResolvedCall {
            name: name.into(),
            args: args.iter().map(|n| ResolvedValue::Number(*n)).collect(),
        }
    }

    fn read(name: &str, args: &[f64]) -> Result<Pattern, Error> {
        Pattern::read(&call(name, args), Span::default())
    }

    #[test]
    fn each_form_reads_its_own_arity() {
        assert_eq!(
            read("grid", &[2.0, 3.0, 10.0, -4.0]).unwrap(),
            Pattern::Grid {
                cols: 2,
                rows: 3,
                dx: 10.0,
                dy: -4.0
            }
        );
        assert_eq!(
            read("radial", &[4.0, 25.0]).unwrap(),
            Pattern::Radial {
                count: 4,
                radius: 25.0
            }
        );
    }

    #[test]
    fn an_extra_argument_is_an_error_not_a_silent_drop() {
        // The reported bug: `radial(4, 14, 45)` read as a start angle
        // compiled clean and rotated nothing.
        for args in [
            vec![4.0, 14.0, 45.0],
            vec![4.0, 14.0, 45.0, 99.0, 7.0],
            vec![4.0],
        ] {
            let err = read("radial", &args).unwrap_err();
            assert!(err.message.contains("radial(count, radius)"), "{err:?}");
        }
        for args in [vec![2.0, 1.0, 30.0, 0.0, 77.0], vec![2.0, 1.0, 30.0]] {
            let err = read("grid", &args).unwrap_err();
            assert!(err.message.contains("grid(cols, rows, dx, dy)"), "{err:?}");
        }
    }

    #[test]
    fn the_ranges_are_the_specs() {
        assert!(
            read("radial", &[1.0, 20.0])
                .unwrap_err()
                .message
                .contains("count ≥ 2")
        );
        assert!(
            read("radial", &[4.0, 0.0])
                .unwrap_err()
                .message
                .contains("radius > 0")
        );
        assert!(
            read("grid", &[0.0, 1.0, 5.0, 0.0])
                .unwrap_err()
                .message
                .contains("cols ≥ 1")
        );
    }

    #[test]
    fn an_unknown_name_reads_as_the_usage() {
        assert!(
            read("gird", &[2.0, 1.0, 30.0, 0.0])
                .unwrap_err()
                .message
                .contains("'pattern' takes")
        );
    }

    #[test]
    fn the_count_and_the_ring_come_off_the_form() {
        assert_eq!(read("grid", &[2.0, 3.0, 1.0, 1.0]).unwrap().count(), 6);
        assert_eq!(read("radial", &[6.0, 28.0]).unwrap().count(), 6);
        assert_eq!(
            read("radial", &[6.0, 28.0]).unwrap().ring_radius(),
            Some(28.0)
        );
        assert_eq!(
            read("grid", &[2.0, 3.0, 1.0, 1.0]).unwrap().ring_radius(),
            None
        );
    }
}
