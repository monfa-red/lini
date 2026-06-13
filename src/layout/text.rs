//! Text width approximation for bbox math.
//!
//! A coarse linear approximation per char. SPEC §7 calls for embedded font
//! metrics for reproducibility — that lands once a default font is picked.

/// Average char width as a fraction of font size. Picked to roughly match
/// proportional sans-serif fonts (Inter, system-ui, Arial).
const AVG_CHAR_WIDTH_RATIO: f64 = 0.55;

/// Approximate the width of a single-line label at the given font size, in px.
/// Multi-line labels (containing `\n`) take the widest line.
pub fn approx_width(text: &str, font_size: f64) -> f64 {
    text.split('\n')
        .map(|line| line.chars().count() as f64 * font_size * AVG_CHAR_WIDTH_RATIO)
        .fold(0.0_f64, f64::max)
}

/// Height of a (possibly multi-line) text block: line_count × size × 1.2 per
/// SPEC §5.
pub fn approx_height(text: &str, font_size: f64) -> f64 {
    let line_count = text.split('\n').count().max(1) as f64;
    line_count * font_size * 1.2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_width_scales_with_chars_and_size() {
        let w = approx_width("hello", 13.0);
        // 5 chars × 13 × 0.55 = 35.75
        assert!((w - 35.75).abs() < 0.01, "got {}", w);
    }

    #[test]
    fn multi_line_picks_widest() {
        let w = approx_width("hi\nhello", 10.0);
        // "hello" wins: 5 × 10 × 0.55 = 27.5
        assert!((w - 27.5).abs() < 0.01, "got {}", w);
    }

    #[test]
    fn height_grows_with_lines() {
        let h1 = approx_height("a", 10.0);
        let h2 = approx_height("a\nb", 10.0);
        assert!((h1 - 12.0).abs() < 0.01);
        assert!((h2 - 24.0).abs() < 0.01);
    }
}
