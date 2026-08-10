use lini::Options;

fn render_live(src: &str) -> String {
    lini::compile_str(src).expect("compile")
}

fn render_baked(src: &str) -> String {
    lini::compile_str_with(
        src,
        &Options {
            static_mode: true,
            ..Default::default()
        },
    )
    .expect("compile")
}

fn render_themed(src: &str, theme_css: &str) -> String {
    lini::compile_str_with(
        src,
        &Options {
            theme_css: Some(theme_css.to_string()),
            ..Default::default()
        },
    )
    .expect("compile")
}

fn lini_root_rule(svg: &str) -> String {
    svg.lines()
        .find(|l| l.trim_start().starts_with(".lini {"))
        .expect(".lini root rule")
        .to_string()
}

/// Every `marker`-introduced value in an SVG, in document order, each read to
/// the next `end`. The one scrape every assertion below shares: `attr="` for
/// an attribute value, a full opening tag for an element's text.
fn scrape_to<'a>(svg: &'a str, marker: &str, end: char) -> Vec<&'a str> {
    svg.match_indices(marker)
        .map(|(i, m)| {
            let rest = &svg[i + m.len()..];
            &rest[..rest.find(end).unwrap_or(rest.len())]
        })
        .collect()
}

/// [`scrape_to`] for the common case — an attribute value, closed by its quote.
fn scrape<'a>(svg: &'a str, marker: &str) -> Vec<&'a str> {
    scrape_to(svg, marker, '"')
}

/// The drawn link lines of an SVG, split into (dashed, solid) `data-to` targets.
fn link_targets(svg: &str) -> (Vec<&str>, Vec<&str>) {
    let (mut dashed, mut solid) = (Vec::new(), Vec::new());
    for l in svg.lines() {
        let Some(&to) = scrape(l, "data-to=\"").first() else {
            continue;
        };
        if l.contains("lini-link-dashed") {
            dashed.push(to);
        } else if l.contains("lini-link") {
            solid.push(to);
        }
    }
    (dashed, solid)
}

/// The x-axis tick labels of a compiled chart, in document order: muted tick
/// text nodes, minus the value-axis ticks (small numbers — the test data keeps
/// its values < 1900 so year ticks stay).
fn x_tick_texts(svg: &str) -> Vec<String> {
    scrape_to(
        svg,
        "var(--lini-muted); font-size: 11px; font-weight: normal\">",
        '<',
    )
    .into_iter()
    .map(str::to_string)
    .filter(|t| t.parse::<f64>().map(|n| n >= 1900.0).unwrap_or(true))
    .collect()
}

/// Compile with local `src:` paths anchored at `samples/` (the committed
/// assets) and an optional traversal boundary.
fn render_assets(src: &str, root: Option<&str>) -> Result<String, lini::Error> {
    lini::compile_str_with(
        src,
        &Options {
            base_dir: Some("samples".into()),
            asset_root: root.map(Into::into),
            ..Default::default()
        },
    )
}

mod assets;
mod charts;
mod links;
mod paint;
mod shapes;
mod text;
