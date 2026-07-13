//! Text-leaf emission [SPEC 3/16]: one `<text>` for a node's text **and** a
//! link's label — the single code path behind both, so a styleable text leaf
//! reads the same wherever it sits. It bakes `letter-spacing` into a per-glyph
//! `dx`, splits multi-line `\n` text into `<tspan>`s (leading `font-size × 1.2`
//! plus `line-spacing`), turns `rotate` into a `transform`, and centres each
//! line vertically by **cap height** [SPEC 5]: a baked `dy` of half the
//! measurement font's cap (em units, so a class-stated size scales it) drops
//! the baseline below the centre — optical centring that renders identically
//! everywhere, where `dominant-baseline` varies by renderer. `translate` is
//! folded into the placed centre upstream — a node at layout, a label at
//! routing — so it never reappears here.

use super::fonts::FontSink;
use super::rules::RuleSet;
use super::values::{escape_xml, num};
use crate::Options;
use crate::font::{Font, Kind};
use crate::layout::approx_width;
use crate::ledger::consts::TEXT_LEADING;
use crate::resolve::{AttrMap, ResolvedValue};
use std::fmt::Write;

/// The effective rendered font and size of a text leaf, resolved through the
/// same cascade CSS will apply: the leaf's own/inherited attrs, then its class
/// rules, then the root — so `--static` outlining and `--embed-font` face
/// collection see exactly what a browser would use. Values a class rule
/// states as an unresolved `var()` fall through to the root default.
fn effective_font(
    attrs: &AttrMap,
    classes: &[String],
    ruleset: &RuleSet,
    sink: &FontSink,
) -> (Font, f64) {
    let kind = match attrs.get("font-family") {
        Some(v) => Kind::of_family(Some(v)),
        None => match ruleset.provided(classes, &[], "font-family") {
            Some(css) => Kind::of_family(Some(&ResolvedValue::RawCss(css.into()))),
            None => sink.root_font.kind,
        },
    };
    let font = if attrs.get("font-weight").is_some() {
        Font::of(attrs).with_kind(kind)
    } else {
        match ruleset
            .provided(classes, &[], "font-weight")
            .map(str::trim)
            .unwrap_or("")
        {
            "500" | "medium" => Font::medium(kind),
            "600" | "semibold" => Font::semibold(kind),
            "700" | "bold" => Font::bold(kind),
            "400" | "normal" => Font::regular(kind),
            _ => sink.root_font.with_kind(kind),
        }
    };
    let size = attrs
        .number("font-size")
        .or_else(|| {
            ruleset
                .provided(classes, &[], "font-size")
                .and_then(|v| v.trim().trim_end_matches("px").parse().ok())
        })
        .unwrap_or(sink.root_size);
    (font, size)
}

/// Emit a `<text class="{class}">` centred on `(x, y)` — `text-anchor:
/// middle` comes from the class rule, the vertical centring from the baked
/// cap-height `dy`. `style` is the caller's precomputed paint/font `style=`
/// string (a node and a link diff against different rule defaults, so each
/// builds its own); everything geometric is shared. `attrs` are the
/// node/label's resolved attrs, read for the measurement font,
/// `letter-spacing`, `line-spacing`, `font-size`, and `rotate`.
#[allow(clippy::too_many_arguments)] // the text chokepoint: geometry + cascade + sinks
pub(crate) fn emit(
    out: &mut String,
    indent: &str,
    classes: &[String],
    content: &str,
    pos: (f64, f64),
    attrs: &AttrMap,
    style: &str,
    ruleset: &RuleSet,
    opts: &Options,
    sink: &FontSink,
) {
    let (font, size) = effective_font(attrs, classes, ruleset, sink);
    #[cfg(feature = "font")]
    if opts.static_mode {
        // `text-transform` is live CSS on a `<text>`, but outlined paths bake
        // the glyph choice — apply it before the defs registration so the
        // `<use>` references and the emitted defs agree.
        let content = apply_text_transform(content, attrs);
        sink.register(font, &content, true);
        return emit_outlined(
            out, indent, classes, &content, pos, attrs, style, font, size,
        );
    }
    if opts.embed_font {
        sink.register(font, content, false);
    }

    let class = classes.join(" ");
    let (x, y) = pos;
    let (xs, ys) = (num(x), num(y));
    // Half the cap height in em: baseline sits cap/2 below the centre, so the
    // cap-height box is optically centred on (x, y) [SPEC 5]. Em units track
    // the rendered size even when a class rule states it (dim text, labels).
    let cap_dy = format!(" dy=\"{}em\"", num(font.cap_height_em() / 2.0));
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
            r#"{indent}<text class="{class}" x="{xs}" y="{ys}"{cap_dy}{}{style}{xform}>{}</text>"#,
            dx_attr(content, ls),
            escape_xml(content),
        )
        .unwrap();
        return;
    }

    // Multi-line [SPEC 5]: one tspan per line, leading `font-size × 1.2` plus
    // `line-spacing`, the block centred on (x, y); each line's `y` carries the
    // cap-height half-shift (a tspan's explicit `y` resets the parent `dy`, so
    // the shift is folded in px — `font-size` is in attrs wherever multi-line
    // text exists, the same premise the leading already stands on). The
    // layout-stamped `line-align` [SPEC 6] anchors each line to the block's
    // edge: `text-anchor: middle` stands (the class rule), so a line's `x` is
    // its own centre — computed through the one measurement API. (The attrs
    // read shadows the cascade-resolved size on purpose: it is what the
    // measurement saw, byte-stable with the historic emission.)
    let size = attrs.number("font-size").unwrap_or(0.0);
    let spacing = size * TEXT_LEADING + attrs.number("line-spacing").unwrap_or(0.0);
    let top = y - spacing * (lines.len() as f64 - 1.0) / 2.0 + font.cap_height_em() / 2.0 * size;
    let line_x: Box<dyn Fn(&str) -> f64> = match attrs.get("line-align") {
        Some(ResolvedValue::Ident(a)) if a == "start" || a == "end" => {
            let font = crate::font::Font::of(attrs);
            let block = approx_width(content, font, size, ls);
            if a == "start" {
                let left = x - block / 2.0;
                Box::new(move |line| left + approx_width(line, font, size, ls) / 2.0)
            } else {
                let right = x + block / 2.0;
                Box::new(move |line| right - approx_width(line, font, size, ls) / 2.0)
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

/// `text-transform` applied at compile time — only for outlining, where no
/// live CSS can apply it later.
#[cfg(feature = "font")]
fn apply_text_transform(content: &str, attrs: &AttrMap) -> String {
    let t = match attrs.get("text-transform") {
        Some(ResolvedValue::Ident(s)) => s.as_str(),
        _ => "",
    };
    match t {
        "uppercase" => content.to_uppercase(),
        "lowercase" => content.to_lowercase(),
        "capitalize" => content
            .split_inclusive(char::is_whitespace)
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    None => String::new(),
                }
            })
            .collect(),
        _ => content.to_string(),
    }
}

/// The `--static` twin of the `<text>` path [SPEC 17]: each line becomes
/// `<use>` references to deduped glyph paths in `<defs>` (font units, y-up —
/// the per-use `scale(s, -s)` flips and sizes them), positioned by the same
/// measurement fold layout reserved room with. `text-transform` bakes into
/// the glyph choice, `text-decoration` becomes a filled band from the face's
/// own metrics, `font-style: italic` a synthetic oblique — live CSS can't
/// reach outlined paths, so their effects bake here.
#[cfg(feature = "font")]
#[allow(clippy::too_many_arguments)] // the text chokepoint's twin — same surface
fn emit_outlined(
    out: &mut String,
    indent: &str,
    classes: &[String],
    content: &str,
    pos: (f64, f64),
    attrs: &AttrMap,
    style: &str,
    font: Font,
    size: f64,
) {
    use super::fonts;

    let (x, y) = pos;
    let ls = attrs.number("letter-spacing").unwrap_or(0.0);
    let ident_of = |name: &str| match attrs.get(name) {
        Some(ResolvedValue::Ident(s)) => Some(s.as_str()),
        _ => None,
    };

    let mut xform = match attrs.number("rotate") {
        Some(r) if r != 0.0 => format!(" transform=\"rotate({} {} {})\"", num(r), num(x), num(y)),
        _ => String::new(),
    };
    // Synthetic oblique: shear each glyph about its own baseline origin —
    // identical to shearing the run about the shared baseline.
    let oblique = ident_of("font-style") == Some("italic");
    if oblique && xform.is_empty() {
        xform = String::new(); // per-glyph skew below; no run transform needed
    }

    writeln!(
        out,
        r#"{indent}<g class="{}"{style}{xform}>"#,
        classes.join(" ")
    )
    .unwrap();

    let lines: Vec<&str> = content.split('\n').collect();
    let spacing = size * TEXT_LEADING + attrs.number("line-spacing").unwrap_or(0.0);
    // First baseline: block centred on y, then the cap-height half-shift —
    // the same values the <text> path encodes as tspan tops and the em dy.
    let top = y - spacing * (lines.len() as f64 - 1.0) / 2.0 + font.cap_height_em() / 2.0 * size;
    let line_x: Box<dyn Fn(&str) -> f64> = match attrs.get("line-align") {
        Some(ResolvedValue::Ident(a)) if a == "start" || a == "end" => {
            let mfont = Font::of(attrs);
            let block = approx_width(content, mfont, size, ls);
            if a == "start" {
                let left = x - block / 2.0;
                Box::new(move |line| left + approx_width(line, mfont, size, ls) / 2.0)
            } else {
                let right = x + block / 2.0;
                Box::new(move |line| right - approx_width(line, mfont, size, ls) / 2.0)
            }
        }
        _ => Box::new(move |_| x),
    };

    let scale = size / font.face().upem as f64;
    let skew = if oblique { " skewX(-12)" } else { "" };
    for (i, line) in lines.iter().enumerate() {
        let baseline = top + spacing * i as f64;
        for (ch, gx) in fonts::glyph_starts(line, font, size, ls, line_x(line)) {
            if ch.is_whitespace() {
                continue;
            }
            writeln!(
                out,
                r##"{indent}  <use href="#{}" transform="translate({} {}){} scale({} {})"/>"##,
                fonts::glyph_ref(font, fonts::glyph_id(font, ch)),
                num(gx),
                num(baseline),
                skew,
                num(scale),
                num(-scale),
            )
            .unwrap();
        }
        // Decoration bands bake from the face's own metrics — filled rects,
        // so `fill: currentColor` paints them like the glyphs.
        let deco = ident_of("text-decoration");
        if matches!(deco, Some("underline") | Some("line-through")) {
            let strike = deco == Some("line-through");
            let (off, th) = fonts::decoration_band(font, size, strike);
            let w = approx_width(line, font, size, ls);
            let lx = line_x(line);
            writeln!(
                out,
                r#"{indent}  <rect x="{}" y="{}" width="{}" height="{}"/>"#,
                num(lx - w / 2.0),
                num(baseline + off - th / 2.0),
                num(w),
                num(th),
            )
            .unwrap();
        }
    }
    writeln!(out, "{indent}</g>").unwrap();
}
