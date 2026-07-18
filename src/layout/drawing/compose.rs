//! Dimension text composition [SPEC 15.6] — each source owns one thing: the
//! **op** the glyph (`⌀` / `R` / `°`), the **geometry** the number, the
//! **label** the words (two-ended: replaces the number; one-ended: follows
//! it), **`tol:`** the tolerance, **`pattern:`** the `N×` count prefix, and
//! (`unit:` is the semantic quantity only — no per-value suffix; a glyph reading is
//! symbol-speak — the SPEC 24 dims read `300 mm` but `⌀20 h6`).

use super::super::ir::PlacedNode;
use super::super::{approx_width, prim};
use super::anchors::rotated;
use super::geometry::P;
use crate::error::Error;
use crate::ledger::consts::TOL_STACK;
use crate::ledger::format::{self, Format};
use crate::resolve::{AttrMap, ResolvedValue};
use crate::span::Span;

/// The measured reading's symbol [SPEC 15.6]: the feature picks it.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Glyph {
    /// A linear span — a bare number (`unit:` applies).
    None,
    /// A diameter — `⌀`, glued before the number.
    Dia,
    /// A radius — `R`, glued before the number.
    R,
    /// An angle — `°`, glued after the number.
    Deg,
}

/// A composed dimension text: the main run, an optional `fraction D` drafting
/// stack (raised numerator, slash, lowered denominator — the same raised /
/// lowered machinery as `tol:` deviations [SPEC 15.6]) with the runs that
/// compose after it in `tail`, plus the deviation pair of a `tol: +u -l`
/// (drawn at 0.7 × font [SPEC 15.6]).
pub(super) struct DimText {
    pub main: String,
    pub frac: Option<(String, String)>,
    pub tail: String,
    pub devs: Option<(String, String)>,
}

/// Air between the main run and its deviation stack, px.
const DEV_PAD: f64 = 2.0;

/// Air between a leading run and the fraction stack's numerator, px.
const FRAC_PAD: f64 = 1.0;

/// Compose one dimension's text from its sources [SPEC 15.6]. `format:`
/// shapes the **number** only — the glyph, the label's words, `tol:`, and the
/// `N×` count compose around the formatted number.
#[allow(clippy::too_many_arguments)]
pub(super) fn compose(
    glyph: Glyph,
    value: f64,
    count: Option<usize>,
    replaces: Option<&str>,
    follows: Option<&str>,
    attrs: &AttrMap,
    span: Span,
) -> Result<DimText, Error> {
    let f = format::read_or(attrs, Format::Auto, span)?;
    // A dimension is never a date — the chart consumers' gate [SPEC 16].
    if matches!(f, Format::Date(_)) {
        return Err(Error::at(span, "a date preset reads a time axis"));
    }
    let mut main = String::new();
    let mut frac = None;
    if let Some(n) = count {
        main.push_str(&format!("{n}× "));
    }
    match replaces {
        Some(label) => main.push_str(label),
        None => {
            // A bare number: drafting states units once, in the title block —
            // the presentation is `format:`'s [SPEC 15.6/16].
            match glyph {
                Glyph::Dia => main.push('⌀'),
                Glyph::R => main.push('R'),
                Glyph::None | Glyph::Deg => {}
            }
            match f {
                Format::Fraction(den) => match format::fraction_parts(value, den) {
                    // A whole reading stays a plain number.
                    (.., 0, _) => main.push_str(&format::render(value, f)),
                    (neg, whole, num, d) => {
                        if neg {
                            main.push('-');
                        }
                        if whole > 0 {
                            main.push_str(&whole.to_string());
                        }
                        frac = Some((num.to_string(), d.to_string()));
                    }
                },
                _ => main.push_str(&number(value, f)),
            }
        }
    }
    // The runs after the number land after the stack when one is standing.
    let mut tail = String::new();
    {
        let dest = if frac.is_some() { &mut tail } else { &mut main };
        if glyph == Glyph::Deg && replaces.is_none() {
            dest.push('°');
        }
        if let Some(label) = follows {
            dest.push(' ');
            dest.push_str(label);
        }
    }
    let devs = tol(
        if frac.is_some() { &mut tail } else { &mut main },
        attrs,
        span,
    )?;
    if devs.is_some() && frac.is_some() {
        return Err(Error::at(
            span,
            "'tol: +upper -lower' stacks where 'format: fraction' already stacks — \
             use a numeric 'tol' or a decimal format",
        ));
    }
    Ok(DimText {
        main,
        frac,
        tail,
        devs,
    })
}

/// The measured number under the cascaded `format:` [SPEC 15.6]: `auto` is
/// the drafting precision rule ([`fmt`], ≤ 2 decimals) — an explicit family
/// renders the exact value through the one engine.
fn number(v: f64, f: Format) -> String {
    match f {
        Format::Auto | Format::Date(_) => fmt(v),
        f => format::render(v, f),
    }
}

/// `tol:` [SPEC 15.6] — a number (`±0.1`, appended), `+upper -lower` (the
/// stacked deviation pair), or a fit ident (`H7`, appended).
fn tol(main: &mut String, attrs: &AttrMap, span: Span) -> Result<Option<(String, String)>, Error> {
    let bad = || {
        Error::at(
            span,
            "'tol' takes a number, '+upper -lower', or a fit ident",
        )
    };
    match attrs.get("tol") {
        None => Ok(None),
        Some(ResolvedValue::Number(t)) => {
            main.push_str(&format!("±{}", fmt(t.abs())));
            Ok(None)
        }
        Some(ResolvedValue::Ident(fit)) => {
            main.push(' ');
            main.push_str(fit);
            Ok(None)
        }
        Some(ResolvedValue::Tuple(pair)) => {
            let (Some(u), Some(l)) = (
                pair.first().and_then(ResolvedValue::as_number),
                pair.get(1).and_then(ResolvedValue::as_number),
            ) else {
                return Err(bad());
            };
            if pair.len() != 2 {
                return Err(bad());
            }
            Ok(Some((signed(u), signed(l))))
        }
        Some(_) => Err(bad()),
    }
}

fn signed(v: f64) -> String {
    if v > 0.0 {
        format!("+{}", fmt(v))
    } else {
        fmt(v)
    }
}

/// A composed section / detail view title [SPEC 15.8]: the uppercased letter —
/// **doubled** for a section (`A-A`), single for a detail (`C`) — then the
/// drafting ratio in parentheses — the view's authored `scale:` (the ratio,
/// default 1) read directly [SPEC 15.1/15.8]: a magnified view reads `2:1`,
/// a reduced one `1:1.5`.
pub(super) fn section_title(kind: &str, letter: &str, ratio: f64) -> String {
    let l = letter.to_uppercase();
    let head = if kind == "detail" {
        l
    } else {
        format!("{l}-{l}")
    };
    format!("{head} ({})", ratio_text(ratio))
}

/// A drafting ratio normalised so one side is 1 [SPEC 15.8]: an enlargement
/// `r ≥ 1` reads `r:1`, a reduction `1:1/r`; each side at most 2 dp.
fn ratio_text(r: f64) -> String {
    if r >= 1.0 {
        format!("{}:1", fmt(r))
    } else {
        format!("1:{}", fmt(1.0 / r))
    }
}

/// A measured value at drafting precision [SPEC 15.6]: at most 2 decimals,
/// trailing zeros trimmed.
pub(super) fn fmt(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    let r = if r == 0.0 { 0.0 } else { r };
    let mut s = format!("{r:.2}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}

impl DimText {
    /// The fraction stack's runs, left to right: `(content, size factor,
    /// raise factor)` — numerator raised, slash full-size, denominator
    /// lowered, then the tail (`°`, a following label, an appended `tol:`).
    fn frac_runs(&self) -> Vec<(&str, f64, f64)> {
        let Some((num, den)) = &self.frac else {
            return Vec::new();
        };
        let mut runs = vec![
            (num.as_str(), TOL_STACK, -0.55),
            ("/", 1.0, 0.0),
            (den.as_str(), TOL_STACK, 0.55),
        ];
        if !self.tail.is_empty() {
            runs.push((self.tail.as_str(), 1.0, 0.0));
        }
        runs
    }

    /// The drawn width of the whole run — main plus the fraction stack plus
    /// the deviation stack.
    pub fn width(&self, fs: f64, font: crate::font::Font) -> f64 {
        let mut main = approx_width(&self.main, font, fs, 0.0);
        if self.frac.is_some() {
            if !self.main.is_empty() {
                main += FRAC_PAD;
            }
            for (t, k, _) in self.frac_runs() {
                main += approx_width(t, font, fs * k, 0.0);
            }
        }
        match &self.devs {
            None => main,
            Some((u, l)) => {
                let dfs = fs * TOL_STACK;
                main + DEV_PAD
                    + approx_width(u, font, dfs, 0.0).max(approx_width(l, font, dfs, 0.0))
            }
        }
    }

    /// Lower to text nodes centred on `centre`, turned by `rot` (ISO-aligned
    /// text rotates with its dimension line [SPEC 15.6]). The fraction stack's
    /// raised / lowered runs follow the main run; deviations sit raised /
    /// lowered after everything, in the rotated frame.
    pub fn nodes(&self, centre: P, rot: f64, fs: f64, font: crate::font::Font) -> Vec<PlacedNode> {
        let place = |content: &str, local: P, size: f64| {
            let p = rotated(local, rot);
            let mut n = prim::dim_text(content, centre.0 + p.0, centre.1 + p.1, size, font.kind);
            if rot != 0.0 {
                n.rotation = rot;
                n.attrs.insert("rotate", ResolvedValue::Number(rot));
            }
            n
        };
        if self.frac.is_some() {
            // The drafting stack (devs never join it — compose errors).
            let total = self.width(fs, font);
            let mut x = -total / 2.0;
            let mut out = Vec::new();
            if !self.main.is_empty() {
                let wm = approx_width(&self.main, font, fs, 0.0);
                out.push(place(&self.main, (x + wm / 2.0, 0.0), fs));
                x += wm + FRAC_PAD;
            }
            for (t, k, raise) in self.frac_runs() {
                let w = approx_width(t, font, fs * k, 0.0);
                out.push(place(t, (x + w / 2.0, fs * TOL_STACK * raise), fs * k));
                x += w;
            }
            return out;
        }
        let Some((u, l)) = &self.devs else {
            return vec![place(&self.main, (0.0, 0.0), fs)];
        };
        let dfs = fs * TOL_STACK;
        let wm = approx_width(&self.main, font, fs, 0.0);
        let wd = approx_width(u, font, dfs, 0.0).max(approx_width(l, font, dfs, 0.0));
        let total = wm + DEV_PAD + wd;
        let dev_x = total / 2.0 - wd / 2.0;
        vec![
            place(&self.main, (wm / 2.0 - total / 2.0, 0.0), fs),
            place(u, (dev_x, -dfs * 0.55), dfs),
            place(l, (dev_x, dfs * 0.55), dfs),
        ]
    }
}
