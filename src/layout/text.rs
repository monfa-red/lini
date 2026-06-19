//! Text width approximation for bbox math.
//!
//! A coarse linear approximation per char. SPEC §7 calls for embedded font
//! metrics for reproducibility — that lands once a default font is picked.

/// Average char width as a fraction of font size. The default font is
/// **monospace**, where every glyph shares one advance (~0.6 em across Menlo /
/// Consolas / Courier), so this linear estimate is essentially exact — no font
/// metrics needed. A proportional `font-family` override makes it approximate
/// again (SPEC §19).
const AVG_CHAR_WIDTH_RATIO: f64 = 0.6;

/// Approximate the width of a single-line label at the given font size, in px.
/// Multi-line labels (containing `\n`) take the widest line.
pub fn approx_width(text: &str, font_size: f64) -> f64 {
    text.split('\n')
        .map(|line| line.chars().count() as f64 * font_size * AVG_CHAR_WIDTH_RATIO)
        .fold(0.0_f64, f64::max)
}

/// Per-line box for multi-line text: lines render ~1.2 em tall and abut at this
/// spacing (SPEC §5), so a multi-line block needs the full `n × 1.2` or its
/// outer lines clip.
const LINE_SPACING: f64 = 1.2;
/// A single line's tight box — about one em, so the glyphs nearly fill it.
/// `dominant-baseline:central` keeps them centred, so a snug box just hugs them:
/// with `padding:0` the text almost touches the edges, no slack 1.2 halo.
const LINE_HEIGHT: f64 = 1.0;

/// Height of a (possibly multi-line) text block. A lone line gets the tight box;
/// multi-line keeps the full per-line leading so nothing clips (SPEC §5).
pub fn approx_height(text: &str, font_size: f64) -> f64 {
    let line_count = text.split('\n').count().max(1) as f64;
    let ems = if line_count > 1.0 {
        line_count * LINE_SPACING
    } else {
        LINE_HEIGHT
    };
    ems * font_size
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_width_scales_with_chars_and_size() {
        let w = approx_width("hello", 13.0);
        // 5 chars × 13 × 0.6 = 39
        assert!((w - 39.0).abs() < 0.01, "got {}", w);
    }

    #[test]
    fn multi_line_picks_widest() {
        let w = approx_width("hi\nhello", 10.0);
        // "hello" wins: 5 × 10 × 0.6 = 30
        assert!((w - 30.0).abs() < 0.01, "got {}", w);
    }

    #[test]
    fn height_grows_with_lines() {
        // One line is a tight em (10); multi-line keeps the full 1.2 per line.
        let h1 = approx_height("a", 10.0);
        let h2 = approx_height("a\nb", 10.0);
        assert!((h1 - 10.0).abs() < 0.01, "got {h1}");
        assert!((h2 - 24.0).abs() < 0.01, "got {h2}");
    }
}
