mod ast;
mod desugar;
mod error;
mod expr;
mod fmt;
mod font;
mod glyph;
mod grammar;
mod icon;
mod json;
mod layout;
mod ledger;
mod lexer;
mod lint;
mod math;
mod name;
mod palette;
mod path_data;
mod render;
mod resolve;
mod routing;
mod schema;
mod serve;
mod span;
mod suggest;
mod syntax;
#[cfg(test)]
mod testutil;
mod theme;
mod validate;

use error::Phase;
pub use error::{Diagnostic, Error, Level};
pub use fmt::format as format_source;

/// The generated, ledger-backed tooling contract [ROADMAP 3.8]: the
/// machine-readable JSON schema and its compact Markdown mirror, plus the
/// compiled per-property examples the schema embeds. `cargo xtask gen-schema`
/// writes the two files; `tests/schema.rs` guards them byte-identical.
pub use ledger::examples::EXAMPLES as schema_examples;
pub use schema::{reference_md, schema_json};

/// The **generated** grammar homes [SPEC 22 / 23]: the VS Code TextMate bundle,
/// the Zed tree-sitter highlight query, and the playground tokenizer's
/// word-list region — all fed from one word source over the ledger and the
/// parse tables. `cargo xtask gen-grammars` writes them; `tests/grammar.rs`
/// guards them byte-identical.
pub use grammar::{splice_playground, vscode_grammar, zed_highlights};

/// Lini source as syntax-highlighted HTML — `<span class="lini-tok-…">` runs
/// over escaped text, and **the** highlighter [SPEC 20 / 22]. It reads the word
/// sets the editor grammars are generated from, so a new type, property,
/// builder, or glyph colours the moment it has a ledger row.
///
/// **Every character survives.** Strip the tags, undo the four entity escapes
/// (`&amp; &lt; &gt; &quot;`), and the source comes back byte for byte — which
/// is what lets a caller drop the output into a `<pre>` and trust the listing
/// to be the author's own text. Newlines pass through as newlines; a caller
/// that cannot carry one (an HTML block inside Markdown) rewrites them itself.
///
/// It is **lexical** — it never parses — so a file mid-keystroke still colours,
/// and it cannot fail.
///
/// The classes are the token kinds, under the reserved prefix so a host page's
/// own `.tok-string` can never repaint a listing [SPEC 23]: `lini-tok-` +
/// `comment` · `string` · `number` · `const` · `keyword` · `type` · `type-user`
/// · `prop` · `prop-user` · `var` · `op` · `class` · `punct`. Text that needs no
/// colour goes out bare, with no span; [`highlight_css`] paints the rest.
///
/// Two thin wrappers reach the same scanner from outside a Rust crate:
/// `lini highlight <file>` at a shell, and `highlight()` in the wasm build.
///
/// ```
/// let html = lini::highlight_html("|box#a| \"Hi\"\n");
/// assert!(html.contains("<span class=\"lini-tok-type\">box</span>"));
/// ```
pub use grammar::highlight_html;

/// The stylesheet [`highlight_html`]'s markup wears — the one token palette,
/// nine `--lini-tok-*` role variables as `light-dark()` pairs plus the rules
/// that paint the thirteen classes from them. `lini highlight --css` prints it;
/// a book or a site ships it beside its own CSS, and the playground splices it,
/// so a Lini listing reads the same wherever it lands [SPEC 18 / 20].
///
/// It sets no `color-scheme`: that stays the host's, and `light-dark()` reads
/// whatever the host has set on the listing's ancestors — one sheet for a
/// book's five themes and an editor's toggle alike. Re-tint a role by
/// redeclaring its variable; the defaults are layered, so no `!important`.
pub use grammar::highlight_css;

/// Lower a source file's sugar to primitives + `.lini-*` classes and print canonical
/// `.lini` — what `lini desugar` shows: every typed instance becomes a `|primitive|`
/// wearing its `.lini-*` chain, defines and templates collapse into generated
/// `.lini-*` class defs, scene/link defaults fill the global block, and labels /
/// `along:` become explicit. Comments are dropped. The lowered form re-renders
/// identically and is a fixed point of desugar.
pub fn desugar_source(src: &str) -> Result<String, Error> {
    let file = parse_stage(src)?;
    // The same gate the compiler runs ahead of the lowering [SPEC 12/21]: what
    // `lini desugar` prints must re-render identically, so a file it accepts is
    // a file `lini build` accepts.
    desugar::tree::validate(&file).map_err(|e| e.in_phase(Phase::Resolve))?;
    let lowered = desugar::desugar(&file).map_err(|e| e.in_phase(Phase::Resolve))?;
    Ok(fmt::print_file(&lowered))
}
pub use routing::{Rule, Severity, Violation};

/// Whether the bundled font subsets were compiled in (the default-on `font`
/// feature) — the gate for `--embed-font` / `--static` outlining [SPEC 20].
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
    /// `--embed-font` [SPEC 18]: inline a base64 `@font-face` per bundled
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
    /// The serve traversal boundary [SPEC 20]: asset reads are confined to
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
    compile_str_checked(src, opts).map(|(svg, _)| svg)
}

/// Validate and compile to SVG, collecting validation and routing warnings in
/// the same result. Any error-level validation diagnostic rejects the compile.
pub fn compile_str_checked(src: &str, opts: &Options) -> Result<(String, Vec<Diagnostic>), Error> {
    let (program, mut diags) = validated_resolve_pipeline(src, opts)?;
    let (svg, later_diags) = compile_program_checked(&program, opts)?;
    diags.extend(later_diags);
    Ok((svg, diags))
}

fn compile_program_checked(
    program: &resolve::Program,
    opts: &Options,
) -> Result<(String, Vec<Diagnostic>), Error> {
    let mut laid_out = layout_stage(program)?;
    render::lower_paints(&mut laid_out);
    let mut diags = error::stamp_phase(layout::layout_hints(&laid_out, program), Phase::Layout);
    diags.extend(routing_diagnostics_of(layout::validate_routing(&laid_out)));
    Ok((finish_svg(&laid_out, opts), diags))
}

/// The full diagnostic set for a source, as one serde-free JSON document
/// [ROADMAP 3.8, decision 9] — the `--json` CLI form. Runs the same passes the
/// default compile does (validation, then layout + routing when validation is
/// clean), collecting every diagnostic — errors, warnings, and a fatal
/// compile error — each with its stable code, span, related span, and any
/// machine-applicable replacement. Returns the document plus whether any
/// **error**-level diagnostic fired (the caller's exit code).
pub fn diagnostics_json(src: &str, opts: &Options, filename: &str) -> (String, bool) {
    let mut items = Vec::new();
    let mut had_error = false;

    // The property/lint pass [SPEC 17/21] — surfaces on the raw parse. A parse
    // or lex error here is fatal and stops the pipeline, exactly as the default
    // CLI path returns early.
    match lint_str(src) {
        Ok(diags) => {
            for d in &diags {
                had_error |= d.level == Level::Error;
                items.push(d.to_json(src));
            }
        }
        Err(e) => {
            items.push(e.to_json(src));
            return (error::diagnostics_document(items, filename), true);
        }
    }

    // Validation errors stop the compile [SPEC 20] — mirror that: only route on
    // a clean validation, so layout never runs on a rejected file.
    if !had_error {
        match resolve_pipeline(src, opts)
            .and_then(|program| compile_program_checked(&program, opts))
        {
            Ok((_, route_diags)) => {
                for d in &route_diags {
                    had_error |= d.level == Level::Error;
                    items.push(d.to_json(src));
                }
            }
            Err(e) => {
                items.push(e.to_json(src));
                had_error = true;
            }
        }
    }

    (error::diagnostics_document(items, filename), had_error)
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
    let _file = parse_stage(src)?;
    Ok(())
}

/// Lex, parse, and run the lint pass. Returns warnings (no errors).
/// Parse errors are surfaced as `Err`; missing lints just return an empty Vec.
pub fn lint_str(src: &str) -> Result<Vec<Diagnostic>, Error> {
    let file = parse_stage(src)?;
    let mut out = validate::validate(&file);
    out.extend(lint::lint(&file));
    Ok(error::stamp_phase(out, Phase::Validate))
}

/// Validate, resolve, and reject any error-level diagnostic without running
/// layout or render. The CLI's `--check` flag goes through here.
pub fn check(src: &str) -> Result<(), Error> {
    check_with(src, &Options::default())
}

pub fn check_with(src: &str, opts: &Options) -> Result<(), Error> {
    let _ = validated_resolve_pipeline(src, opts)?;
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
    let (program, _) = validated_resolve_pipeline(src, opts)?;
    let laid_out = layout_stage(&program)?;
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
            let code = match v.rule {
                Rule::Impossible => error::Code::IMPOSSIBLE_LINK,
                _ => error::Code::LAW_BREACH,
            };
            Diagnostic::warn(
                v.span,
                format!("{} ({}): {}", v.rule.id(), v.links.join(", "), v.detail),
            )
            .code(code)
        })
        .collect()
}

fn resolve_pipeline(src: &str, opts: &Options) -> Result<resolve::Program, Error> {
    let file = parse_stage(src)?;
    // Tree structure errors [SPEC 21] read the still-nested AST — before desugar
    // flattens each `layout: tree` scope's topic hierarchy [SPEC 12].
    desugar::tree::validate(&file).map_err(|e| e.in_phase(Phase::Resolve))?;
    let lowered = desugar::desugar(&file).map_err(|e| e.in_phase(Phase::Resolve))?;
    let theme = match &opts.theme_css {
        Some(css) => theme::extract_lini_vars(css),
        None => Vec::new(),
    };
    let env = resolve::AssetEnv {
        base_dir: opts.base_dir.clone(),
        root: opts.asset_root.clone(),
    };
    resolve::resolve_with_env(&lowered, &theme, env).map_err(|e| e.in_phase(Phase::Resolve))
}

/// The single acceptance gate shared by every public compile/check surface.
/// Warnings continue down the pipeline; the first error-level diagnostic
/// becomes the fatal error carried by the public `Result` API.
fn validated_resolve_pipeline(
    src: &str,
    opts: &Options,
) -> Result<(resolve::Program, Vec<Diagnostic>), Error> {
    let diags = lint_str(src)?;
    if let Some(diag) = diags
        .iter()
        .find(|diag| diag.level == Level::Error)
        .cloned()
    {
        return Err(diag.into_error());
    }
    let program = resolve_pipeline(src, opts)?;
    Ok((program, diags))
}

/// Lex + parse, each stamped with its phase code at the boundary — the single
/// funnel every pipeline shares [decision 7].
fn parse_stage(src: &str) -> Result<syntax::ast::File, Error> {
    let tokens = lexer::lex(src).map_err(|e| e.in_phase(Phase::Lex))?;
    syntax::parser::parse(src, &tokens).map_err(|e| e.in_phase(Phase::Parse))
}

/// Lay out a resolved program, stamping any layout error with its phase code.
fn layout_stage(program: &resolve::Program) -> Result<layout::LaidOut, Error> {
    layout::layout(program).map_err(|e| e.in_phase(Phase::Layout))
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
    use std::path::{Path, PathBuf};

    pub use crate::layout::LaidOut;
    pub use crate::layout::ir::PlacedNode;

    /// **Does this source compile clean?** — the public compiler's verdict,
    /// rendered the way the CLI prints it.
    ///
    /// The two suites that judge a whole source by whether it compiles — the
    /// deferred-surface ledger (`tests/deferred.rs`) and the SPEC fenced-block
    /// guard (`tests/spec_blocks.rs`) — read the verdict from here, so
    /// "compiles clean" means one thing and a lint-phase gate can never hide
    /// behind a clean render.
    pub fn compile_verdict(src: &str, filename: &str) -> Result<(), String> {
        crate::compile_str(src)
            .map(|_| ())
            .map_err(|e| e.display_with_source(src, filename).to_string())
    }

    /// Every **type** a source may wear as a `lini-{type}` class [SPEC 18] —
    /// the primitives, the built-in templates, and the file's own `define`s
    /// (each link of a define chain is itself one of the three). The hook
    /// inventory (`tests/hooks.rs`) reads it to tell a type class apart from
    /// generated chrome, so the two can never be confused by a name alone.
    pub fn type_class_names(src: &str) -> Vec<String> {
        let mut out: Vec<String> = crate::resolve::NodeKind::ALL
            .iter()
            .map(|k| k.as_str().to_string())
            .collect();
        out.extend(
            crate::desugar::types::TEMPLATES
                .iter()
                .map(|(name, _)| (*name).to_string()),
        );
        if let Ok(tokens) = crate::lexer::lex(src)
            && let Ok(file) = crate::syntax::parser::parse(src, &tokens)
        {
            out.extend(file.stylesheet.iter().filter_map(|it| match it {
                crate::syntax::ast::StyleItem::Define(d) => Some(d.name.clone()),
                _ => None,
            }));
        }
        out
    }

    /// The hue names the mindmap walk mints `lini-hue-{name}` from [SPEC 8/18]
    /// — the palette's own walk order, so the inventory checks the parameter
    /// rather than waving the family through.
    pub fn hue_class_names() -> Vec<String> {
        crate::palette::walk_hues().map(str::to_string).collect()
    }

    /// The showroom sheets [SPEC 19] plus the routing-oracle fixtures — the one
    /// corpus every sweep walks, sorted so failures report in a stable order.
    ///
    /// Two directories, one policy. `samples/` is the showroom (also the
    /// conformance glob's snapshot set); `tests/fixtures/routing/` holds the
    /// three `links_*` scenes that exist only to feed the router's oracles —
    /// they carry no snapshot (a snapshot would pin one router's coordinates
    /// and churn every phase, see `tests/conformance.rs`) but must still parse,
    /// resolve, format, desugar and route like any other sheet.
    ///
    /// The single skip: icon-bearing sheets when the `icons` feature is off,
    /// since `|icon|` is then an error by construction. Nothing else is
    /// skipped — an untracked scratch file in `samples/` is the author's
    /// problem, not a hole in the sweep.
    pub fn samples() -> Vec<PathBuf> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut paths: Vec<PathBuf> = [root.join("samples"), root.join("tests/fixtures/routing")]
            .iter()
            .flat_map(|dir| {
                std::fs::read_dir(dir)
                    .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
                    .filter_map(|e| e.ok().map(|e| e.path()))
            })
            .filter(|p| p.extension().is_some_and(|x| x == "lini"))
            .filter(|p| {
                cfg!(feature = "icons")
                    || !std::fs::read_to_string(p)
                        .unwrap_or_default()
                        .contains("|icon|")
            })
            .collect();
        paths.sort();
        paths
    }

    /// Read one sweep entry, naming the file if it cannot be read.
    pub fn read_sample(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
    }

    /// The options every sweep compiles under: samples resolve their image
    /// assets against `samples/` [SPEC 7].
    pub fn sample_opts() -> Options {
        Options {
            base_dir: Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("samples")),
            ..Default::default()
        }
    }

    /// A placed node with its **world** centre — `cx`/`cy` summed down the tree.
    pub type Placed<'a> = (&'a PlacedNode, f64, f64);

    /// A predicate over placed nodes, for the two walks below.
    pub type Pred<'a> = dyn Fn(&PlacedNode) -> bool + 'a;

    /// The first node satisfying `pred`, depth-first, self before children.
    ///
    /// Every placement assertion in the crate looks a node up the same way — by
    /// id, by type, by class — and needs its world position; this is that walk,
    /// written once. [`all_placed`] is the collecting twin.
    pub fn find_placed<'a>(nodes: &'a [PlacedNode], pred: &Pred<'_>) -> Option<Placed<'a>> {
        fn walk<'a>(
            nodes: &'a [PlacedNode],
            pred: &Pred<'_>,
            ox: f64,
            oy: f64,
        ) -> Option<Placed<'a>> {
            for n in nodes {
                let (x, y) = (ox + n.cx, oy + n.cy);
                if pred(n) {
                    return Some((n, x, y));
                }
                if let Some(hit) = walk(&n.children, pred, x, y) {
                    return Some(hit);
                }
            }
            None
        }
        walk(nodes, pred, 0.0, 0.0)
    }

    /// Every node satisfying `pred`, depth-first, each with its world centre.
    /// A match's own children are still searched — a classed box may hold one.
    pub fn all_placed<'a>(nodes: &'a [PlacedNode], pred: &Pred<'_>) -> Vec<Placed<'a>> {
        fn walk<'a>(
            nodes: &'a [PlacedNode],
            pred: &Pred<'_>,
            ox: f64,
            oy: f64,
            out: &mut Vec<Placed<'a>>,
        ) {
            for n in nodes {
                let (x, y) = (ox + n.cx, oy + n.cy);
                if pred(n) {
                    out.push((n, x, y));
                }
                walk(&n.children, pred, x, y, out);
            }
        }
        let mut out = Vec::new();
        walk(nodes, pred, 0.0, 0.0, &mut out);
        out
    }

    /// The placed node carrying `id`, with its world centre.
    #[track_caller]
    pub fn placed_by_id<'a>(nodes: &'a [PlacedNode], id: &str) -> Placed<'a> {
        find_placed(nodes, &|n| n.id.as_deref() == Some(id))
            .unwrap_or_else(|| panic!("no placed node '{id}'"))
    }

    /// The first placed node whose type chain carries `ty`, with its world centre.
    #[track_caller]
    pub fn placed_by_type<'a>(nodes: &'a [PlacedNode], ty: &str) -> Placed<'a> {
        find_placed(nodes, &|n| n.type_chain.iter().any(|t| t == ty))
            .unwrap_or_else(|| panic!("no placed '{ty}' node"))
    }

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

    /// One fixed-port injection for [`route_sample_with_ports`]: on the link
    /// statement running `from … to`, the end whose resolved path is `at`
    /// takes the forced side (`"left"` · `"right"` · `"top"` · `"bottom"`)
    /// and the exact landing ordinate (ROUTING.md Fixed ports).
    pub type FixedPort<'a> = (&'a str, &'a str, &'a str, &'a str, f64);

    /// [`route_sample`] with fixed ports injected onto resolved endpoints —
    /// the Phase-1 probe until schematic pins set them [SPEC 16.5].
    pub fn route_sample_with_ports(src: &str, clearance: f64, ports: &[FixedPort]) -> LaidOut {
        let side = |name: &str| match name {
            "left" => crate::ast::Side::Left,
            "right" => crate::ast::Side::Right,
            "top" => crate::ast::Side::Top,
            "bottom" => crate::ast::Side::Bottom,
            _ => panic!("unknown side '{name}'"),
        };
        let mut prog = super::resolve_pipeline(src, &Options::default()).expect("resolve");
        for w in &mut prog.links {
            w.attrs
                .insert("clearance", ResolvedValue::Number(clearance));
            let first = w.endpoints.first().expect("endpoint").path.clone();
            let last = w.endpoints.last().expect("endpoint").path.clone();
            for e in &mut w.endpoints {
                for &(from, to, at, s, ord) in ports {
                    if first == from && last == to && e.path == at {
                        e.side = Some(side(s));
                        e.port = Some(ord);
                    }
                }
            }
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
                ) && !w.written_in.consumes_links()
            })
            .map(|w| w.endpoints.len().saturating_sub(1))
            .sum()
    }

    /// Judge a laid-out scene against the four laws (the independent validator).
    pub fn laws(laid: &LaidOut) -> Vec<crate::Violation> {
        layout::validate_routing(laid)
    }

    /// Law breaches: everything a report flags above counted output (`Info`
    /// crossings) and honest strays (`Impossible`, counted by [`strays`]).
    /// The one predicate every law assertion in the suite judges by.
    pub fn breaches(report: Vec<crate::Violation>) -> Vec<crate::Violation> {
        report
            .into_iter()
            .filter(|v| v.severity != crate::Severity::Info && v.rule != crate::Rule::Impossible)
            .collect()
    }

    /// Honest strays: links the router reported rather than drew.
    pub fn strays(report: &[crate::Violation]) -> usize {
        report
            .iter()
            .filter(|v| v.rule == crate::Rule::Impossible)
            .count()
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

    /// The annotation-packing oracle [SPEC 15.6]: pairs of dimension values
    /// whose painted boxes overlap. A row stands `clearance` off everything
    /// painted, so no dim value may land on another annotation's text —
    /// another row's, a callout's, an angle's. An empty result means the
    /// packer cleared every one.
    pub fn annotation_text_overlaps(laid: &LaidOut) -> Vec<String> {
        let boxes: Vec<crate::layout::ir::Bbox> = all_placed(&laid.nodes, &|n| {
            n.type_chain.iter().any(|t| t == "dim-text")
        })
        .into_iter()
        .map(|(n, x, y)| {
            crate::layout::ir::Bbox::extent_of(std::slice::from_ref(n), |_| true)
                .shifted(x - n.cx, y - n.cy)
        })
        .collect();
        let mut out = Vec::new();
        for (i, a) in boxes.iter().enumerate() {
            for b in &boxes[i + 1..] {
                if a.inflate(-0.5).overlaps(b.inflate(-0.5)) {
                    out.push(format!("annotation texts overlap: {a:?} vs {b:?}"));
                }
            }
        }
        out
    }

    /// The carried-stack oracle [SPEC 15.6/15.9]: carried annotation boxes
    /// that cross the geometry their statement annotates. What a statement
    /// paints below its text is part of its own painted band, so it must
    /// clear the drawn geometry like the text does. The count of carried
    /// boxes judged rides along, so a sweep can assert it saw any.
    pub fn carried_over_geometry(laid: &LaidOut) -> (Vec<String>, usize) {
        use crate::layout::ir::{Bbox, PlacedNode};
        fn walk(nodes: &[PlacedNode], out: &mut Vec<String>, seen: &mut usize) {
            for n in nodes {
                let carried: Vec<Bbox> = n
                    .children
                    .iter()
                    .filter(|c| c.type_chain.iter().any(|t| t == "carried"))
                    .map(|c| c.bbox.shifted(c.cx, c.cy))
                    .collect();
                if !carried.is_empty() {
                    let geo = crate::layout::drawing::annotate::drawn_geometry(&n.children);
                    for b in &carried {
                        *seen += 1;
                        if b.overlaps(geo) {
                            out.push(format!(
                                "a carried annotation {b:?} crosses the drawn geometry {geo:?}"
                            ));
                        }
                    }
                }
                walk(&n.children, out, seen);
            }
        }
        let (mut out, mut seen) = (Vec::new(), 0);
        walk(&laid.nodes, &mut out, &mut seen);
        (out, seen)
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
