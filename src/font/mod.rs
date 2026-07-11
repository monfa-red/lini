//! The bundled font model [SPEC 5/6, ROADMAP 3.7]: two families — Google Sans
//! Code (the mono default) and Google Sans (proportional) — × four static
//! weights {400, 500, 600, 700}. Metrics ride generated tables (`metrics.rs`,
//! `cargo xtask extract-fonts`) that are **always compiled in** — layout never
//! varies by build flags; only the subset TTF *bytes* (for `--embed-font` /
//! `--static`) sit behind the default-on `font` feature.
//!
//! **Metrics follow the kind, not the name** [SPEC 5]: a `font-family`
//! override changes only the emitted name — a known-mono name (or one
//! containing "mono") measures on the mono table, everything else on the
//! proportional one, so a runtime CSS restyle keeps the compiled layout box.

mod metrics;

use crate::resolve::{AttrMap, ResolvedValue};

pub use metrics::CHARSET;

/// Vertical + advance metrics for one family × weight, in font units. The
/// advance table indexes across [`CHARSET`] in order; 0 marks a glyph the
/// face does not cover (lookups fall back).
pub struct Face {
    pub upem: u16,
    /// Extracted with the tables [ROADMAP 3.7]; unread until something needs
    /// real vertical extents (the 1 em line box stands, [SPEC 5]).
    #[allow(dead_code)]
    pub ascent: i16,
    #[allow(dead_code)]
    pub descent: i16,
    pub cap_height: i16,
    pub advances: &'static [u16],
}

/// Which metrics table measures a family [SPEC 5].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Kind {
    #[default]
    Mono,
    Prop,
}

/// A resolved measurement font: the kind and the weight index into the four
/// statics (0..=3 ↔ 400/500/600/700). The default — no `font-family`, no
/// `font-weight` — is mono regular, whose advances are exactly the historic
/// flat 0.6 em estimate.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Font {
    pub kind: Kind,
    weight: usize,
}

/// Known-mono family names that don't carry the "mono" substring; compared
/// case-folded. The heuristic (`is_mono_name`) covers the rest.
const KNOWN_MONO: &[&str] = &[
    "google sans code",
    "menlo",
    "consolas",
    "courier",
    "courier new",
    "cascadia code",
    "fira code",
    "source code pro",
];

/// Glyphs neither bundled face covers, measured and outlined as a covered
/// typographic twin: the ISO diameter sign becomes the slashed O.
const SUBSTITUTES: &[(char, char)] = &[('⌀', 'Ø')];

/// Advance for a glyph outside the charset (or missing from the face), in em:
/// the historic flat estimate, doubled for the wide CJK ranges [SPEC 5].
fn fallback_advance_em(ch: char) -> f64 {
    let cp = ch as u32;
    let wide = matches!(
        cp,
        0x1100..=0x115F      // Hangul Jamo
        | 0x2E80..=0x9FFF    // CJK radicals … unified ideographs
        | 0xAC00..=0xD7AF    // Hangul syllables
        | 0xF900..=0xFAFF    // CJK compatibility ideographs
        | 0xFF00..=0xFF60    // fullwidth forms
        | 0x20000..=0x3FFFD, // CJK extensions
    );
    if wide { 1.0 } else { 0.6 }
}

fn is_mono_name(family: &str) -> bool {
    let f = family.trim().trim_matches(['"', '\'']).to_ascii_lowercase();
    f.contains("mono") || KNOWN_MONO.contains(&f.as_str())
}

impl Kind {
    /// The measurement kind of a `font-family` value — the first family in a
    /// stack decides. No value ⇒ the mono default.
    pub fn of_family(value: Option<&ResolvedValue>) -> Kind {
        let name = match value {
            Some(ResolvedValue::String(s))
            | Some(ResolvedValue::Ident(s))
            | Some(ResolvedValue::RawCss(s)) => s,
            _ => return Kind::Mono,
        };
        let first = name.split(',').next().unwrap_or(name);
        if is_mono_name(first) {
            Kind::Mono
        } else {
            Kind::Prop
        }
    }
}

impl Font {
    /// The default measurement font — mono regular, the flat 0.6 em table.
    #[allow(dead_code)] // exercised by unit tests; the lib target sees no use yet
    pub const MONO_REGULAR: Font = Font {
        kind: Kind::Mono,
        weight: 0,
    };

    /// A kind at its regular weight — for generated chrome whose class rule
    /// states `font-weight: normal`.
    pub fn regular(kind: Kind) -> Font {
        Font { kind, weight: 0 }
    }

    /// A kind at bold — for generated chrome whose rule or inline style says
    /// `bold` (mono advances are weight-invariant, so this only moves
    /// proportional measurements).
    pub fn bold(kind: Kind) -> Font {
        Font { kind, weight: 3 }
    }

    /// Resolve the measurement font off a node's effective attrs — the
    /// inherited `font-family` picks the kind, `font-weight` the static
    /// [SPEC 6]. The one constructor every measurement caller shares.
    pub fn of(attrs: &AttrMap) -> Font {
        Font {
            kind: Kind::of_family(attrs.get("font-family")),
            weight: weight_index(attrs.get("font-weight")),
        }
    }

    pub fn face(&self) -> &'static Face {
        match self.kind {
            Kind::Mono => metrics::MONO[self.weight],
            Kind::Prop => metrics::PROP[self.weight],
        }
    }

    /// One glyph's advance, in em. Falls through: the face's table, the
    /// substitute twin's, then the fixed fallback [SPEC 5].
    pub fn advance_em(&self, ch: char) -> f64 {
        let face = self.face();
        let lookup = |c: char| {
            charset_index(c)
                .map(|i| face.advances[i])
                .filter(|&a| a != 0)
        };
        let sub = || {
            SUBSTITUTES
                .iter()
                .find(|&&(from, _)| from == ch)
                .and_then(|&(_, to)| lookup(to))
        };
        match lookup(ch).or_else(sub) {
            Some(units) => units as f64 / face.upem as f64,
            None => fallback_advance_em(ch),
        }
    }

    /// Cap height in em — the optical centring anchor [SPEC 5].
    #[allow(dead_code)] // reader lands with cap-height centring (M5)
    pub fn cap_height_em(&self) -> f64 {
        let face = self.face();
        face.cap_height as f64 / face.upem as f64
    }

    /// The numeric CSS weight (400/500/600/700).
    #[allow(dead_code)] // reader lands with `--embed-font` / `--static` (M5)
    pub fn weight(&self) -> u16 {
        [400, 500, 600, 700][self.weight]
    }
}

/// `font-weight` → static index [SPEC 6]: `normal | medium | semibold | bold`
/// or `400 | 500 | 600 | 700`; anything else (including the theme var's
/// unresolved default) reads as regular.
fn weight_index(value: Option<&ResolvedValue>) -> usize {
    match value {
        Some(ResolvedValue::Ident(w)) => match w.as_str() {
            "medium" => 1,
            "semibold" => 2,
            "bold" => 3,
            _ => 0,
        },
        Some(ResolvedValue::Number(n)) => match *n as u16 {
            500 => 1,
            600 => 2,
            700 => 3,
            _ => 0,
        },
        _ => 0,
    }
}

/// Index of a char in the concatenated [`CHARSET`] ranges, if covered.
fn charset_index(ch: char) -> Option<usize> {
    let cp = ch as u32;
    let mut base = 0usize;
    for &(start, end) in CHARSET {
        if (start..=end).contains(&cp) {
            return Some(base + (cp - start) as usize);
        }
        base += (end - start + 1) as usize;
    }
    None
}

/// Whether the subset TTF bytes are compiled in (the default-on `font`
/// feature). Name-only output never needs them; `--embed-font` / `--static`
/// error helpfully without them [SPEC 19].
#[allow(dead_code)] // reader lands with `--embed-font` / `--static` (M5)
pub const ENABLED: bool = cfg!(feature = "font");

/// The subset TTF for a family × weight — `--embed-font` inlines it,
/// `--static` outlines from it.
#[allow(dead_code)] // reader lands with `--embed-font` / `--static` (M5)
#[cfg(feature = "font")]
pub fn subset_bytes(kind: Kind, weight: u16) -> &'static [u8] {
    let w = match weight {
        500 => 1,
        600 => 2,
        700 => 3,
        _ => 0,
    };
    match kind {
        Kind::Mono => [
            include_bytes!("../../assets/fonts/subset/GoogleSansCode-Regular.ttf").as_slice(),
            include_bytes!("../../assets/fonts/subset/GoogleSansCode-Medium.ttf"),
            include_bytes!("../../assets/fonts/subset/GoogleSansCode-SemiBold.ttf"),
            include_bytes!("../../assets/fonts/subset/GoogleSansCode-Bold.ttf"),
        ][w],
        Kind::Prop => [
            include_bytes!("../../assets/fonts/subset/GoogleSans-Regular.ttf").as_slice(),
            include_bytes!("../../assets/fonts/subset/GoogleSans-Medium.ttf"),
            include_bytes!("../../assets/fonts/subset/GoogleSans-SemiBold.ttf"),
            include_bytes!("../../assets/fonts/subset/GoogleSans-Bold.ttf"),
        ][w],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mono invariant [SPEC 5]: every covered glyph advances exactly
    /// 0.6 em, at every weight — the flat historic estimate stays exact.
    #[test]
    fn mono_advances_are_uniformly_point_six_em() {
        for face in metrics::MONO {
            for &adv in face.advances {
                assert!(adv == 0 || (adv as f64 / face.upem as f64 - 0.6).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn kind_follows_the_name_not_the_table() {
        let kind = |s: &str| Kind::of_family(Some(&ResolvedValue::String(s.into())));
        assert_eq!(kind("Google Sans Code"), Kind::Mono);
        assert_eq!(kind("JetBrains Mono"), Kind::Mono);
        assert_eq!(kind("ui-monospace, SF Mono"), Kind::Mono);
        assert_eq!(kind("Google Sans"), Kind::Prop);
        assert_eq!(kind("Inter, system-ui"), Kind::Prop);
        assert_eq!(Kind::of_family(None), Kind::Mono);
    }

    #[test]
    fn advances_fall_back_for_unknown_glyphs() {
        let mono = Font::default();
        // The diameter sign measures as its substitute — 0.6 em in mono.
        assert!((mono.advance_em('⌀') - 0.6).abs() < 1e-12);
        // CJK is wide, other unknowns keep the flat estimate.
        assert!((mono.advance_em('你') - 1.0).abs() < 1e-12);
        assert!((mono.advance_em('Ж') - 0.6).abs() < 1e-12);
    }

    #[test]
    fn proportional_glyphs_differ() {
        let prop = Font {
            kind: Kind::Prop,
            weight: 0,
        };
        let (i, m) = (prop.advance_em('i'), prop.advance_em('M'));
        assert!(i < m, "i {i} vs M {m}");
    }

    #[test]
    fn every_face_has_a_cap_height() {
        for face in metrics::MONO.into_iter().chain(metrics::PROP) {
            assert!(face.cap_height > 0);
            assert!(face.ascent > 0 && face.descent < 0);
        }
    }
}
