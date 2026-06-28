//! `layout: chart` ([CHARTS.md]) — a container that reads all its children, fixes a
//! shared data→pixel scale, and **lowers to primitive `PlacedNode`s**. The renderer,
//! cascade, palette, theming, and `--bake-vars` are all reused unchanged; the chart
//! adds only the scale-and-place algorithm here.
//!
//! Step 1 covers categorical vertical `|bars|` with an auto value axis, gridlines,
//! x labels, a title, and a legend. Lines, dots, explicit axes, formulas, bands,
//! annotations, radial, and pie follow in later steps (see `PLAN.md`).

mod axis;
mod bars;
mod palette;
mod prim;
mod project;
mod scale;

use crate::error::Error;
use crate::layout::{Bbox, PlacedNode};
use crate::resolve::{AttrMap, NodeKind, ResolvedInst, ResolvedValue};
use crate::span::Span;
use bars::Series;
use project::Plot;
use scale::{Scale, fmt_tick};

const TITLE_SIZE: f64 = 13.0;
const LABEL_SIZE: f64 = 11.0;

/// Is this node a chart container ([CHARTS.md] §2)? Detected by its `layout:` attr —
/// the same key `read_layout_mode` owns — so a chart is intercepted before the
/// generic container path. (`layout: pie` is recognised in a later step.)
pub(super) fn is_chart(attrs: &AttrMap) -> bool {
    matches!(attrs.get("layout"), Some(ResolvedValue::Ident(s)) if s == "chart")
}

/// Lay a chart out into one `PlacedNode`: the chart box, carrying the lowered
/// gridlines / bars / labels / title / legend as its pre-positioned children.
pub(super) fn layout_chart(inst: &ResolvedInst) -> Result<PlacedNode, Error> {
    let span = inst.span;
    let w = inst.attrs.number("width").unwrap_or(360.0);
    let h = inst.attrs.number("height").unwrap_or(220.0);

    let (series_insts, title) = collect(inst)?;
    let categories = read_categories(&inst.attrs, span)?;
    let series = read_series(&series_insts, &categories, span)?;
    if series.is_empty() {
        return Err(Error::at(span, "a chart needs at least one series"));
    }
    let n = categories
        .as_ref()
        .map(Vec::len)
        .unwrap_or_else(|| series.iter().map(|s| s.values.len()).max().unwrap_or(0));
    if n == 0 {
        return Err(Error::at(
            span,
            "a chart series needs at least one data value",
        ));
    }

    let vmax = series
        .iter()
        .flat_map(|s| s.values.iter().copied())
        .fold(0.0_f64, f64::max);
    let scale = Scale::nice(vmax);

    // Plot rect = the chart box inset by the gutters its labels / title / legend
    // need, all measured at compile time (SPEC §6).
    let cats = categories.clone().unwrap_or_default();
    let legend = legend_entries(&series);
    let title_h = if title.is_some() {
        TITLE_SIZE * 1.4
    } else {
        0.0
    };
    let x_label_h = LABEL_SIZE * 1.4;
    let legend_h = if legend.len() >= 2 {
        LABEL_SIZE * 1.6
    } else {
        0.0
    };
    let left = scale
        .ticks
        .iter()
        .map(|t| prim::text_width(&fmt_tick(*t), LABEL_SIZE))
        .fold(0.0_f64, f64::max)
        + 10.0;
    let plot = Plot {
        x0: -w / 2.0 + left,
        x1: w / 2.0 - 12.0,
        y0: -h / 2.0 + 8.0 + title_h,
        y1: h / 2.0 - 6.0 - x_label_h - legend_h,
        n,
        vmax: scale.max,
    };

    // Semantic draw order ([CHARTS.md] §15): gridlines, bars, labels, title, legend —
    // pushed in order, so a later child paints over an earlier one.
    let mut kids = Vec::new();
    axis::value_axis(&plot, &scale, &mut kids);
    bars::lay_out(&plot, &series, &cats, &mut kids);
    axis::x_labels(&plot, &cats, &mut kids);
    if let Some(t) = &title {
        kids.push(prim::text(
            t,
            0.0,
            -h / 2.0 + 8.0 + TITLE_SIZE * 0.7,
            TITLE_SIZE,
            None,
            true,
        ));
    }
    if legend.len() >= 2 {
        lay_out_legend(&legend, plot.y1 + x_label_h + legend_h * 0.5, &mut kids);
    }

    Ok(PlacedNode {
        id: inst.id.clone(),
        kind: NodeKind::Block,
        type_chain: inst.type_chain.clone(),
        applied_styles: inst.applied_styles.clone(),
        label: None,
        attrs: inst.attrs.clone(),
        own_style: AttrMap::new(),
        markers: inst.markers.clone(),
        cx: 0.0,
        cy: 0.0,
        bbox: Bbox::centered(w, h),
        rotation: inst.attrs.number("rotate").unwrap_or(0.0),
        children: kids,
        dividers: Vec::new(),
        span,
    })
}

/// Split the chart's resolved children into series instances and the (optional)
/// title harvested from the chart's own smart-label text child ([CHARTS.md] §9).
fn collect(inst: &ResolvedInst) -> Result<(Vec<&ResolvedInst>, Option<String>), Error> {
    let mut series = Vec::new();
    let mut title = None;
    for child in &inst.children {
        if child.kind == NodeKind::Text {
            if title.is_none() {
                title = child
                    .label
                    .as_deref()
                    .filter(|t| !t.is_empty())
                    .map(str::to_string);
            }
            continue;
        }
        match series_tag(child) {
            Some("bars") => series.push(child),
            Some(other) => {
                return Err(Error::at(
                    child.span,
                    format!("'|{other}|' arrives in a later charts step"),
                ));
            }
            None => {
                return Err(Error::at(
                    child.span,
                    "a chart's children are series (e.g. '|bars|')",
                ));
            }
        }
    }
    Ok((series, title))
}

/// The chart type tag a child carries — its `type_chain` entry, or `line` for the
/// reused `|line|` primitive — for series dispatch.
fn series_tag(inst: &ResolvedInst) -> Option<&str> {
    const TAGS: &[&str] = &[
        "line", "area", "bars", "dots", "bubble", "slice", "axis", "band", "mark",
    ];
    if inst.kind == NodeKind::Line {
        return Some("line");
    }
    inst.type_chain
        .iter()
        .rev()
        .find_map(|t| TAGS.iter().copied().find(|&tag| tag == t))
}

/// The `categories:` strings (each a quoted-string value), or `None` for index ticks.
fn read_categories(attrs: &AttrMap, span: Span) -> Result<Option<Vec<String>>, Error> {
    let Some(v) = attrs.get("categories") else {
        return Ok(None);
    };
    let mut out = Vec::new();
    collect_strings(v, &mut out, span)?;
    Ok(Some(out))
}

fn collect_strings(v: &ResolvedValue, out: &mut Vec<String>, span: Span) -> Result<(), Error> {
    match v {
        ResolvedValue::String(s) => out.push(s.clone()),
        ResolvedValue::Tuple(items) | ResolvedValue::List(items) => {
            for it in items {
                collect_strings(it, out, span)?;
            }
        }
        _ => return Err(Error::at(span, "'categories' is a list of quoted strings")),
    }
    Ok(())
}

/// Read each series' values + colour ([CHARTS.md] §3/§4/§10). Step 1 handles `data:`
/// (categorical); a `fn:` series resolves to a held `Deferred` whose sampling needs
/// an x-axis (a later step). Colour is an explicit `fill:` else the palette walk.
fn read_series(
    insts: &[&ResolvedInst],
    categories: &Option<Vec<String>>,
    _span: Span,
) -> Result<Vec<Series>, Error> {
    let mut out = Vec::with_capacity(insts.len());
    for (i, inst) in insts.iter().enumerate() {
        let has_data = inst.attrs.get("data").is_some();
        let has_fn = matches!(inst.attrs.get("fn"), Some(ResolvedValue::Deferred(_)));
        let values = match (has_data, has_fn) {
            (true, true) => {
                return Err(Error::at(
                    inst.span,
                    "a series takes 'data' or 'fn', not both",
                ));
            }
            (false, false) => return Err(Error::at(inst.span, "a series needs 'data' or 'fn'")),
            (false, true) => {
                return Err(Error::at(
                    inst.span,
                    "a computed 'fn:' series needs an axis — added in a later charts step",
                ));
            }
            (true, false) => read_data(inst)?,
        };
        if let Some(cats) = categories
            && values.len() != cats.len()
        {
            return Err(Error::at(
                inst.span,
                format!(
                    "series data has {} values but the chart has {} categories",
                    values.len(),
                    cats.len()
                ),
            ));
        }
        let color = explicit(&inst.attrs, "fill").unwrap_or_else(|| ResolvedValue::LiveVar {
            name: palette::hue(i).to_string(),
            raw: false,
        });
        out.push(Series {
            values,
            label: label_of(inst),
            color,
        });
    }
    Ok(out)
}

/// Categorical `data:` → a value list. `|bars|` is categorical only, so `x y` points
/// (a `List`) are rejected.
fn read_data(inst: &ResolvedInst) -> Result<Vec<f64>, Error> {
    match inst.attrs.get("data") {
        Some(ResolvedValue::Number(n)) => Ok(vec![*n]),
        Some(ResolvedValue::Tuple(items)) => {
            let mut v = Vec::with_capacity(items.len());
            for it in items {
                match it {
                    ResolvedValue::Number(n) => v.push(*n),
                    _ => return Err(Error::at(inst.span, "'data' values must be numbers")),
                }
            }
            Ok(v)
        }
        Some(ResolvedValue::List(_)) => Err(Error::at(
            inst.span,
            "'|bars|' takes categorical data ('data: 9 15 24'), not 'x y' points",
        )),
        _ => Err(Error::at(inst.span, "'data' must be a list of numbers")),
    }
}

/// A series' legend label — its harvested smart-label text child ([CHARTS.md] §9).
fn label_of(inst: &ResolvedInst) -> Option<String> {
    inst.children
        .iter()
        .find(|c| c.kind == NodeKind::Text)
        .and_then(|c| c.label.as_deref())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
}

/// An *explicit* paint value — present and not the inherited `|block|` default
/// `none`, so it overrides the palette walk ([CHARTS.md] §10).
fn explicit(attrs: &AttrMap, name: &str) -> Option<ResolvedValue> {
    match attrs.get(name) {
        Some(ResolvedValue::Ident(s)) if s == "none" => None,
        other => other.cloned(),
    }
}

/// The legend entries — one per series that carries a label (no label → no entry,
/// [CHARTS.md] §9).
fn legend_entries(series: &[Series]) -> Vec<(String, ResolvedValue)> {
    series
        .iter()
        .filter_map(|s| s.label.clone().map(|l| (l, s.color.clone())))
        .collect()
}

/// A centred row of swatch + label entries at vertical `cy`.
fn lay_out_legend(entries: &[(String, ResolvedValue)], cy: f64, out: &mut Vec<PlacedNode>) {
    const SW: f64 = 11.0; // swatch side
    const GAP: f64 = 5.0; // swatch → label
    const ITEM_GAP: f64 = 16.0; // entry → entry
    let widths: Vec<f64> = entries
        .iter()
        .map(|(l, _)| prim::text_width(l, LABEL_SIZE))
        .collect();
    let per: f64 = widths.iter().map(|w| SW + GAP + w).sum();
    let total = per + ITEM_GAP * widths.len().saturating_sub(1) as f64;
    let mut x = -total / 2.0;
    for ((label, color), &tw) in entries.iter().zip(&widths) {
        out.push(prim::rect(x + SW / 2.0, cy, SW, SW, color.clone()));
        out.push(prim::text(
            label,
            x + SW + GAP + tw / 2.0,
            cy,
            LABEL_SIZE,
            None,
            false,
        ));
        x += SW + GAP + tw + ITEM_GAP;
    }
}

#[cfg(test)]
mod tests {
    /// Live-mode SVG for a source (palette vars stay `var(--lini-…)`).
    fn svg(src: &str) -> String {
        crate::compile_str(src).expect("compile")
    }

    /// The layout-phase error message for a chart that resolves but won't lay out.
    fn layout_err(src: &str) -> String {
        let toks = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(&toks).expect("parse");
        let lowered = crate::desugar::desugar(&file).expect("desugar");
        let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
        crate::layout::layout(&program)
            .err()
            .expect("expected a layout error")
            .to_string()
    }

    #[test]
    fn bars_chart_lowers_to_axis_bars_legend_and_title() {
        let s = svg(
            "|chart| \"T\" { categories: \"a\" \"b\" } [\n  |bars| \"S1\" { data: 3 6 }\n  |bars| \"S2\" { data: 4 2 }\n]\n",
        );
        assert!(s.contains("lini-chart"), "chart container class: {s}");
        // The palette walk colours series 0 rose, series 1 orange — red skipped.
        assert!(s.contains("var(--lini-rose)"), "series 0 hue: {s}");
        assert!(s.contains("var(--lini-orange)"), "series 1 hue: {s}");
        assert!(!s.contains("var(--lini-red)"), "red is reserved: {s}");
        // Gridlines paint with the faint grid role var.
        assert!(s.contains("var(--lini-grid)"), "gridlines: {s}");
        // The `<title>` tooltip floor on a bar, the title, and a tick label.
        assert!(s.contains("<title>a · S1: 3</title>"), "bar title: {s}");
        assert!(s.contains(">T</text>"), "chart title text: {s}");
        assert!(
            s.contains(">25</text>") || s.contains(">6</text>"),
            "value tick: {s}"
        );
    }

    #[test]
    fn an_explicit_fill_overrides_the_palette_walk() {
        let s = svg("|chart| { categories: \"a\" } [\n  |bars| { data: 5; fill: --teal }\n]\n");
        assert!(s.contains("var(--lini-teal)"), "explicit fill kept: {s}");
        assert!(!s.contains("var(--lini-rose)"), "palette not walked: {s}");
    }

    #[test]
    fn empty_chart_errors() {
        assert!(layout_err("|chart| \"T\"\n").contains("at least one series"));
    }

    #[test]
    fn data_count_must_match_categories() {
        let e = layout_err("|chart| { categories: \"a\" \"b\" } [\n  |bars| { data: 1 2 3 }\n]\n");
        assert!(e.contains("3 values but the chart has 2 categories"), "{e}");
    }

    #[test]
    fn data_and_fn_together_error() {
        let e = layout_err("|chart| { categories: \"a\" } [\n  |bars| { data: 1; fn: `2` }\n]\n");
        assert!(e.contains("not both"), "{e}");
    }

    #[test]
    fn a_series_needs_data_or_fn() {
        let e = layout_err("|chart| { categories: \"a\" } [\n  |bars| { }\n]\n");
        assert!(e.contains("needs 'data' or 'fn'"), "{e}");
    }

    #[test]
    fn a_non_series_child_is_rejected() {
        let e = layout_err("|chart| [\n  |box| \"x\"\n]\n");
        assert!(e.contains("series"), "{e}");
    }
}
