//! Deterministic floating-point math — the **one** home for every transcendental
//! the compiler evaluates.
//!
//! `f64::sin` and friends call the *platform's* libm: Apple's on macOS, glibc's
//! on Linux, and — because `wasm32-unknown-unknown` has no system library at all
//! — a Rust one in the browser. The three disagree by up to 1 ULP on `tan`,
//! `atan2`, `exp`, `ln`, `log10`, `pow`, and `hypot`. One ULP is invisible on a
//! page and fatal to a promise: [ROADMAP §2] says the same input yields
//! byte-identical SVG, and the README sells diffing those bytes in CI. Under the
//! platform libm that holds only so long as everyone is on the same platform,
//! and nothing would report the day they were not.
//!
//! So the compiler calls none of them. Every site routes through this module,
//! which delegates to [`libm`] — a pure-Rust port of musl's, compiled from the
//! same source for every target. macOS, Linux, and the browser then agree bit
//! for bit.
//!
//! **Not wrapped, deliberately:** `sqrt` (IEEE-754 exact — a hardware
//! instruction everywhere), `powi` (LLVM expands it to multiplications, the
//! same expansion on every target), and the exact operations `abs`, `floor`,
//! `ceil`, `round`, `clamp`, `to_degrees`, `to_radians`. Those stay as inherent
//! methods; adding them here would imply the rest are unsafe to call, which
//! they are not.
//!
//! `tests/determinism.rs` greps `src/` for the wrapped methods, so a new
//! `.atan2(` cannot creep back in unnoticed.

/// Sine of `x` radians.
pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}

/// Cosine of `x` radians.
pub fn cos(x: f64) -> f64 {
    libm::cos(x)
}

/// Tangent of `x` radians.
pub fn tan(x: f64) -> f64 {
    libm::tan(x)
}

/// Arc cosine, in radians.
pub fn acos(x: f64) -> f64 {
    libm::acos(x)
}

/// Four-quadrant arc tangent of `y / x`, in radians — argument order matches
/// `f64::atan2`, so `y.atan2(x)` becomes `math::atan2(y, x)`.
pub fn atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}

/// `e` raised to `x`.
pub fn exp(x: f64) -> f64 {
    libm::exp(x)
}

/// Natural logarithm.
pub fn ln(x: f64) -> f64 {
    libm::log(x)
}

/// Base-10 logarithm.
pub fn log10(x: f64) -> f64 {
    libm::log10(x)
}

/// `base` raised to `exp` — the float exponent form (`f64::powf`).
pub fn powf(base: f64, exp: f64) -> f64 {
    libm::pow(base, exp)
}

/// Euclidean length `√(x² + y²)`, without the intermediate overflow a naive
/// `sqrt` would suffer.
pub fn hypot(x: f64, y: f64) -> f64 {
    libm::hypot(x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The values that exposed the platform split — the browser build computed
    /// each of these one ULP away from macOS before this module existed. Pinned
    /// as exact bits: if a future `libm` bump moves one, that is a
    /// snapshot-churning event and it should be a deliberate one.
    #[test]
    fn transcendentals_are_pinned_bit_exact() {
        for (got, want) in [
            (tan(0.6435011087932844), 0.75f64),
            (atan2(3.0, 4.0), 0.6435011087932844),
            (ln(1.2345678901234567), 0.21072102231565248),
            (log10(1000.0), 3.0),
            (exp(1.2345678901234567), 3.436893084346008),
            (hypot(3.0, 4.0), 5.0),
            (powf(10.0, 2.5), 316.2277660168379),
        ] {
            assert_eq!(
                got.to_bits(),
                want.to_bits(),
                "{got:.17} != {want:.17} — libm changed under us"
            );
        }
    }

    /// `sin`/`cos` happened to agree across platforms already; wrap them anyway,
    /// and check the wrapper is the identity we think it is.
    #[test]
    fn wrappers_agree_with_the_inherent_methods_where_it_matters() {
        for x in [0.0, 0.5, 1.0, -2.25, 36.8699] {
            assert_eq!(sin(x).to_bits(), libm::sin(x).to_bits());
            assert_eq!(cos(x).to_bits(), libm::cos(x).to_bits());
        }
    }
}
