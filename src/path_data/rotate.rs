//! Quarter turns of path data — the geometry side of a schematic part's pose
//! [SPEC 16.1]: a rotated part is **re-laid**, not paint-transformed, so its
//! symbol's `d` is rewritten here and its ports move by the same [`point`] map.
//! Turns are exact (no trigonometry): a quarter turn only swaps and negates
//! coordinates, so a glyph's round numbers stay round and four turns are the
//! identity.

use super::{P, Scanner};
use crate::render::values::num;

/// `p` after `quarters` clockwise quarter turns of the `w` × `h` box anchored
/// at the origin. The turned box is again anchored at the origin — transposed
/// (`h` × `w`) on an odd quarter — so a glyph's coordinates stay non-negative
/// and its ports stay in its own frame.
pub(crate) fn point(p: P, quarters: u8, w: f64, h: f64) -> P {
    match quarters % 4 {
        1 => (h - p.1, p.0),
        2 => (w - p.0, h - p.1),
        3 => (p.1, w - p.0),
        _ => p,
    }
}

/// `d` turned `quarters` clockwise quarter turns in the `w` × `h` box, in the
/// same frame [`point`] uses. The result is absolute and canonical — `H`/`V`
/// become `L`, a `M`'s trailing pairs become explicit `L`s — because a turn
/// takes a horizontal run to a vertical one; every other command keeps its
/// letter (`S`/`T`'s implied control reflects about the current point, which
/// commutes with rotation, and an arc's sweep survives a rotation, only its
/// x-axis angle turning with the rest).
pub(crate) fn rotated(d: &str, quarters: u8, w: f64, h: f64) -> String {
    let q = quarters % 4;
    if q == 0 {
        return d.to_string();
    }
    let m = |p: P| point(p, q, w, h);
    let mut s = Scanner::new(d);
    let mut out = String::new();
    let (mut cx, mut cy) = (0.0, 0.0); // current point
    let (mut sx, mut sy) = (0.0, 0.0); // subpath start (for Z)

    while let Some(cmd) = s.command() {
        let rel = cmd.is_ascii_lowercase();
        match cmd.to_ascii_uppercase() {
            b'M' => {
                let Some(p) = s.coord(rel, cx, cy) else { break };
                (cx, cy) = p;
                (sx, sy) = p;
                emit(&mut out, 'M', &[m(p)]);
                while let Some(p) = s.coord(rel, cx, cy) {
                    (cx, cy) = p;
                    emit(&mut out, 'L', &[m(p)]);
                }
            }
            b'L' => {
                while let Some(p) = s.coord(rel, cx, cy) {
                    (cx, cy) = p;
                    emit(&mut out, 'L', &[m(p)]);
                }
            }
            b'H' => {
                while let Some(n) = s.number() {
                    cx = if rel { cx + n } else { n };
                    emit(&mut out, 'L', &[m((cx, cy))]);
                }
            }
            b'V' => {
                while let Some(n) = s.number() {
                    cy = if rel { cy + n } else { n };
                    emit(&mut out, 'L', &[m((cx, cy))]);
                }
            }
            b'C' => {
                while let Some([c1, c2, end]) = s.coords3(rel, cx, cy) {
                    emit(&mut out, 'C', &[m(c1), m(c2), m(end)]);
                    (cx, cy) = end;
                }
            }
            b'S' => {
                while let Some([c2, end]) = s.coords2(rel, cx, cy) {
                    emit(&mut out, 'S', &[m(c2), m(end)]);
                    (cx, cy) = end;
                }
            }
            b'Q' => {
                while let Some([ctrl, end]) = s.coords2(rel, cx, cy) {
                    emit(&mut out, 'Q', &[m(ctrl), m(end)]);
                    (cx, cy) = end;
                }
            }
            b'T' => {
                while let Some(end) = s.coord(rel, cx, cy) {
                    emit(&mut out, 'T', &[m(end)]);
                    (cx, cy) = end;
                }
            }
            b'A' => {
                while let Some(a) = s.arc(rel, cx, cy) {
                    let end = m(a.end);
                    out.push_str(&format!(
                        " A {} {} {} {} {} {} {}",
                        num(a.rx),
                        num(a.ry),
                        num((a.rot + 90.0 * q as f64).rem_euclid(360.0)),
                        u8::from(a.large),
                        u8::from(a.sweep),
                        num(end.0),
                        num(end.1),
                    ));
                    (cx, cy) = a.end;
                }
            }
            b'Z' => {
                (cx, cy) = (sx, sy);
                out.push_str(" Z");
            }
            _ => break, // unknown command — stop, keep what was turned
        }
    }
    out.trim_start().to_string()
}

fn emit(out: &mut String, cmd: char, pts: &[P]) {
    out.push(' ');
    out.push(cmd);
    for (x, y) in pts {
        out.push(' ');
        out.push_str(&num(*x));
        out.push(' ');
        out.push_str(&num(*y));
    }
}

#[cfg(test)]
mod tests {
    use super::super::extent_points;
    use super::*;

    #[test]
    fn a_quarter_turn_maps_every_point_by_the_same_map() {
        // Every registered glyph, every quarter: the turned path's points are
        // the original's under `point` — one map for the `d` and the ports.
        for name in crate::glyph::names() {
            let g = crate::glyph::lookup(name).expect(name);
            for (_, frag) in g.frags {
                let d = frag
                    .strip_prefix(r#"<path d=""#)
                    .and_then(|f| f.strip_suffix(r#""/>"#))
                    .expect("one <path d=…/>");
                let src = extent_points(d);
                for q in 1..4u8 {
                    let got = extent_points(&rotated(d, q, g.width, g.height));
                    assert_eq!(got.len(), src.len(), "{name} q{q}: command stream kept");
                    for (a, b) in got
                        .iter()
                        .zip(src.iter().map(|p| point(*p, q, g.width, g.height)))
                    {
                        assert!(
                            (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6,
                            "{name} q{q}: {a:?} vs {b:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn four_quarters_are_the_identity() {
        let d = "M 0 6 L 12 6 M 12 0 L 52 0 L 52 12 L 12 12 Z M 52 6 L 64 6";
        let mut turned = d.to_string();
        for q in 0..4 {
            let (w, h) = if q % 2 == 0 {
                (64.0, 12.0)
            } else {
                (12.0, 64.0)
            };
            turned = rotated(&turned, 1, w, h);
        }
        assert_eq!(turned, d);
    }

    #[test]
    fn a_turn_canonicalizes_relative_runs_and_shorthands() {
        // `h`/`v` runs become the absolute `L`s the turn makes of them, and a
        // relative subpath is read against the running point.
        assert_eq!(
            rotated("M 0 0 h 10 v 5 z", 1, 10.0, 5.0),
            "M 5 0 L 5 10 L 0 10 Z"
        );
    }

    #[test]
    fn an_arcs_axis_turns_with_it_and_its_sweep_survives() {
        assert_eq!(
            rotated("M 0 0 A 50 50 0 0 1 100 0", 1, 100.0, 50.0),
            "M 50 0 A 50 50 90 0 1 50 100"
        );
    }
}
