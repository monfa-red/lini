mod ast;
mod desugar;
mod error;
mod expr;
mod fmt;
mod font;
mod glyph;
mod icon;
mod layout;
mod ledger;
mod lexer;
mod lint;
mod palette;
mod render;
mod resolve;
mod routing;
mod schema;
mod serve;
mod span;
mod suggest;
mod syntax;
mod theme;
mod validate;

pub use error::{Diagnostic, Error, Level};
pub use fmt::format as format_source;

/// The generated, ledger-backed tooling contract [ROADMAP 3.8]: the
/// machine-readable JSON schema and its compact Markdown mirror, plus the
/// compiled per-property examples the schema embeds. `cargo xtask gen-schema`
/// writes the two files; `tests/schema.rs` guards them byte-identical.
pub use ledger::examples::EXAMPLES as schema_examples;
pub use schema::{reference_md, schema_json};

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

/// Whether the bundled font subsets were compiled in (the default-on `font`
/// feature) — the gate for `--embed-font` / `--static` outlining [SPEC 19].
pub fn font_support() -> bool {
    font::ENABLED
}
pub use serve::{ServeTarget, serve};
pub use theme::{builtin_css, extract_lini_vars, list_themes, pair_css};

/// Top-level compile options threaded through every phase. Build with
/// `Options::default()` and override fields with the struct-update syntax —
/// future versions may add knobs.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// `--static` [SPEC 10.6/17/19]: emit `var()` values inline as their
    /// resolved literal **and** outline text to paths — self-contained for
    /// renderers without CSS-variable or font support (resvg, librsvg, image
    /// converters). The structural class rules stay; only the `@layer`
    /// variable defaults are dropped (their values are inlined). Outlining
    /// needs the default-on `font` feature; without it the vars still bake
    /// and text stays name-only `<text>`.
    pub static_mode: bool,
    /// `--embed-font` [SPEC 17]: inline a base64 `@font-face` per bundled
    /// family × weight actually used, under Lini-scoped family names.
    /// Browser-only by design (resvg/librsvg ignore `@font-face`); needs the
    /// `font` feature.
    pub embed_font: bool,
    /// Output wrapper format.
    pub format: OutputFormat,
    /// Raw CSS text whose `--lini-*` declarations override built-in defaults
    /// before the `defaults {}` block. `extract_lini_vars` does the parse.
    pub theme_css: Option<String>,
    /// The source file's directory — where a local `|image| src:` path
    /// resolves [SPEC 7]. `None` (stdin) resolves paths as written.
    pub base_dir: Option<std::path::PathBuf>,
    /// The serve traversal boundary [SPEC 19]: asset reads are confined to
    /// this root — an escape is a compile error. `None` (the plain CLI) is
    /// unbounded: you compile your own file.
    pub asset_root: Option<std::path::PathBuf>,
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
    let mut diags = layout::extent_hints(&laid_out, &program);
    diags.extend(routing_diagnostics_of(layout::validate_routing(&laid_out)));
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
    let mut out = validate::validate(&file);
    out.extend(lint::lint(&file));
    Ok(out)
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
    validate_str_with(src, &Options::default())
}

/// [`validate_str`] with options — a sample sweeping suite passes `base_dir`
/// so file-relative image assets resolve [SPEC 7].
pub fn validate_str_with(src: &str, opts: &Options) -> Result<Vec<Violation>, Error> {
    let program = resolve_pipeline(src, opts)?;
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
    // Tree structure errors [SPEC 20] read the still-nested AST — before desugar
    // flattens each `layout: tree` scope's topic hierarchy [SPEC 12].
    desugar::tree::validate(&file)?;
    let lowered = desugar::desugar(&file)?;
    let theme = match &opts.theme_css {
        Some(css) => theme::extract_lini_vars(css),
        None => Vec::new(),
    };
    let env = resolve::AssetEnv {
        base_dir: opts.base_dir.clone(),
        root: opts.asset_root.clone(),
    };
    resolve::resolve_with_env(&lowered, &theme, env)
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
        routes_str_with(src, &Options::default())
    }

    /// [`routes_str`] with options (`base_dir` for sample sweeps [SPEC 7]).
    #[allow(clippy::type_complexity)]
    pub fn routes_str_with(
        src: &str,
        opts: &Options,
    ) -> Result<Vec<((String, String), Vec<(f64, f64)>)>, crate::Error> {
        let program = super::resolve_pipeline(src, opts)?;
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
        route_sample_with(src, &Options::default(), clearance)
    }

    /// [`route_sample`] with options (`base_dir` for sample sweeps [SPEC 7]).
    pub fn route_sample_with(src: &str, opts: &Options, clearance: f64) -> LaidOut {
        let mut prog = super::resolve_pipeline(src, opts).expect("resolve");
        for w in &mut prog.links {
            w.attrs
                .insert("clearance", ResolvedValue::Number(clearance));
        }
        layout::layout(&prog).expect("layout")
    }

    /// The number of routable corridor edges (orthogonal and natural) the source
    /// declares (fans/chains already expanded at resolve into one `ResolvedLink`
    /// per edge-chain). Sequence-scope messages are
    /// **not** routable — the sequence layout draws them as time-row arrows [SPEC 13],
    /// so the router never sees them — and a drawing scope's links belong to its own
    /// engine [SPEC 15]; both are excluded here, mirroring `routing::ortho::request`.
    pub fn declared_edges(src: &str) -> usize {
        declared_edges_with(src, &Options::default())
    }

    /// [`declared_edges`] with options (`base_dir` for sample sweeps [SPEC 7]).
    pub fn declared_edges_with(src: &str, opts: &Options) -> usize {
        let prog = super::resolve_pipeline(src, opts).expect("resolve");
        prog.links
            .iter()
            .filter(|w| {
                matches!(
                    w.routing,
                    crate::resolve::Strategy::Orthogonal | crate::resolve::Strategy::Natural
                ) && !layout::sequence::is_sequence_scope(&prog, &w.scope)
                    && !layout::drawing::is_drawing_scope(&prog, &w.scope)
            })
            .map(|w| w.endpoints.len().saturating_sub(1))
            .sum()
    }

    /// Judge a laid-out scene against the four laws (the independent validator).
    pub fn laws(laid: &LaidOut) -> Vec<crate::Violation> {
        layout::validate_routing(laid)
    }

    /// Lay out a source string (with options) — the probe hook for geometric
    /// assertions on a full scene.
    pub fn layout_sample(src: &str, opts: &Options) -> LaidOut {
        let prog = super::resolve_pipeline(src, opts).expect("resolve");
        layout::layout(&prog).expect("layout")
    }

    /// The no-spill oracle [SPEC 15.8]: any `|page|` content — a view, its
    /// annotations, a note, the title block — whose painted bbox crosses the
    /// sheet's inner `|frame|`. Generated furniture (the frame, zones, ticks,
    /// centring marks — the margin chrome) is excluded; a flush-seated title
    /// block sits *on* the frame line, so a small tolerance admits it. An empty
    /// result means every view is packed inside its walls.
    pub fn frame_overflow(laid: &LaidOut) -> Vec<String> {
        use crate::layout::ir::PlacedNode;
        const EPS: f64 = 2.0;
        fn abs(n: &PlacedNode, ox: f64, oy: f64) -> (f64, f64, f64, f64) {
            let (cx, cy) = (ox + n.cx, oy + n.cy);
            (
                cx + n.bbox.min_x,
                cy + n.bbox.min_y,
                cx + n.bbox.max_x,
                cy + n.bbox.max_y,
            )
        }
        fn walk(nodes: &[PlacedNode], ox: f64, oy: f64, out: &mut Vec<String>) {
            for n in nodes {
                let (cx, cy) = (ox + n.cx, oy + n.cy);
                if n.type_chain.iter().any(|t| t == "page")
                    && let Some(frame) = n
                        .children
                        .iter()
                        .find(|c| c.type_chain.iter().any(|t| t == "frame"))
                {
                    let (fx0, fy0, fx1, fy1) = abs(frame, cx, cy);
                    for c in &n.children {
                        if c.attrs.get("chrome").is_some() {
                            continue;
                        }
                        let (x0, y0, x1, y1) = abs(c, cx, cy);
                        if x0 < fx0 - EPS || x1 > fx1 + EPS || y0 < fy0 - EPS || y1 > fy1 + EPS {
                            out.push(format!(
                                "{}: [{x0:.1},{y0:.1},{x1:.1},{y1:.1}] crosses frame [{fx0:.1},{fy0:.1},{fx1:.1},{fy1:.1}]",
                                c.id.clone().unwrap_or_else(|| format!("<{:?}>", c.kind))
                            ));
                        }
                    }
                }
                walk(&n.children, cx, cy, out);
            }
        }
        let mut out = Vec::new();
        walk(&laid.nodes, 0.0, 0.0, &mut out);
        out
    }

    /// Drawn links that answer to `declared_edges`: what the corridor
    /// strategies (orthogonal and natural) drew. Straight wires stay out on both sides of the count —
    /// a sequence's messages are the layout's own, and a `routing: straight`
    /// pair whose trim leaves nothing lawfully draws nothing.
    pub fn drawn_edges(laid: &LaidOut) -> usize {
        laid.links
            .iter()
            .filter(|w| {
                matches!(
                    w.strategy,
                    crate::resolve::Strategy::Orthogonal | crate::resolve::Strategy::Natural
                )
            })
            .count()
    }
}
