mod ast;
mod desugar;
mod error;
mod expr;
mod fmt;
mod icon;
mod layout;
mod ledger;
mod lexer;
mod lint;
mod palette;
mod render;
mod resolve;
mod routing;
mod serve;
mod span;
mod suggest;
mod syntax;
mod theme;

pub use error::{Diagnostic, Error, Level};
pub use fmt::format as format_source;

/// Lower a source file's sugar to primitives + `.lini-*` classes and print canonical
/// `.lini` — what `lini desugar` shows: every typed instance becomes a `|primitive|`
/// wearing its `.lini-*` chain, defines and templates collapse into generated
/// `.lini-*` class defs, scene/link defaults fill the global block, and labels /
/// `along:` become explicit. Comments are dropped. The lowered form re-renders
/// identically and is a fixed point of desugar.
pub fn desugar_source(src: &str) -> Result<String, Error> {
    let tokens = lexer::lex(src)?;
    let file = syntax::parser::parse(src, &tokens)?;
    Ok(fmt::print_file(&desugar::desugar(&file)?))
}
pub use routing::{Rule, Severity, Violation};
pub use serve::{ServeTarget, serve};
pub use theme::{builtin_css, extract_lini_vars, list_themes, pair_css};

/// Top-level compile options threaded through every phase. Build with
/// `Options::default()` and override fields with the struct-update syntax —
/// future versions may add knobs.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Emit `var()` values inline as their resolved literal so renderers
    /// without CSS-variable support (resvg, librsvg, image converters) still
    /// display the diagram correctly. The structural class rules stay; only the
    /// `@layer` variable defaults are dropped (their values are inlined).
    pub bake_vars: bool,
    /// Output wrapper format.
    pub format: OutputFormat,
    /// Raw CSS text whose `--lini-*` declarations override built-in defaults
    /// before the `defaults {}` block. `extract_lini_vars` does the parse.
    pub theme_css: Option<String>,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    #[default]
    Svg,
    Html,
}

pub fn compile_str(src: &str) -> Result<String, Error> {
    compile_str_with(src, &Options::default())
}

pub fn compile_str_with(src: &str, opts: &Options) -> Result<String, Error> {
    let program = resolve_pipeline(src, opts)?;
    let mut laid_out = layout::layout(&program)?;
    render::lower_paints(&mut laid_out);
    Ok(finish_svg(&laid_out, opts))
}

/// Compile to SVG **and** collect the routing diagnostics in a single layout
/// pass. The CLI's default path needs both (the SVG to emit, the diagnostics
/// to warn); routing through here runs the link router once instead of twice.
pub fn compile_str_checked(src: &str, opts: &Options) -> Result<(String, Vec<Diagnostic>), Error> {
    let program = resolve_pipeline(src, opts)?;
    let mut laid_out = layout::layout(&program)?;
    render::lower_paints(&mut laid_out);
    let diags = routing_diagnostics_of(layout::validate_routing(&laid_out));
    Ok((finish_svg(&laid_out, opts), diags))
}

fn finish_svg(laid_out: &layout::LaidOut, opts: &Options) -> String {
    let svg = render::render(laid_out, opts);
    match opts.format {
        OutputFormat::Svg => svg,
        OutputFormat::Html => wrap_html(&svg),
    }
}

/// Lex and parse only — verifies syntactic correctness without running
/// resolve/layout/render.
pub fn check_parse(src: &str) -> Result<(), Error> {
    let tokens = lexer::lex(src)?;
    let _file = syntax::parser::parse(src, &tokens)?;
    Ok(())
}

/// Lex, parse, and run the lint pass. Returns warnings (no errors).
/// Parse errors are surfaced as `Err`; missing lints just return an empty Vec.
pub fn lint_str(src: &str) -> Result<Vec<Diagnostic>, Error> {
    let tokens = lexer::lex(src)?;
    let file = syntax::parser::parse(src, &tokens)?;
    Ok(lint::lint(&file))
}

/// Lex, parse, and resolve. Verifies semantic correctness without running
/// layout or render. The CLI's `--check` flag goes through here.
pub fn check(src: &str) -> Result<(), Error> {
    check_with(src, &Options::default())
}

pub fn check_with(src: &str, opts: &Options) -> Result<(), Error> {
    let _ = resolve_pipeline(src, opts)?;
    Ok(())
}

/// Lex, parse, resolve, lay out, route, then validate the routing against the
/// contract in ROUTING.md. Returns the violations found (empty = clean). Parse
/// and resolve errors surface as `Err`.
pub fn validate_str(src: &str) -> Result<Vec<Violation>, Error> {
    let program = resolve_pipeline(src, &Options::default())?;
    let laid_out = layout::layout(&program)?;
    Ok(layout::validate_routing(&laid_out))
}

/// Surface routing violations as user-facing diagnostics. Crossings are normal,
/// counted output (`Info`) and stay silent here; everything else — an impossible
/// link, or a law breach (which would mean an engine bug) — is flagged, never
/// silent. The CLI prints these as warnings; `--strict` makes them fail the build.
fn routing_diagnostics_of(violations: Vec<Violation>) -> Vec<Diagnostic> {
    violations
        .into_iter()
        .filter(|v| v.severity != Severity::Info)
        .map(|v| {
            Diagnostic::warn(
                v.span,
                format!("{} ({}): {}", v.rule.id(), v.links.join(", "), v.detail),
            )
        })
        .collect()
}

fn resolve_pipeline(src: &str, opts: &Options) -> Result<resolve::Program, Error> {
    let tokens = lexer::lex(src)?;
    let file = syntax::parser::parse(src, &tokens)?;
    let lowered = desugar::desugar(&file)?;
    let theme = match &opts.theme_css {
        Some(css) => theme::extract_lini_vars(css),
        None => Vec::new(),
    };
    resolve::resolve_with_theme(&lowered, &theme)
}

fn wrap_html(svg: &str) -> String {
    format!(
        "<!doctype html>\n<html>\n<head>\n  <meta charset=\"utf-8\">\n  <title>lini</title>\n</head>\n<body>\n{}</body>\n</html>\n",
        svg
    )
}

/// Test-only hooks for the routing suite (see `ROUTING-LOG.md` stage 4/6).
/// Not part of the public API.
#[doc(hidden)]
pub mod testing {
    use crate::Options;
    use crate::layout;
    use crate::resolve::ResolvedValue;

    pub use crate::layout::LaidOut;

    /// A node's absolute rect by full dot-path, for geometric assertions.
    pub fn node_rect(laid: &LaidOut, path: &str) -> Option<(f64, f64, f64, f64)> {
        crate::routing::node_rect(&laid.nodes, path)
    }

    /// Routed polylines by endpoint pair, in declaration order — the contract
    /// tests' geometry hook (ROUTING-LOG.md stage 4): parse → resolve → layout,
    /// then each drawn link's `(seg_from, seg_to)` and path.
    #[allow(clippy::type_complexity)]
    pub fn routes_str(src: &str) -> Result<Vec<((String, String), Vec<(f64, f64)>)>, crate::Error> {
        let program = super::resolve_pipeline(src, &Options::default())?;
        let laid = layout::layout(&program)?;
        Ok(laid
            .links
            .iter()
            .map(|l| ((l.seg_from.clone(), l.seg_to.clone()), l.path.clone()))
            .collect())
    }

    /// Compile `src` to a laid-out scene with `clearance` forced on every link,
    /// overriding whatever the source set.
    pub fn route_sample(src: &str, clearance: f64) -> LaidOut {
        let mut prog = super::resolve_pipeline(src, &Options::default()).expect("resolve");
        for w in &mut prog.links {
            w.attrs
                .insert("clearance", ResolvedValue::Number(clearance));
        }
        layout::layout(&prog).expect("layout")
    }

    /// The number of routable edges the source declares (fans/chains already expanded
    /// at resolve into one `ResolvedLink` per edge-chain). Sequence-scope messages are
    /// **not** routable — the sequence layout draws them as time-row arrows [SPEC 13],
    /// so the router never sees them — and a drawing scope's links belong to its own
    /// engine [SPEC 15]; both are excluded here, mirroring `routing::ortho::request`.
    pub fn declared_edges(src: &str) -> usize {
        let prog = super::resolve_pipeline(src, &Options::default()).expect("resolve");
        prog.links
            .iter()
            .filter(|w| {
                w.routing == crate::resolve::Strategy::Orthogonal
                    && !layout::sequence::is_sequence_scope(&prog, &w.scope)
                    && !layout::drawing::is_drawing_scope(&prog, &w.scope)
            })
            .map(|w| w.endpoints.len().saturating_sub(1))
            .sum()
    }

    /// Judge a laid-out scene against the four laws (the independent validator).
    pub fn laws(laid: &LaidOut) -> Vec<crate::Violation> {
        layout::validate_routing(laid)
    }

    /// Drawn links that answer to `declared_edges`: what the orthogonal
    /// strategy drew. Straight wires stay out on both sides of the count —
    /// a sequence's messages are the layout's own, and a `routing: straight`
    /// pair whose trim leaves nothing lawfully draws nothing.
    pub fn drawn_edges(laid: &LaidOut) -> usize {
        laid.links
            .iter()
            .filter(|w| w.strategy == crate::resolve::Strategy::Orthogonal)
            .count()
    }
}
