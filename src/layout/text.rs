//! Text width / height approximation for bbox math.
//!
//! Width is measured from the bundled per-glyph metrics tables [SPEC 5]:
//! Σ advances at the resolved kind × weight (`crate::font`) — exact for the
//! bundled families, and for mono identical to the historic flat 0.6 em
//! estimate at every weight. `letter-spacing` / `line-spacing` [SPEC 13] feed
//! in here so the box grows to fit the baked spacing.

use crate::font::Font;
use crate::ledger::consts::TEXT_LEADING;

/// The width of a label at the given measurement font and size, in px.
/// Multi-line labels (containing `\n`) take the widest line. With
/// `letter-spacing`, each adjacent glyph pair steps by a further `spacing`;
/// once `min-advance + spacing` turns negative the glyphs overlap and reverse,
/// so the **drawn extent** never falls below one glyph and grows again past
/// the flip (it is the magnitude, not the signed step, that the rendered box
/// spans — with per-glyph advances, folded pair by pair).
pub fn approx_width(text: &str, font: Font, font_size: f64, letter_spacing: f64) -> f64 {
    text.split('\n')
        .map(|line| line_width(line, font, font_size, letter_spacing))
        .fold(0.0_f64, f64::max)
}

/// One line's drawn extent: glyph advances summed left to right, each
/// inter-glyph step widened (or narrowed) by `letter-spacing`. The running
/// position may retreat under a negative spacing; the extent is the furthest
/// right edge any glyph reached, floored at the widest single glyph.
///
/// A uniform-advance line (every mono line) takes the closed form
/// `advance + (n − 1) × |advance + spacing|` — the same value the fold
/// yields, computed without the fold's float accumulation, so mono widths
/// stay bit-exact with the historic flat estimate.
fn line_width(line: &str, font: Font, font_size: f64, letter_spacing: f64) -> f64 {
    let mut chars = line.chars();
    let Some(first) = chars.next() else {
        return 0.0;
    };
    let first_em = font.advance_em(first);
    let mut n = 1usize;
    let mut uniform = true;
    // `pos` tracks the next glyph's start (previous right edge + spacing).
    let mut pos = first_em * font_size;
    let mut extent = first_em * font_size;
    for ch in chars {
        n += 1;
        let em = font.advance_em(ch);
        uniform &= em == first_em;
        let advance = em * font_size;
        pos += letter_spacing;
        extent = extent.max(pos.abs() + advance);
        pos += advance;
    }
    if uniform {
        let advance = first_em * font_size;
        let step = (advance + letter_spacing).abs();
        return advance + (n as f64 - 1.0) * step;
    }
    extent
}

/// A single line's tight box — about one em, so the glyphs nearly fill it.
/// `dominant-baseline:central` keeps them centred, so a snug box just hugs them:
/// with `padding:0` the text almost touches the edges, no slack 1.2 halo.
const SINGLE_LINE_EM: f64 = 1.0;

/// Height of a (possibly multi-line) text block. A lone line gets the tight box;
/// multi-line keeps the full per-line leading so nothing clips [SPEC 5]. Lines
/// step by `leading + line_spacing`; like `letter-spacing`, once that turns
/// negative the lines overlap and reverse, so the drawn block spans `leading +
/// (n − 1) × |step|` — never below one line.
pub fn approx_height(text: &str, font_size: f64, line_spacing: f64) -> f64 {
    let line_count = text.split('\n').count().max(1);
    if line_count == 1 {
        return SINGLE_LINE_EM * font_size;
    }
    let leading = TEXT_LEADING * font_size;
    let step = (leading + line_spacing).abs();
    leading + (line_count as f64 - 1.0) * step
}

/// Break text into lines that fit `max_w` [SPEC 5]: authored `\n` lines wrap
/// independently, breaks prefer **whitespace**, and a word too wide for the
/// cap alone breaks inside itself (at `char` boundaries — the practical
/// grapheme approximation for the bundled charset) so the no-clip law holds
/// at any width; a line never goes below one glyph. The scalar
/// [`approx_width`] / [`approx_height`] measure the `\n`-joined result, so
/// the wrapped size **is** the measured size.
pub fn wrap(
    text: &str,
    font: Font,
    font_size: f64,
    letter_spacing: f64,
    max_w: f64,
) -> Vec<String> {
    let fits = |s: &str| approx_width(s, font, font_size, letter_spacing) <= max_w + 1e-9;
    let mut out = Vec::new();
    for raw in text.split('\n') {
        let mut cur = String::new();
        for word in raw.split_whitespace() {
            let candidate = if cur.is_empty() {
                word.to_string()
            } else {
                format!("{cur} {word}")
            };
            if fits(&candidate) {
                cur = candidate;
                continue;
            }
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            // The word alone may still exceed the cap — hard-break it: each
            // line takes the largest fitting prefix, floored at one glyph.
            let mut rest: &str = word;
            while !fits(rest) && rest.chars().count() > 1 {
                let mut cut = rest.chars().next().map_or(1, char::len_utf8);
                for (i, _) in rest.char_indices().skip(1) {
                    if fits(&rest[..i]) {
                        cut = i;
                    } else {
                        break;
                    }
                }
                out.push(rest[..cut].to_string());
                rest = &rest[cut..];
            }
            cur = rest.to_string();
        }
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const F: Font = Font::MONO_REGULAR;

    #[test]
    fn single_line_width_scales_with_chars_and_size() {
        let w = approx_width("hello", F, 13.0, 0.0);
        // 5 chars × 13 × 0.6 = 39
        assert!((w - 39.0).abs() < 0.01, "got {}", w);
    }

    #[test]
    fn multi_line_picks_widest() {
        let w = approx_width("hi\nhello", F, 10.0, 0.0);
        // "hello" wins: 5 × 10 × 0.6 = 30
        assert!((w - 30.0).abs() < 0.01, "got {}", w);
    }

    #[test]
    fn letter_spacing_widens_by_the_gaps_only() {
        // "hello" = 5 glyphs → 4 gaps; +4 each → 39 + 16 = 55. A single glyph
        // has no gap, so it is unaffected.
        assert!((approx_width("hello", F, 13.0, 4.0) - 55.0).abs() < 0.01);
        assert!((approx_width("x", F, 13.0, 4.0) - approx_width("x", F, 13.0, 0.0)).abs() < 0.01);
    }

    #[test]
    fn height_grows_with_lines() {
        // One line is a tight em (10); multi-line keeps the full 1.2 per line.
        let h1 = approx_height("a", 10.0, 0.0);
        let h2 = approx_height("a\nb", 10.0, 0.0);
        assert!((h1 - 10.0).abs() < 0.01, "got {h1}");
        assert!((h2 - 24.0).abs() < 0.01, "got {h2}");
    }

    #[test]
    fn line_spacing_adds_between_lines_only() {
        // 2 lines → 1 gap; +6 → 24 + 6 = 30. A single line has no gap.
        assert!((approx_height("a\nb", 10.0, 6.0) - 30.0).abs() < 0.01);
        assert!((approx_height("a", 10.0, 6.0) - approx_height("a", 10.0, 0.0)).abs() < 0.01);
    }

    #[test]
    fn wrap_prefers_whitespace() {
        // 10 px/char at size 10 × 0.6 = 6 px/char; 60 px fits 10 chars.
        let lines = wrap("wrap me into some lines", F, 10.0, 0.0, 60.0);
        assert_eq!(lines, ["wrap me", "into some", "lines"]);
    }

    #[test]
    fn wrap_breaks_inside_an_oversized_word() {
        // 5 chars fit; an 12-char word hard-breaks at glyph boundaries.
        let lines = wrap("abcdefghijkl", F, 10.0, 0.0, 30.0);
        assert_eq!(lines, ["abcde", "fghij", "kl"]);
    }

    #[test]
    fn wrap_never_goes_below_one_glyph() {
        // A cap below one glyph still emits one glyph per line — the no-clip
        // law degrades, never divides by zero.
        let lines = wrap("abc", F, 10.0, 0.0, 1.0);
        assert_eq!(lines, ["a", "b", "c"]);
    }

    #[test]
    fn wrap_respects_authored_newlines() {
        let lines = wrap("one two\nthree", F, 10.0, 0.0, 30.0);
        assert_eq!(lines, ["one", "two", "three"]);
        // A blank authored line survives.
        assert_eq!(wrap("a\n\nb", F, 10.0, 0.0, 60.0), ["a", "", "b"]);
    }

    #[test]
    fn wrapped_size_is_the_measured_size() {
        let joined = wrap("wrap me into some lines", F, 10.0, 0.0, 60.0).join("\n");
        assert!(approx_width(&joined, F, 10.0, 0.0) <= 60.0);
        assert!(approx_height(&joined, 10.0, 0.0) > approx_height("x", 10.0, 0.0));
    }

    #[test]
    fn negative_letter_spacing_collapses_then_grows_back() {
        // advance = 15 × 0.6 = 9. At -9 the glyphs stack: the box bottoms out at
        // one glyph (never negative), then grows again as the text reverses.
        let advance = 9.0;
        assert!((approx_width("hello", F, 15.0, -9.0) - advance).abs() < 0.01);
        // -18: |9 − 18| = 9 step → 9 + 4 × 9 = 45.
        assert!((approx_width("hello", F, 15.0, -18.0) - 45.0).abs() < 0.01);
        assert!(approx_width("hello", F, 15.0, -1000.0) >= advance);
    }

    #[test]
    fn negative_line_spacing_collapses_then_grows_back() {
        // leading = 10 × 1.2 = 12. At -12 the three lines stack to one line's
        // height; past it the block reverses and grows again, never negative.
        let leading = 12.0;
        assert!((approx_height("a\nb\nc", 10.0, -12.0) - leading).abs() < 0.01);
        assert!(approx_height("a\nb\nc", 10.0, -1000.0) >= leading);
    }
}
