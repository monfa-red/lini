//! Bundled-font emission [SPEC 17]: `--embed-font` inlines a base64
//! `@font-face` per family × weight actually used (Lini-scoped names, so an
//! installed Google Sans never collides), and `--static` outlines text to
//! deduped glyph paths (`<defs>`/`<use>`) so the SVG is faithful in renderers
//! with no font at all. Both draw on the subset TTFs behind the default-on
//! `font` feature; without it these are inert stubs and text stays name-only.
//!
//! The scene body is rendered before the `<style>`/`<defs>` blocks are
//! assembled, so the one text emitter ([`super::text::emit`]) registers what
//! it used in a [`FontSink`] as it goes — the defs and `@font-face` blocks
//! then emit exactly that set, nothing re-walked, nothing drifting.

#![cfg_attr(not(feature = "font"), allow(unused_variables, dead_code))]

use crate::font::{Font, Kind};
use crate::layout::LaidOut;

/// The Lini-scoped `@font-face` family names [SPEC 17].
pub fn scoped_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Mono => "Lini Sans Code",
        Kind::Prop => "Lini Sans",
    }
}

/// `--embed-font`: lead a `font-family` stack with the scoped twin of its
/// first family when that family is a bundled one, so the embedded face wins
/// and the original name stays as the fallback.
pub fn lead_with_scoped(stack: &str) -> String {
    let first = stack.split(',').next().unwrap_or(stack).trim();
    let bare = first.trim_matches(['"', '\'']);
    match bare {
        "Google Sans Code" => format!("\"{}\", {}", scoped_name(Kind::Mono), stack),
        "Google Sans" => format!("\"{}\", {}", scoped_name(Kind::Prop), stack),
        _ => stack.to_string(),
    }
}

/// What the body's text emission used, collected as it renders: faces for
/// `--embed-font`, glyph ids for `--static` outlining. Also carries the root
/// text context so the emitter can resolve the inline → class → root cascade
/// without re-walking the tree.
pub struct FontSink {
    /// The root's effective font (authored global `font-family`/`font-weight`
    /// or the mono default) — the inherited tail of the cascade.
    pub root_font: Font,
    /// The root `font-size` (the `.lini` rule's baked literal).
    pub root_size: f64,
    #[cfg(feature = "font")]
    used: std::cell::RefCell<std::collections::BTreeSet<(u8, u16, u16)>>,
}

impl FontSink {
    pub fn new(laid: &LaidOut) -> FontSink {
        FontSink {
            root_font: Font::of(&laid.sheet.root_text),
            root_size: laid.sheet.root_font_size,
            #[cfg(feature = "font")]
            used: Default::default(),
        }
    }
}

#[cfg(feature = "font")]
mod enabled {
    use super::{FontSink, scoped_name};
    use crate::font::{self, Font, Kind};
    use std::fmt::Write;
    use std::sync::OnceLock;

    fn kind_tag(kind: Kind) -> u8 {
        match kind {
            Kind::Mono => 0,
            Kind::Prop => 1,
        }
    }
    fn kind_of_tag(tag: u8) -> Kind {
        if tag == 0 { Kind::Mono } else { Kind::Prop }
    }

    /// The parsed subset faces, one per family × weight, parsed once.
    fn face(kind: Kind, weight: u16) -> &'static ttf_parser::Face<'static> {
        static FACES: OnceLock<Vec<ttf_parser::Face<'static>>> = OnceLock::new();
        let faces = FACES.get_or_init(|| {
            let mut v = Vec::with_capacity(8);
            for kind in [Kind::Mono, Kind::Prop] {
                for w in [400u16, 500, 600, 700] {
                    v.push(
                        ttf_parser::Face::parse(font::subset_bytes(kind, w), 0)
                            .expect("bundled subset parses"),
                    );
                }
            }
            v
        });
        let wi = match weight {
            500 => 1,
            600 => 2,
            700 => 3,
            _ => 0,
        };
        &faces[kind_tag(kind) as usize * 4 + wi]
    }

    /// The glyph a char outlines as: the face's own, the substitute twin's
    /// ([SPEC 5] — the same chain measurement walks), else `.notdef` (0).
    pub fn glyph_id(f: Font, ch: char) -> u16 {
        let fc = face(f.kind, f.weight());
        let direct = |c: char| fc.glyph_index(c).map(|g| g.0);
        direct(ch)
            .or_else(|| {
                font::SUBSTITUTES
                    .iter()
                    .find(|&&(from, _)| from == ch)
                    .and_then(|&(_, to)| direct(to))
            })
            .unwrap_or(0)
    }

    impl FontSink {
        /// Register one glyph run's face (+ its glyph ids under `--static`).
        pub fn register(&self, f: Font, content: &str, glyphs: bool) {
            let mut used = self.used.borrow_mut();
            let (k, w) = (kind_tag(f.kind), f.weight());
            used.insert((k, w, 0)); // the face itself (gid 0 always subset-present)
            if glyphs {
                for ch in content.chars() {
                    used.insert((k, w, glyph_id(f, ch)));
                }
            }
        }
    }

    /// A registered glyph's stable def id.
    pub fn glyph_ref(f: Font, gid: u16) -> String {
        format!("lg{}{}-{}", kind_tag(f.kind), f.weight(), gid)
    }

    struct PathSink(String);
    impl ttf_parser::OutlineBuilder for PathSink {
        fn move_to(&mut self, x: f32, y: f32) {
            let _ = write!(self.0, "M{} {}", x, y);
        }
        fn line_to(&mut self, x: f32, y: f32) {
            let _ = write!(self.0, "L{} {}", x, y);
        }
        fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
            let _ = write!(self.0, "Q{} {} {} {}", x1, y1, x, y);
        }
        fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
            let _ = write!(self.0, "C{} {} {} {} {} {}", x1, y1, x2, y2, x, y);
        }
        fn close(&mut self) {
            self.0.push('Z');
        }
    }

    /// `--static`: one `<path>` def per used glyph, in font units (y-up — the
    /// `<use>` flips with `scale(s, -s)`), deduped across every text run.
    pub fn emit_glyph_defs(out: &mut String, sink: &FontSink) {
        for &(k, w, gid) in sink.used.borrow().iter() {
            let f = font_of(k, w);
            let mut d = PathSink(String::new());
            if face(f.kind, f.weight())
                .outline_glyph(ttf_parser::GlyphId(gid), &mut d)
                .is_none()
            {
                continue; // a blank glyph (space) — advances, draws nothing
            }
            writeln!(
                out,
                r##"    <path id="{}" d="{}"/>"##,
                glyph_ref(f, gid),
                d.0
            )
            .unwrap();
        }
    }

    fn font_of(k: u8, w: u16) -> Font {
        let kind = kind_of_tag(k);
        match w {
            500 => Font::medium(kind),
            600 => Font::semibold(kind),
            700 => Font::bold(kind),
            _ => Font::regular(kind),
        }
    }

    /// Whether any glyph def was registered (skip the defs group otherwise).
    pub fn has_glyphs(sink: &FontSink) -> bool {
        !sink.used.borrow().is_empty()
    }

    /// `--embed-font`: one `@font-face` per used face, Lini-scoped names,
    /// base64 subset bytes. Browser-only by design [SPEC 17].
    pub fn emit_font_faces(out: &mut String, sink: &FontSink) {
        let mut seen = std::collections::BTreeSet::new();
        for &(k, w, _) in sink.used.borrow().iter() {
            if !seen.insert((k, w)) {
                continue;
            }
            let kind = kind_of_tag(k);
            writeln!(
                out,
                "    @font-face {{ font-family: \"{}\"; font-weight: {}; src: url(data:font/ttf;base64,{}) format(\"truetype\"); }}",
                scoped_name(kind),
                w,
                base64(font::subset_bytes(kind, w)),
            )
            .unwrap();
        }
    }

    /// The underline / line-through band for a decorated run, from the face's
    /// own metrics: (offset of the band's centre below the baseline in px —
    /// negative = above, thickness in px).
    pub fn decoration_band(f: Font, size: f64, strike: bool) -> (f64, f64) {
        let fc = face(f.kind, f.weight());
        let upem = fc.units_per_em() as f64;
        let m = if strike {
            fc.strikeout_metrics()
        } else {
            fc.underline_metrics()
        };
        match m {
            Some(m) => (
                -(m.position as f64) / upem * size,
                m.thickness as f64 / upem * size,
            ),
            // No table: the CSS-ish fallback band.
            None if strike => (-0.3 * size, 0.05 * size),
            None => (0.1 * size, 0.05 * size),
        }
    }

    // Base64 is the asset embedder's (`resolve::assets::base64`) — the one
    // encoder for every data: URL [SPEC 17].
    use crate::resolve::assets::base64;

    /// The advance-scaled `x` positions each glyph of `line` starts at, when
    /// the run is centred on `cx` — the exact mirror of the measurement fold
    /// ([`crate::layout::approx_width`]), so outlined text sits where layout
    /// reserved room for it.
    pub fn glyph_starts(line: &str, f: Font, size: f64, ls: f64, cx: f64) -> Vec<(char, f64)> {
        let width = crate::layout::approx_width(line, f, size, ls);
        let mut pos = cx - width / 2.0;
        let mut out = Vec::with_capacity(line.chars().count());
        for (i, ch) in line.chars().enumerate() {
            if i > 0 {
                pos += ls;
            }
            out.push((ch, pos));
            pos += f.advance_em(ch) * size;
        }
        out
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn every_subset_face_parses_and_outlines() {
            for kind in [Kind::Mono, Kind::Prop] {
                for w in [400u16, 500, 600, 700] {
                    let f = font_of(kind_tag(kind), w);
                    let gid = glyph_id(f, 'A');
                    assert_ne!(gid, 0, "{kind:?} {w} lacks 'A'");
                    let mut d = PathSink(String::new());
                    face(kind, w)
                        .outline_glyph(ttf_parser::GlyphId(gid), &mut d)
                        .expect("outline");
                    assert!(d.0.starts_with('M'), "{}", d.0);
                }
            }
        }

        #[test]
        fn diameter_outlines_as_slashed_o() {
            let f = Font::default();
            assert_eq!(glyph_id(f, '⌀'), glyph_id(f, 'Ø'));
            assert_ne!(glyph_id(f, 'Ø'), 0);
        }

        #[test]
        fn glyph_starts_mirror_measurement() {
            let f = Font::default();
            let starts = glyph_starts("abc", f, 10.0, 0.0, 0.0);
            // 3 × 6 px wide, centred on 0 → starts at -9, -3, 3.
            let xs: Vec<f64> = starts.iter().map(|&(_, x)| x).collect();
            assert_eq!(xs, [-9.0, -3.0, 3.0]);
        }
    }
}

#[cfg(feature = "font")]
pub use enabled::*;

#[cfg(not(feature = "font"))]
mod disabled {
    use super::FontSink;
    use crate::font::Font;

    impl FontSink {
        pub fn register(&self, _f: Font, _content: &str, _glyphs: bool) {}
    }
    pub fn has_glyphs(_sink: &FontSink) -> bool {
        false
    }
    pub fn emit_glyph_defs(_out: &mut String, _sink: &FontSink) {}
    pub fn emit_font_faces(_out: &mut String, _sink: &FontSink) {}
}

#[cfg(not(feature = "font"))]
pub use disabled::*;
