//! OKLCH → sRGB hex. Björn Ottosson's OKLab transform
//! (<https://bottosson.github.io/posts/oklab/>) inverted, then the sRGB transfer
//! function. Pure `f64`, no dependencies.
//!
//! The palette tiers [SPEC 10.2] are chosen in OKLCH — perceptually even lightness
//! and chroma — and baked to `#rrggbb` literals here. Targets are picked in-gamut, so
//! the channel clamp is a backstop, not a normal path.

/// An OKLCH colour (`l` 0..1, `c` chroma, `h` degrees) as a 6-digit hex string,
/// no leading `#` — the form [`crate::resolve::ResolvedValue::Hex`] stores.
pub fn oklch_to_hex(l: f64, c: f64, h_deg: f64) -> String {
    let h = h_deg.to_radians();
    let (r, g, b) = oklab_to_srgb(l, c * h.cos(), c * h.sin());
    format!("{:02x}{:02x}{:02x}", encode(r), encode(g), encode(b))
}

/// OKLab → gamma-encoded sRGB in 0..1 (out-of-gamut channels fall outside and are
/// clamped by [`encode`]).
fn oklab_to_srgb(l: f64, a: f64, b: f64) -> (f64, f64, f64) {
    let l_ = l + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m_ = l - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s_ = l - 0.089_484_177_5 * a - 1.291_485_548_0 * b;

    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);

    let r = 4.076_741_662_1 * l3 - 3.307_711_591_3 * m3 + 0.230_969_929_2 * s3;
    let g = -1.268_438_004_6 * l3 + 2.609_757_401_1 * m3 - 0.341_319_396_5 * s3;
    let b = -0.004_196_086_3 * l3 - 0.703_418_614_7 * m3 + 1.707_614_701_0 * s3;

    (gamma(r), gamma(g), gamma(b))
}

/// The sRGB transfer function. The linear branch also handles negatives (an
/// out-of-gamut channel), keeping `powf` off a negative base.
fn gamma(x: f64) -> f64 {
    if x >= 0.003_130_8 {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    } else {
        12.92 * x
    }
}

fn encode(channel: f64) -> u8 {
    (channel.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each channel within `tol` of the expected hex (reference OKLCH values are
    /// themselves rounded, so an exact match isn't promised for the primaries).
    fn assert_near(got: &str, want: &str, tol: i32) {
        let chan = |s: &str, i: usize| i32::from_str_radix(&s[i..i + 2], 16).unwrap();
        for i in [0, 2, 4] {
            let (a, b) = (chan(got, i), chan(want, i));
            assert!(
                (a - b).abs() <= tol,
                "channel at {i}: got {got} want {want} (Δ {})",
                (a - b).abs()
            );
        }
    }

    #[test]
    fn white_and_black_are_exact() {
        assert_eq!(oklch_to_hex(1.0, 0.0, 0.0), "ffffff");
        assert_eq!(oklch_to_hex(0.0, 0.0, 0.0), "000000");
    }

    #[test]
    fn a_mid_grey_has_equal_channels() {
        let hex = oklch_to_hex(0.6, 0.0, 0.0);
        assert_eq!(&hex[0..2], &hex[2..4]);
        assert_eq!(&hex[2..4], &hex[4..6]);
    }

    #[test]
    fn srgb_primaries_round_trip() {
        // OKLCH of the sRGB primaries (rounded), so allow a couple of levels.
        assert_near(&oklch_to_hex(0.6279, 0.2577, 29.23), "ff0000", 2);
        assert_near(&oklch_to_hex(0.8664, 0.2948, 142.50), "00ff00", 2);
        assert_near(&oklch_to_hex(0.4520, 0.3132, 264.05), "0000ff", 2);
    }

    #[test]
    fn out_of_gamut_chroma_clamps_not_panics() {
        // Absurd chroma at mid lightness — channels blow past [0,1]; encode clamps.
        let hex = oklch_to_hex(0.5, 0.9, 0.0);
        assert_eq!(hex.len(), 6);
        assert!(u32::from_str_radix(&hex, 16).is_ok());
    }
}
