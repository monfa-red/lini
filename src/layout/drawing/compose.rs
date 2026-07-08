//! Dimension text composition [SPEC 15.6] — each source owns one thing: the
//! **op** the glyph (`⌀` / `R` / `°`), the **geometry** the number, the
//! **label** the words (two-ended: replaces the number; one-ended: follows
//! it), **`tol:`** the tolerance, **`pattern:`** the `N×` count prefix, and
//! `unit:` its suffix on auto-measured linear values (a glyph reading is
//! symbol-speak — the SPEC 24 dims read `300 mm` but `⌀20 h6`).

use super::super::ir::PlacedNode;
use super::super::{approx_width, prim};
use super::anchors::rotated;
use super::geometry::P;
use crate::error::Error;
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

/// A composed dimension text: the main run, plus the raised / lowered
/// deviation pair of a `tol: +u -l` (drawn at 0.7 × font [SPEC 15.6]).
pub(super) struct DimText {
    pub main: String,
    pub devs: Option<(String, String)>,
}

/// Stacked deviations draw at this fraction of the dimension font [SPEC 10.5].
const TOL_STACK: f64 = 0.7;
/// Air between the main run and its deviation stack, px.
const DEV_PAD: f64 = 2.0;

/// Compose one dimension's text from its sources [SPEC 15.6].
#[allow(clippy::too_many_arguments)]
pub(super) fn compose(
    glyph: Glyph,
    value: f64,
    count: Option<usize>,
    replaces: Option<&str>,
    follows: Option<&str>,
    attrs: &AttrMap,
    unit: Option<&str>,
    span: Span,
) -> Result<DimText, Error> {
    let mut main = String::new();
    if let Some(n) = count {
        main.push_str(&format!("{n}× "));
    }
    match replaces {
        Some(label) => main.push_str(label),
        None => {
            match glyph {
                Glyph::None => main.push_str(&fmt(value)),
                Glyph::Dia => main.push_str(&format!("⌀{}", fmt(value))),
                Glyph::R => main.push_str(&format!("R{}", fmt(value))),
                Glyph::Deg => main.push_str(&format!("{}°", fmt(value))),
            }
            if glyph == Glyph::None
                && let Some(u) = unit
            {
                main.push(' ');
                main.push_str(u);
            }
        }
    }
    if let Some(label) = follows {
        main.push(' ');
        main.push_str(label);
    }
    let devs = tol(&mut main, attrs, span)?;
    Ok(DimText { main, devs })
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
/// drafting ratio in parentheses. `own` is the view's scale, `page` the
/// enclosing page's; a magnified view reads `2:1`, a reduced one `1:1.5`.
pub(super) fn section_title(kind: &str, letter: &str, own: f64, page: f64) -> String {
    let l = letter.to_uppercase();
    let head = if kind == "detail" {
        l
    } else {
        format!("{l}-{l}")
    };
    format!("{head} ({})", ratio(own, page))
}

/// The drafting scale ratio `own : page` [SPEC 15.8], normalised so one side is
/// 1: an enlargement `r ≥ 1` reads `r:1`, a reduction `1:1/r`; each side at
/// most 2 dp.
fn ratio(own: f64, page: f64) -> String {
    let r = own / page;
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
    /// The drawn width of the whole run — main plus the deviation stack.
    pub fn width(&self, fs: f64) -> f64 {
        let main = approx_width(&self.main, fs, 0.0);
        match &self.devs {
            None => main,
            Some((u, l)) => {
                let dfs = fs * TOL_STACK;
                main + DEV_PAD + approx_width(u, dfs, 0.0).max(approx_width(l, dfs, 0.0))
            }
        }
    }

    /// Lower to text nodes centred on `centre`, turned by `rot` (ISO-aligned
    /// text rotates with its dimension line [SPEC 15.6]). Deviations sit
    /// raised / lowered after the main run, in the rotated frame.
    pub fn nodes(&self, centre: P, rot: f64, fs: f64) -> Vec<PlacedNode> {
        let place = |content: &str, local: P, size: f64| {
            let p = rotated(local, rot);
            let mut n = prim::text(content, centre.0 + p.0, centre.1 + p.1, size, None, false);
            if rot != 0.0 {
                n.rotation = rot;
                n.attrs.insert("rotate", ResolvedValue::Number(rot));
            }
            n
        };
        let Some((u, l)) = &self.devs else {
            return vec![place(&self.main, (0.0, 0.0), fs)];
        };
        let dfs = fs * TOL_STACK;
        let wm = approx_width(&self.main, fs, 0.0);
        let wd = approx_width(u, dfs, 0.0).max(approx_width(l, dfs, 0.0));
        let total = wm + DEV_PAD + wd;
        let dev_x = total / 2.0 - wd / 2.0;
        vec![
            place(&self.main, (wm / 2.0 - total / 2.0, 0.0), fs),
            place(u, (dev_x, -dfs * 0.55), dfs),
            place(l, (dev_x, dfs * 0.55), dfs),
        ]
    }
}
