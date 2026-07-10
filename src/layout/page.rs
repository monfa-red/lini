//! The `|page|` sheet at layout [SPEC 15.8]. Desugar generated the ISO 5457
//! furniture as pinned chrome children (`|frame|` / `|zone|` / `|tick|`) and
//! marked the `|title-block|`; once the sheet is sized, `finish` gives the
//! furniture its geometry — the frame at the margins (a 20 mm filing edge on
//! the left, 10 mm elsewhere), the zone dividers and labels in the margin
//! band (the divisions derive from the children desugar counted, so the two
//! always agree), the four centring marks — and seats any title block flush
//! inside the frame's bottom-right corner. The content area is the frame
//! inset by 5 mm: `padded_attrs` folds that into the page's padding for the
//! arrange pass (`padding:` adds, per the spec).

use super::ir::{Bbox, PlacedNode};
use super::primitives;
use crate::error::Error;
use crate::ledger::consts::{SHEET_FILING, SHEET_MARGIN};
use crate::resolve::{AttrMap, ResolvedValue};
use crate::span::Span;

/// The content area's inset from the frame line, mm [SPEC 15.8].
const CONTENT_CLEAR: f64 = 5.0;
/// A centring mark crosses the frame line into the drawing area by this much.
const MARK_INTO: f64 = 5.0;

pub(super) fn is_page(type_chain: &[String]) -> bool {
    type_chain.iter().any(|t| t == "page")
}

/// The page's padding for the arrange pass: the user's own plus the content
/// area's inset from the sheet edge — frame margin + 5 mm clear, at the
/// page's px-per-mm scale [SPEC 15.8].
pub(super) fn padded_attrs(attrs: &AttrMap, scale: f64, span: Span) -> Result<AttrMap, Error> {
    let pad = primitives::padding(attrs, span)?;
    let mut out = attrs.clone();
    let n = ResolvedValue::Number;
    out.insert(
        "padding",
        ResolvedValue::Tuple(vec![
            n(pad.top + (SHEET_MARGIN + CONTENT_CLEAR) * scale),
            n(pad.right + (SHEET_MARGIN + CONTENT_CLEAR) * scale),
            n(pad.bottom + (SHEET_MARGIN + CONTENT_CLEAR) * scale),
            n(pad.left + (SHEET_FILING + CONTENT_CLEAR) * scale),
        ]),
    );
    Ok(out)
}

/// One chrome child's parsed marker.
enum Marker<'a> {
    Frame,
    Tick(&'a str, usize),
    Mark(&'a str),
    Zone(&'a str, usize),
}

fn marker(attrs: &AttrMap) -> Option<Marker<'_>> {
    match attrs.get("chrome")? {
        ResolvedValue::Ident(k) if k == "frame" => Some(Marker::Frame),
        ResolvedValue::Tuple(items) => match items.as_slice() {
            [
                ResolvedValue::Ident(k),
                ResolvedValue::Ident(e),
                ResolvedValue::Number(i),
            ] if k == "tick" => Some(Marker::Tick(e, *i as usize)),
            [ResolvedValue::Ident(k), ResolvedValue::Ident(e)] if k == "mark" => {
                Some(Marker::Mark(e))
            }
            [
                ResolvedValue::Ident(k),
                ResolvedValue::Ident(e),
                ResolvedValue::Number(i),
            ] if k == "zone" => Some(Marker::Zone(e, *i as usize)),
            _ => None,
        },
        _ => None,
    }
}

/// Position the sheet furniture and seat the title block, in the page's own
/// frame (origin at its centre), once its box is known.
pub(super) fn finish(children: &mut [PlacedNode], sheet: Bbox, s: f64) {
    let (w, h) = (sheet.w(), sheet.h());
    let (x0, y0, x1, y1) = (-w / 2.0, -h / 2.0, w / 2.0, h / 2.0);
    let (fx0, fy0) = (x0 + SHEET_FILING * s, y0 + SHEET_MARGIN * s);
    let (fx1, fy1) = (x1 - SHEET_MARGIN * s, y1 - SHEET_MARGIN * s);

    // The zone divisions come from what desugar generated — labels per edge.
    let cells = |edge: &str| {
        children
            .iter()
            .filter(|c| matches!(marker(&c.attrs), Some(Marker::Zone(e, _)) if e == edge))
            .count()
            .max(1)
    };
    let (cols, rows) = (cells("top"), cells("left"));

    for c in children.iter_mut() {
        let half = c.attrs.number("stroke-width").unwrap_or(0.0) / 2.0;
        let set_line = |c: &mut PlacedNode, a: (f64, f64), b: (f64, f64)| {
            let point = |p: (f64, f64)| {
                ResolvedValue::Tuple(vec![ResolvedValue::Number(p.0), ResolvedValue::Number(p.1)])
            };
            c.attrs
                .insert("points", ResolvedValue::List(vec![point(a), point(b)]));
            c.cx = 0.0;
            c.cy = 0.0;
            // Butt caps end exactly at the endpoint, so a tick may run the
            // full band to the trimmed edge; only the bbox (stroke inflated
            // sideways) clamps to the sheet, keeping the canvas exact.
            let b = Bbox {
                min_x: a.0.min(b.0),
                min_y: a.1.min(b.1),
                max_x: a.0.max(b.0),
                max_y: a.1.max(b.1),
            }
            .inflate(half);
            c.bbox = Bbox {
                min_x: b.min_x.max(x0),
                min_y: b.min_y.max(y0),
                max_x: b.max_x.min(x1),
                max_y: b.max_y.min(y1),
            };
        };
        match marker(&c.attrs) {
            Some(Marker::Frame) => {
                c.cx = (fx0 + fx1) / 2.0;
                c.cy = (fy0 + fy1) / 2.0;
                c.bbox = Bbox::centered(fx1 - fx0, fy1 - fy0).inflate(half);
            }
            Some(Marker::Tick(edge, i)) => {
                // The sheet-edge end pulls in by the half-stroke, so the cap
                // stays on the paper and the canvas is exactly the sheet.
                let (a, b) = match edge {
                    "top" => {
                        let x = x0 + (i as f64) * w / cols as f64;
                        ((x, y0), (x, fy0))
                    }
                    "bottom" => {
                        let x = x0 + (i as f64) * w / cols as f64;
                        ((x, fy1), (x, y1))
                    }
                    "left" => {
                        // The reference band is the 10 mm margin on every
                        // side — the filing margin's extra 10 mm stays truly
                        // empty [SPEC 15.8], so the letters read alike all
                        // round.
                        let y = y0 + (i as f64) * h / rows as f64;
                        ((fx0 - SHEET_MARGIN * s, y), (fx0, y))
                    }
                    _ => {
                        let y = y0 + (i as f64) * h / rows as f64;
                        ((fx1, y), (x1, y))
                    }
                };
                set_line(c, a, b);
            }
            Some(Marker::Mark(edge)) => {
                let (a, b) = match edge {
                    "top" => ((0.0, y0), (0.0, fy0 + MARK_INTO * s)),
                    "bottom" => ((0.0, fy1 - MARK_INTO * s), (0.0, y1)),
                    // The left mark starts at its reference band, not the
                    // trimmed edge — the filing strip stays truly empty,
                    // matching the dividers [SPEC 15.8].
                    "left" => ((fx0 - SHEET_MARGIN * s, 0.0), (fx0 + MARK_INTO * s, 0.0)),
                    _ => ((fx1 - MARK_INTO * s, 0.0), (x1, 0.0)),
                };
                set_line(c, a, b);
            }
            Some(Marker::Zone(edge, i)) => {
                let (cx, cy) = match edge {
                    "top" => (x0 + (i as f64 + 0.5) * w / cols as f64, (y0 + fy0) / 2.0),
                    "bottom" => (x0 + (i as f64 + 0.5) * w / cols as f64, (y1 + fy1) / 2.0),
                    "left" => (
                        fx0 - SHEET_MARGIN * s / 2.0,
                        y0 + (i as f64 + 0.5) * h / rows as f64,
                    ),
                    _ => ((x1 + fx1) / 2.0, y0 + (i as f64 + 0.5) * h / rows as f64),
                };
                c.cx = cx;
                c.cy = cy;
            }
            None => {
                // Seat a pinned title block flush inside the frame corner —
                // the pin put it on the sheet corner; the margins pull it in.
                if c.type_chain.iter().any(|t| t == "title-block") {
                    c.cx -= SHEET_MARGIN * s;
                    c.cy -= SHEET_MARGIN * s;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::drawing::testutil::{by_id, laid, texts};
    use super::*;

    fn compile_err(src: &str) -> String {
        let toks = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(src, &toks).expect("parse");
        match crate::desugar::desugar(&file) {
            Ok(_) => panic!("expected an error"),
            Err(e) => e.message,
        }
    }

    #[test]
    fn sheet_desugars_to_mm_dims_with_iso_orientation() {
        // A3 defaults landscape: 420 × 297 mm at 4 px/mm.
        let l = laid("|page#p| { sheet: a3 }\n\"x\"\n");
        let p = by_id(&l.nodes, "p");
        assert_eq!((p.bbox.w(), p.bbox.h()), (1680.0, 1188.0));
        // A4 defaults portrait; the keyword overrides.
        let l = laid("|page#p| { sheet: a4 }\n\"x\"\n");
        assert_eq!(by_id(&l.nodes, "p").bbox.w(), 840.0);
        let l = laid("|page#p| { sheet: a4 landscape }\n\"x\"\n");
        assert_eq!(by_id(&l.nodes, "p").bbox.w(), 1188.0);
        // Explicit dims override through the ordinary slot.
        let l = laid("|page#p| { sheet: a4; width: 300 }\n\"x\"\n");
        assert_eq!(by_id(&l.nodes, "p").bbox.w(), 1200.0);
    }

    #[test]
    fn bad_sheet_values_error_with_a_hint() {
        assert_eq!(
            compile_err("|page#p| { sheet: a9 }\n"),
            "'sheet' takes a size — a5…a0 (ISO) or a…e (ANSI) — and an optional portrait / landscape; did you mean 'a0'?"
        );
        assert_eq!(
            compile_err("|page#p| { sheet: a4 portrai }\n"),
            "'sheet' takes a size — a5…a0 (ISO) or a…e (ANSI) — and an optional portrait / landscape; did you mean 'portrait'?"
        );
    }

    #[test]
    fn the_furniture_lands_on_the_iso_anatomy() {
        // A4 portrait: 210 × 297 → 4 × 6 zones [SPEC 15.8].
        let l = laid("|page#p| { sheet: a4 }\n\"x\"\n");
        let p = by_id(&l.nodes, "p");
        let frame = p
            .children
            .iter()
            .find(|c| c.type_chain.iter().any(|t| t == "frame"))
            .expect("the frame");
        // Margins ×4 px/mm: filing 80 left, 40 elsewhere; half-stroke inflates.
        let s = 4.0;
        assert!((frame.cx - (SHEET_FILING - SHEET_MARGIN) * s / 2.0).abs() < 1e-9);
        assert!((frame.bbox.w() - (840.0 - (SHEET_FILING + SHEET_MARGIN) * s + 2.0)).abs() < 1e-9);
        let zone = |edge: &str| {
            p.children
                .iter()
                .filter(|c| matches!(marker(&c.attrs), Some(Marker::Zone(e, _)) if e == edge))
                .count()
        };
        assert_eq!((zone("top"), zone("left")), (4, 6));
        // Numbers along the top, letters down the sides.
        let all = texts(&l.nodes);
        assert!(all.iter().any(|(t, ..)| t == "4"));
        assert!(all.iter().any(|(t, ..)| t == "F"));
    }

    #[test]
    fn a_title_block_seats_flush_inside_the_frame_corner() {
        let l = laid(
            "|page#p| { sheet: a4 landscape } [\n  |title-block#tb| { columns: 40 auto } [ \"Part\" \"X\" ]\n]\n",
        );
        let p = by_id(&l.nodes, "p");
        let tb = by_id(&l.nodes, "tb");
        let s = 4.0;
        // Its right/bottom edges sit on the frame line, margins in from the sheet.
        let right = tb.cx + tb.bbox.max_x;
        let bottom = tb.cy + tb.bbox.max_y;
        assert!(
            (right - (p.bbox.w() / 2.0 - SHEET_MARGIN * s)).abs() < 1e-6,
            "flush right: {right}"
        );
        assert!(
            (bottom - (p.bbox.h() / 2.0 - SHEET_MARGIN * s)).abs() < 1e-6,
            "flush bottom: {bottom}"
        );
    }

    #[test]
    fn ansi_sheets_are_the_same_sugar_in_other_millimetres() {
        // ANSI B defaults landscape: 431.8 × 279.4 mm at 4 px/mm; the letter
        // sizes ride the exact ISO mechanism [SPEC 15.8].
        let l = laid("|page#p| { sheet: b }\n\"x\"\n");
        let p = by_id(&l.nodes, "p");
        assert_eq!((p.bbox.w(), p.bbox.h()), (431.8 * 4.0, 279.4 * 4.0));
        let a = laid("|page#p| { sheet: a }\n\"x\"\n");
        assert_eq!(
            by_id(&a.nodes, "p").bbox.w(),
            215.9 * 4.0,
            "ANSI A portrait"
        );
    }

    #[test]
    fn the_left_reference_band_matches_the_other_sides() {
        // The letters sit in the innermost 10 mm of the 20 mm filing margin —
        // the extra 10 mm stays truly empty, so the band reads alike all
        // round [SPEC 15.8].
        let l = laid("|page#p| { sheet: a4 }\n\"x\"\n");
        let p = by_id(&l.nodes, "p");
        let s = 4.0;
        let fx0 = -p.bbox.w() / 2.0 + SHEET_FILING * s;
        let left_zone = p
            .children
            .iter()
            .find(|c| matches!(marker(&c.attrs), Some(Marker::Zone(e, _)) if e == "left"))
            .expect("a left zone label");
        assert!(
            (left_zone.cx - (fx0 - SHEET_MARGIN * s / 2.0)).abs() < 1e-9,
            "letter centred in the 10 mm band beside the frame: {}",
            left_zone.cx
        );
    }

    #[test]
    fn a_lone_sheet_hugs_the_canvas() {
        // Only-pages content drops the root's padding to 0 — the paper is the
        // margin [SPEC 15.8]; other content keeps the scene default, and the
        // user's own padding still wins.
        let l = laid("|page#p| { sheet: a4 landscape }\n");
        assert_eq!((l.viewbox.w, l.viewbox.h), (1188.0, 840.0));
        let framed = laid("|page#p| { sheet: a4 landscape }\n|box| \"beside\"\n");
        assert!(
            framed.viewbox.w > 1188.0,
            "mixed content keeps the scene frame"
        );
        let padded = laid("{ padding: 12 }\n|page#p| { sheet: a4 landscape }\n");
        assert_eq!(padded.viewbox.w, 1188.0 + 24.0, "the user's padding wins");
    }

    #[test]
    fn page_content_flows_inside_the_frame_and_chrome_stays_out_of_flow() {
        // One box: it centres in the content area, which is shifted right by
        // the filing margin's asymmetry — never overlapping the band.
        let l = laid("|page#p| { sheet: a4 } [ |box#card| \"hi\" ]\n");
        let card = by_id(&l.nodes, "card");
        let s = 4.0;
        let expect = ((SHEET_FILING + CONTENT_CLEAR) - (SHEET_MARGIN + CONTENT_CLEAR)) * s / 2.0;
        assert!(
            (card.cx - expect).abs() < 1e-9,
            "content centre rides the asymmetric inset: {} vs {expect}",
            card.cx
        );
    }
}
