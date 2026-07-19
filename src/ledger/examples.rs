//! One authored example per property [ROADMAP 3.8], keyed by name. Beside the
//! ledger so a new row's example is added right here; the schema embeds each,
//! and a test **compiles every one** through `lini::compile_str` (drawing /
//! chart / sequence snippets wrapped in their owning container), so an example
//! can never rot into invalid syntax. Order tracks `PROPERTIES`.

/// `(property name, a complete source that exercises it)`. Every string must
/// compile; the coverage test also asserts one entry per property row.
pub const EXAMPLES: &[(&str, &str)] = &[
    // ── Paint & stroke ──
    ("fill", "|box| \"leaf\" { fill: --sky-soft; }"),
    ("opacity", "|box| \"faint\" { opacity: 0.5; }"),
    ("stroke", "|box| \"edge\" { stroke: --sky-deep; }"),
    ("stroke-width", "|box| \"bold\" { stroke-width: 3; }"),
    ("stroke-style", "|box| \"dashed\" { stroke-style: dashed; }"),
    ("radius", "|box| \"round\" { radius: 12; }"),
    ("shadow", "|box| \"lifted\" { shadow: 2 3 5; }"),
    (
        "gap-fill",
        "|row| { gap: 1; gap-fill: --sky-soft; } [\n  |box| \"a\"\n  |box| \"b\"\n]",
    ),
    // ── Text ──
    (
        "font-family",
        "|box| \"serif\" { font-family: \"Georgia\"; }",
    ),
    ("font-size", "|box| \"big\" { font-size: 20; }"),
    ("font-weight", "|box| \"heavy\" { font-weight: bold; }"),
    ("font-style", "|box| \"slanted\" { font-style: italic; }"),
    (
        "text-transform",
        "|box| \"loud\" { text-transform: uppercase; }",
    ),
    (
        "text-decoration",
        "|box| \"link\" { text-decoration: underline; }",
    ),
    (
        "text-shadow",
        "|box| \"glow\" { text-shadow: 1 1 2 --gray-ink; }",
    ),
    (
        "letter-spacing",
        "|box| \"S P A C E D\" { letter-spacing: 3; }",
    ),
    (
        "line-spacing",
        "|box| \"two\\nlines\" { line-spacing: 12; }",
    ),
    ("color", "|box| \"tinted\" { color: --sky-ink; }"),
    // ── Box model & placement ──
    ("width", "|box| \"wide\" { width: 120; }"),
    ("height", "|box| \"tall\" { height: 80; }"),
    ("padding", "|box| \"roomy\" { padding: 18; }"),
    (
        "max-width",
        "|box| \"a longer run of body copy that wraps\" { max-width: 120; }",
    ),
    (
        "text-wrap",
        "|box| \"one line\" { max-width: 200; text-wrap: nowrap; }",
    ),
    (
        "pin",
        "|box| \"card\" { width: 120; height: 80; } [\n  |box| \"tag\" { pin: top; }\n]",
    ),
    ("translate", "|box| \"nudged\" { translate: 10 -4; }"),
    ("rotate", "|box| \"tilted\" { rotate: -8; }"),
    ("layer", "|box| \"front\" { layer: 2; }"),
    ("scale", "|box| \"scaled\" { scale: 2; }"),
    (
        "pattern",
        "|drawing| [\n  |rect#p| { width: 80; height: 40; } [\n    |hole#b| { width: 8; translate: -20 0; pattern: grid(2, 1, 40, 0); }\n  ]\n]",
    ),
    // ── Media & accessibility ──
    ("href", "|box| \"docs\" { href: \"https://example.com\"; }"),
    ("hint", "|box| \"help\" { hint: \"the accessible label\"; }"),
    // ── Type-owned ──
    ("points", "|line| { points: 0 0, 60 20; }"),
    (
        "samples",
        "|line| { points: (u * 120, sin(u * 6) * 20); samples: 48; }",
    ),
    ("path", "|path| { path: \"M0 0 L60 0 L30 40 Z\"; }"),
    (
        "src",
        "|image| { src: \"assets/logo.svg\"; width: 40; height: 40; }",
    ),
    (
        "symbol",
        "|drawing| [\n  |rect| { width: 40; height: 24; }\n  |surface-finish| \"Ra 1.6\" { symbol: machined; }\n]",
    ),
    (
        "fit",
        "|image| { src: \"assets/logo.svg\"; width: 60; height: 40; fit: contain; }",
    ),
    ("skew", "|slant| \"lean\" { skew: 20; }"),
    ("stack", "|box| \"deck\" { stack: 4; }"),
    ("marker", "|line| { points: -60 0, 60 0; marker: dot; }"),
    (
        "marker-start",
        "|line| { points: -60 0, 60 0; marker-start: dot; }",
    ),
    (
        "marker-end",
        "|line| { points: -60 0, 60 0; marker-end: arrow; }",
    ),
    (
        "draw",
        "|sketch| { draw: move(0, 0) right(40) down(20) close(); }",
    ),
    (
        "mirror",
        "|sketch| { draw: move(-30, 0) up(8) right(30); mirror: x-axis; }",
    ),
    (
        "revolve",
        "|drawing| [\n  |sketch| { draw: move(-20, 0) up(10) right(40) down(10); revolve: x-axis; }\n]",
    ),
    (
        "thread",
        "|drawing| [\n  |rect| { width: 60; height: 40; } [\n    |hole| { width: 8; thread: 1.25; }\n  ]\n]",
    ),
    ("sheet", "|page| { sheet: a4 landscape; }"),
    (
        "break",
        "|drawing| [\n  |sketch| { draw: move(-80, 0) up(8) right(160) down(8); revolve: x-axis; break: -20 20; }\n]",
    ),
    // ── Layout & grid ──
    (
        "layout",
        "|box| { layout: grid; columns: repeat(2); } [\n  |box| \"a\"\n  |box| \"b\"\n]",
    ),
    (
        "direction",
        "|box| { layout: flow; direction: column; } [\n  |box| \"a\"\n  |box| \"b\"\n]",
    ),
    (
        "gap",
        "|row| { gap: 24; } [\n  |box| \"a\"\n  |box| \"b\"\n]",
    ),
    (
        "align",
        "|row| { gap: 12; align: center; } [\n  |box| \"a\"\n  |box| \"tall\\nbox\"\n]",
    ),
    (
        "justify",
        "|row| { gap: 12; justify: center; } [\n  |box| \"a\"\n  |box| \"b\"\n]",
    ),
    (
        "columns",
        "|box| { layout: grid; columns: 40, auto; } [\n  |box| \"a\"\n  |box| \"b\"\n]",
    ),
    (
        "rows",
        "|box| { layout: grid; columns: repeat(2); rows: 40, auto; } [\n  |box| \"a\"\n  |box| \"b\"\n  |box| \"c\"\n  |box| \"d\"\n]",
    ),
    (
        "cell",
        "|box| { layout: grid; columns: repeat(2); } [\n  |box| \"placed\" { cell: 1 2; }\n]",
    ),
    (
        "span",
        "|box| { layout: grid; columns: repeat(2); } [\n  |box| \"wide\" { span: 2; }\n]",
    ),
    // ── Charts ──
    (
        "data",
        "|chart| { categories: \"a\", \"b\", \"c\"; } [\n  |bars| { data: 4, 8, 6; }\n]",
    ),
    (
        "fn",
        "|chart| [\n  |axis| { side: bottom; range: 0 10; }\n  |axis| { side: left; }\n  |line| { fn: (x * x); }\n]",
    ),
    (
        "labels",
        "|chart| { categories: \"a\", \"b\"; } [\n  |line| { data: 4, 8; labels: \"lo\", \"hi\"; marker: dot; }\n]",
    ),
    (
        "curve",
        "|chart| { categories: \"a\", \"b\", \"c\"; } [\n  |line| { data: 4, 8, 6; curve: smooth; }\n]",
    ),
    (
        "baseline",
        "|chart| { categories: \"a\", \"b\", \"c\"; } [\n  |area| { data: 4, 8, 6; baseline: 2; }\n]",
    ),
    (
        "axis",
        "|chart| { categories: \"a\", \"b\"; } [\n  |axis#y| { side: left; }\n  |bars| { data: 4, 8; axis: y; }\n]",
    ),
    (
        "bars",
        "|chart| { categories: \"a\", \"b\"; bars: stacked; } [\n  |bars| \"one\" { data: 4, 8; }\n  |bars| \"two\" { data: 3, 6; }\n]",
    ),
    (
        "categories",
        "|chart| { categories: \"Jan\", \"Feb\", \"Mar\"; } [\n  |bars| { data: 4, 8, 6; }\n]",
    ),
    (
        "hole",
        "|pie| { hole: 0.5; } [\n  |slice| { value: 3; }\n  |slice| { value: 7; }\n]",
    ),
    (
        "legend",
        "|chart| { categories: \"a\", \"b\"; legend: bottom; } [\n  |bars| \"one\" { data: 4, 8; }\n  |bars| \"two\" { data: 3, 6; }\n]",
    ),
    (
        "tooltip",
        "|chart| { categories: \"a\", \"b\"; tooltip: always; } [\n  |bars| { data: 4, 8; }\n]",
    ),
    (
        "value",
        "|pie| [\n  |slice| \"a\" { value: 3; }\n  |slice| \"b\" { value: 7; }\n]",
    ),
    (
        "at",
        "|chart| { categories: \"a\", \"b\"; } [\n  |axis#y| { side: left; }\n  |bars| { data: 4, 8; }\n  |mark| \"target\" { at: 6; axis: y; }\n]",
    ),
    (
        "side",
        "|chart| { categories: \"a\", \"b\"; } [\n  |axis| { side: bottom; }\n  |bars| { data: 4, 8; }\n]",
    ),
    (
        "range",
        "|chart| { categories: \"a\", \"b\"; } [\n  |axis| { side: left; range: 0 100; }\n  |bars| { data: 40, 80; }\n]",
    ),
    (
        "step",
        "|chart| { categories: \"a\", \"b\", \"c\"; } [\n  |axis| { side: left; range: 0 90; step: 30; }\n  |bars| { data: 40, 80, 60; }\n]",
    ),
    (
        "ticks",
        "|chart| { categories: \"a\", \"b\"; } [\n  |axis| { side: left; ticks: 0, 25, 50, 75, 100; }\n  |bars| { data: 40, 80; }\n]",
    ),
    (
        "unit",
        "|chart| { categories: \"a\", \"b\"; } [\n  |axis| { side: left; unit: \"%\"; }\n  |bars| { data: 40, 80; }\n]",
    ),
    (
        "gridlines",
        "|chart| { categories: \"a\", \"b\"; } [\n  |axis| { side: left; gridlines: none; }\n  |bars| { data: 40, 80; }\n]",
    ),
    (
        "format",
        "|chart| { categories: \"a\", \"b\"; format: decimal 1; } [\n  |bars| { data: 4, 8; }\n]",
    ),
    // ── Sequence ──
    (
        "place",
        "{ layout: sequence; }\n\n|box#a| \"A\"\n|box#b| \"B\"\n|note| \"annotates\" { place: over a; }\na -> b \"msg\"",
    ),
    (
        "activation",
        "{ layout: sequence; activation: none; }\n\n|box#a| \"A\"\n|box#b| \"B\"\na -> b \"msg\"",
    ),
    // ── Drawing ──
    (
        "tol",
        "|drawing| [\n  |rect#p| { width: 60; height: 40; }\n  p:top (-) p:bottom { side: left; tol: 0.2 -0.05; }\n]",
    ),
    (
        "characteristic",
        "|drawing| [\n  |rect| { width: 40; height: 24; }\n  |feature-control| { characteristic: flatness; tol: 0.2; }\n]",
    ),
    (
        "zone",
        "|drawing| [\n  |rect| { width: 40; height: 24; }\n  |feature-control| \"position\" { tol: 0.1; zone: diameter; }\n]",
    ),
    (
        "material",
        "|drawing| [\n  |rect| { width: 40; height: 24; }\n  |feature-control| \"position\" { tol: 0.1; material: maximum; }\n]",
    ),
    (
        "datums",
        "|drawing| [\n  |rect#p| { width: 80; height: 50; }\n  p:bottom >- \"A\"\n  p:top >- \"B\"\n  |feature-control| \"position\" { tol: 0.1; datums: A, B; translate: 0 44; }\n]",
    ),
    (
        "modifiers",
        "|drawing| [\n  |rect| { width: 40; height: 24; }\n  |feature-control| \"position\" { tol: 0.1; modifiers: projected 20; }\n]",
    ),
    (
        "project",
        "|drawing| [\n  |rect#p| { width: 80; height: 40; }\n  p:top-left (-) p:top-right { side: top; project: horizontal; }\n]",
    ),
    (
        "facing",
        "|drawing| [\n  |oval#od| { width: 60; height: 60; fill: none; stroke: --stroke-dark; }\n  |plane#a| \"A\" { at: 0; facing: right; }\n]",
    ),
    (
        "of",
        "|page| { align: origin; } [\n  |drawing#front| [\n    |oval#od| { width: 60; height: 60; fill: none; stroke: --stroke-dark; }\n    |magnifier#c| \"C\" { width: 24; }\n  ]\n  |drawing#det| { of: c; }\n]",
    ),
    (
        "title",
        "|page| [\n  |title-block| { title: \"Socket cap screw\"; }\n]",
    ),
    (
        "drawing-number",
        "|page| [\n  |title-block| { drawing-number: \"DIN 912 — M8 × 40\"; }\n]",
    ),
    (
        "revision",
        "|page| [\n  |title-block| { revision: \"A\"; }\n]",
    ),
    (
        "sheet-number",
        "|page| [\n  |title-block| { sheet-number: \"1/1\"; }\n]",
    ),
    (
        "date",
        "|page| [\n  |title-block| { date: \"2026-07-08\"; }\n]",
    ),
    ("author", "|page| [\n  |title-block| { author: \"AM\"; }\n]"),
    (
        "approved",
        "|page| [\n  |title-block| { approved: \"RB\"; }\n]",
    ),
    (
        "department",
        "|page| [\n  |title-block| { department: \"Engineering\"; }\n]",
    ),
    (
        "reference",
        "|page| [\n  |title-block| { reference: \"REF-001\"; }\n]",
    ),
    (
        "document-type",
        "|page| [\n  |title-block| { document-type: \"Drawing\"; }\n]",
    ),
    (
        "status",
        "|page| [\n  |title-block| { status: \"Released\"; }\n]",
    ),
    (
        "density",
        "{ density: 4; }\n\n|drawing| [\n  |rect| { width: 40; height: 20; }\n]",
    ),
    // ── Links ──
    (
        "clearance",
        "|box#a| \"A\"\n|box#b| \"B\"\na -> b { clearance: 24; }",
    ),
    (
        "routing",
        "|box#a| \"A\"\n|box#b| \"B\"\na -> b { routing: orthogonal; }",
    ),
    (
        "along",
        "|box#a| \"A\"\n|box#b| \"B\"\na -> b \"mid\" { along: 0.35; }",
    ),
];
