//! Text-leaf emission [SPEC 3/16]: one `<text>` for a node's text **and** a
//! link's label — the single code path behind both, so a styleable text leaf
//! reads the same wherever it sits. It bakes `letter-spacing` into a per-glyph
//! `dx`, splits multi-line `\n` text into `<tspan>`s (leading `font-size × 1.2`
//! plus `line-spacing`), and turns `rotate` into a `transform`. `translate` is
//! folded into the placed centre upstream — a node at layout, a label at routing
//! — so it never reappears here.

use super::values::{escape_xml, num};
use crate::layout::approx_width;
use crate::ledger::consts::TEXT_LEADING;
use crate::resolve::{AttrMap, ResolvedValue};
use std::fmt::Write;

/// Emit a `<text class="{class}">` centred on `(x, y)` — its `text-anchor:
/// middle` + `dominant-baseline: central` come from the class rule. `style` is
/// the caller's precomputed paint/font `style=` string (a node and a link diff
/// against different rule defaults, so each builds its own); everything geometric
/// is shared. `attrs` are the node/label's resolved attrs, read for
/// `letter-spacing`, `line-spacing`, `font-size`, and `rotate`.
pub(crate) fn emit(
    out: &mut String,
    indent: &str,
    class: &str,
    content: &str,
    pos: (f64, f64),
    attrs: &AttrMap,
    style: &str,
) {
    let (x, y) = pos;
    let (xs, ys) = (num(x), num(y));
    // `letter-spacing` bakes into a per-glyph `dx` list — geometry, never CSS
    // [SPEC 13]; `text-anchor: middle` still centres the spaced run.
    let ls = attrs.number("letter-spacing").unwrap_or(0.0);
    // A styled leaf's `rotate` turns it about its own centre; `translate` is
    // already in `(x, y)`.
    let xform = match attrs.number("rotate") {
        Some(r) if r != 0.0 => format!(r#" transform="rotate({} {} {})""#, num(r), xs, ys),
        _ => String::new(),
    };

    let lines: Vec<&str> = content.split('\n').collect();
    if lines.len() <= 1 {
        writeln!(
            out,
            r#"{indent}<text class="{class}" x="{xs}" y="{ys}"{}{style}{xform}>{}</text>"#,
            dx_attr(content, ls),
            escape_xml(content),
        )
        .unwrap();
        return;
    }

    // Multi-line [SPEC 5]: one tspan per line, leading `font-size × 1.2` plus
    // `line-spacing`, the block centred on (x, y) so `dominant-baseline: central`
    // still holds. The layout-stamped `line-align` [SPEC 6] anchors each line
    // to the block's edge: `text-anchor: middle` stands (the class rule), so a
    // line's `x` is its own centre — computed through the one measurement API.
    let size = attrs.number("font-size").unwrap_or(0.0);
    let spacing = size * TEXT_LEADING + attrs.number("line-spacing").unwrap_or(0.0);
    let top = y - spacing * (lines.len() as f64 - 1.0) / 2.0;
    let line_x: Box<dyn Fn(&str) -> f64> = match attrs.get("line-align") {
        Some(ResolvedValue::Ident(a)) if a == "start" || a == "end" => {
            let block = approx_width(content, size, ls);
            if a == "start" {
                let left = x - block / 2.0;
                Box::new(move |line| left + approx_width(line, size, ls) / 2.0)
            } else {
                let right = x + block / 2.0;
                Box::new(move |line| right - approx_width(line, size, ls) / 2.0)
            }
        }
        _ => Box::new(move |_| x),
    };
    write!(
        out,
        r#"{indent}<text class="{class}" x="{xs}" y="{ys}"{style}{xform}>"#
    )
    .unwrap();
    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            write!(
                out,
                r#"<tspan x="{}" y="{}"{}>{}</tspan>"#,
                num(line_x(line)),
                num(top),
                dx_attr(line, ls),
                escape_xml(line)
            )
            .unwrap();
        } else {
            write!(
                out,
                r#"<tspan x="{}" dy="{}"{}>{}</tspan>"#,
                num(line_x(line)),
                num(spacing),
                dx_attr(line, ls),
                escape_xml(line)
            )
            .unwrap();
        }
    }
    writeln!(out, "</text>").unwrap();
}

/// The ` dx="0 s s …"` glyph-advance list that bakes `letter-spacing` into the
/// glyph positions: 0 before the first glyph, `s` before each later one. Empty
/// when there is no spacing or fewer than two glyphs (nothing to space).
fn dx_attr(line: &str, letter_spacing: f64) -> String {
    let count = line.chars().count();
    if letter_spacing == 0.0 || count < 2 {
        return String::new();
    }
    let mut s = String::from(r#" dx="0"#);
    for _ in 1..count {
        s.push(' ');
        s.push_str(&num(letter_spacing));
    }
    s.push('"');
    s
}
