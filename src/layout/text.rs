//! Text width / height approximation for bbox math.
//!
//! A coarse linear approximation per char. [SPEC 7] calls for embedded font
//! metrics for reproducibility — that lands once a default font is picked.
//! `letter-spacing` / `line-spacing` [SPEC 13] feed in here so the box grows to
//! fit the baked spacing.

/// Average char width as a fraction of font size. The default font is
/// **monospace**, where every glyph shares one advance (~0.6 em across Menlo /
/// Consolas / Courier), so this linear estimate is essentially exact — no font
/// metrics needed. A proportional `font-family` override makes it approximate
/// again [SPEC 22].
const AVG_CHAR_WIDTH_RATIO: f64 = 0.6;

/// Approximate the width of a label at the given font size, in px. Multi-line
/// labels (containing `\n`) take the widest line. Each adjacent glyph pair steps
/// by `advance + letter_spacing`; once that turns negative the glyphs overlap and
/// reverse, so the **drawn extent** is `advance + (n − 1) × |step|` — never below
/// one glyph, and growing again past the flip (it is `|step|`, not the signed
/// step, that the rendered box spans).
pub fn approx_width(text: &str, font_size: f64, letter_spacing: f64) -> f64 {
    let advance = font_size * AVG_CHAR_WIDTH_RATIO;
    let step = (advance + letter_spacing).abs();
    text.split('\n')
        .map(|line| match line.chars().count() {
            0 => 0.0,
            n => advance + (n as f64 - 1.0) * step,
        })
        .fold(0.0_f64, f64::max)
}

/// Per-line box for multi-line text: lines render ~1.2 em tall and abut at this
/// leading [SPEC 5], so a multi-line block needs the full `n × 1.2` or its
/// outer lines clip.
const LINE_LEADING: f64 = 1.2;
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
    let leading = LINE_LEADING * font_size;
    let step = (leading + line_spacing).abs();
    leading + (line_count as f64 - 1.0) * step
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_width_scales_with_chars_and_size() {
        let w = approx_width("hello", 13.0, 0.0);
        // 5 chars × 13 × 0.6 = 39
        assert!((w - 39.0).abs() < 0.01, "got {}", w);
    }

    #[test]
    fn multi_line_picks_widest() {
        let w = approx_width("hi\nhello", 10.0, 0.0);
        // "hello" wins: 5 × 10 × 0.6 = 30
        assert!((w - 30.0).abs() < 0.01, "got {}", w);
    }

    #[test]
    fn letter_spacing_widens_by_the_gaps_only() {
        // "hello" = 5 glyphs → 4 gaps; +4 each → 39 + 16 = 55. A single glyph
        // has no gap, so it is unaffected.
        assert!((approx_width("hello", 13.0, 4.0) - 55.0).abs() < 0.01);
        assert!((approx_width("x", 13.0, 4.0) - approx_width("x", 13.0, 0.0)).abs() < 0.01);
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
    fn negative_letter_spacing_collapses_then_grows_back() {
        // advance = 15 × 0.6 = 9. At -9 the glyphs stack: the box bottoms out at
        // one glyph (never negative), then grows again as the text reverses.
        let advance = 9.0;
        assert!((approx_width("hello", 15.0, -9.0) - advance).abs() < 0.01);
        // -18: |9 − 18| = 9 step → 9 + 4 × 9 = 45.
        assert!((approx_width("hello", 15.0, -18.0) - 45.0).abs() < 0.01);
        assert!(approx_width("hello", 15.0, -1000.0) >= advance);
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
