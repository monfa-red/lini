//! **The documentation compiles** — every fenced block in `SPEC.md`,
//! `ROUTING.md`, `SKILL.md`, and `README.md` is fed to the real compiler, so a
//! doc example can never silently rot [PLAN-PRE-V1 chunk 3]. It has already
//! caught four broken examples once, by hand; this is that sweep, automated.
//!
//! **Compiled by default.** A fence is lini unless a ledger row below says
//! otherwise — so a newly written example is guarded the moment it lands, and
//! the only way to opt out is to write down *why*. Each block runs through
//! `compile_verdict`: the property/lint pass [SPEC 17/21] **and** a full
//! compile, the same verdict `tests/deferred.rs` reads, so a gate that lives
//! in validation cannot hide behind a clean render.
//!
//! **Three ways out, all explicit.** A row classifies one block by index and
//! pins its first line as a fingerprint, so re-ordering or editing a block
//! fails the guard rather than silently exempting the wrong one:
//!
//! - `NotLini` — the fence is another language: the SVG output shapes, the
//!   EBNF grammar, the CLI synopsis, a table of constants or names.
//! - `Fragment` — lini-shaped, but not a file: a menu of alternative spellings
//!   (writing them together duplicates an id), an elided body (`[ … ]`), a
//!   declaration deliberately shown out of its `{ }`.
//! - `Wrapped` — a file once its context is supplied. The block is compiled
//!   between a prefix and a suffix that state exactly what the surrounding
//!   prose already gave the reader: the scope the snippet lives in, the class
//!   it wears, the parts it wires. Wrapping beats skipping — the example still
//!   has to compile.
//!
//! `ROUTING.md` carries a single fence (its `src/routing/` file tree), so its
//! ledger is one `NotLini` row; a lini example added there is compiled.
//!
//! `SKILL.md` is the agent skill and `README.md` the front door — the two
//! documents most read and least tested, so they take the same guard. Their
//! only standing excuses are shell transcripts, a CSS snippet, the CLI
//! synopsis, and the pipeline arrow; every Lini example in both compiles as
//! written, bar the icons block, which is `Wrapped` in the colour classes its
//! prose describes.

use std::path::Path;

/// One fenced block: where it starts in the document, and what is inside it.
struct Block {
    index: usize,
    line: usize,
    first: String,
    body: String,
}

/// How the guard treats one block. Absent from the ledger means "compile it as
/// written" — the conservative default.
enum Kind {
    /// Not the language at all; the reason names what it is instead.
    NotLini(&'static str),
    /// Lini-shaped but not a file; the reason names what stops it.
    Fragment(&'static str),
    /// A file once wrapped — `prefix + block + suffix`.
    Wrapped(&'static str, &'static str),
}

/// `(block index, its first line, how to treat it)`.
type Row = (usize, &'static str, Kind);

/// Split a markdown document into its fenced blocks, in document order.
///
/// Fences in these documents always start at column 0, so the scan is a
/// straight toggle: a ```` ``` ```` opens a block and the next one closes it.
fn fenced_blocks(src: &str) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    let mut lines = src.lines().enumerate();
    while let Some((i, line)) = lines.next() {
        if !line.starts_with("```") {
            continue;
        }
        let mut body = String::new();
        for (_, l) in lines.by_ref() {
            if l.starts_with("```") {
                break;
            }
            body.push_str(l);
            body.push('\n');
        }
        let first = body
            .lines()
            .next()
            .unwrap_or_default()
            .trim_end()
            .to_string();
        out.push(Block {
            index: out.len(),
            line: i + 2, // 1-based line of the block's first body line
            first,
            body,
        });
    }
    out
}

/// Compile every fenced block of `doc` that the ledger does not excuse,
/// reporting **all** breakages at once so one SPEC edit shows its whole blast
/// radius.
#[track_caller]
fn compile_every_block(doc: &str, ledger: &[Row]) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(doc);
    let src =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let blocks = fenced_blocks(&src);

    // The fingerprint check: every row still names the block it was written
    // for. A block inserted, deleted, or rewritten fails here, so an exemption
    // can never drift onto a different example.
    for (index, first, _) in ledger {
        let block = blocks.get(*index).unwrap_or_else(|| {
            panic!(
                "{doc}: the ledger excuses block #{index}, but the document has {} blocks",
                blocks.len()
            )
        });
        assert_eq!(
            &block.first, first,
            "{doc}: block #{index} (line {}) no longer opens with the ledgered line — re-classify it",
            block.line
        );
    }

    let (mut failures, mut excused) = (Vec::new(), Vec::new());
    for block in &blocks {
        let excuse = ledger
            .iter()
            .find(|(i, _, _)| *i == block.index)
            .map(|(_, _, kind)| kind);
        let source = match excuse {
            None => block.body.clone(),
            Some(Kind::Wrapped(prefix, suffix)) => format!("{prefix}{}{suffix}", block.body),
            Some(Kind::NotLini(why) | Kind::Fragment(why)) => {
                excused.push(format!("  #{} (line {}) — {why}", block.index, block.line));
                continue;
            }
        };
        let name = format!("{doc}-block-{}.lini", block.index);
        if let Err(msg) = lini::testing::compile_verdict(&source, &name) {
            failures.push(format!(
                "{doc} block #{} (opens at line {}):\n{msg}\n",
                block.index, block.line
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {doc}'s fenced blocks no longer compile — fix the example, or \
         classify it in this test's ledger.\n\n{}\nnot compiled (the ledger's \
         standing excuses):\n{}",
        failures.len(),
        failures.join("\n"),
        excused.join("\n")
    );
}

/// The `SPEC.md` ledger — every block the guard does **not** compile as
/// written, in document order.
const SPEC_LEDGER: &[Row] = &[
    (
        3,
        r#"|type#id| [ "label" ] [ .class… ] [ { style } ] [ [ children ] ]"#,
        Kind::NotLini("the node-declaration form — the brackets are meta, not syntax"),
    ),
    (
        4,
        r#"|cyl#db| "Postgres" .primary { fill: #eef } ["#,
        Kind::Wrapped("{ .primary { fill: none; } }\n", ""),
    ),
    (
        5,
        r#"|box#api| "API" .hot { fill: red }        // label + class + the node's own style"#,
        Kind::Fragment("two spellings of one node — written together they duplicate '#api'"),
    ),
    (
        7,
        "|box| { … }              // every box (element selector)",
        Kind::NotLini("a selector menu — '{ … }' stands in for a block"),
    ),
    (
        11,
        r#"endpoints op endpoints [op endpoints …] [ "label" ] [ .class… ] [ { style } ] [ [ labels ] ]"#,
        Kind::NotLini("the link-statement form — the brackets are meta, not syntax"),
    ),
    (
        14,
        r#"a -> b "watches"                                // the common case — one label, auto-placed"#,
        Kind::Wrapped("{ .loud { stroke: red; } }\n", ""),
    ),
    (
        15,
        r#"endpoint = ( ident | ident_bars ) { "." ident } [ ":" side ]"#,
        Kind::NotLini("the endpoint grammar [SPEC 9]"),
    ),
    (
        16,
        "cat -> |cyl#db|                    // declare db (empty, per SPEC 3), link to it",
        Kind::Fragment(
            "four spellings of one capsule endpoint — '{ … }' elides a block, and the two '#db' lines collide",
        ),
    ),
    (
        18,
        "--lini-bg            light-dark(white, #1b1b1f)      the scene background",
        Kind::NotLini("the role-variable table [SPEC 10.1]"),
    ),
    (
        19,
        "red  rose  orange  amber  lime  green  teal  sky  blue  purple  gray",
        Kind::NotLini("the palette's hue names [SPEC 10.2]"),
    ),
    (
        23,
        "dim clearance 4 (the drawing scope's link default)",
        Kind::NotLini("the drawing chrome constants [SPEC 10.5]"),
    ),
    (
        24,
        "schematic track gap 60    pin-pitch 20    pin-stub 20    junction 4 (radius)    tag-point 8 (a flag's nose reservation; the nose draws at 45°)",
        Kind::NotLini("the schematic chrome constants [SPEC 10.5]"),
    ),
    (
        25,
        "gap: 8;                     // a literal — bare",
        Kind::Fragment(
            "declarations shown out of their '{ }', to contrast bare values with groups",
        ),
    ),
    (
        26,
        "(r = 40; n = 6; 2 * pi * r / n)   // r, n are locals; the last line is the value",
        Kind::Fragment("one expression group — a value, not a statement"),
    ),
    (
        28,
        "|line| { points: (u*300, 20*sin(2*pi*3*u)); samples: 60 }   // a sine wave",
        Kind::Wrapped("{ wave(a, f) = (u*300, a*sin(2*pi*f*u)); }\n", ""),
    ),
    (
        30,
        r#"api -> db "query""#,
        Kind::Wrapped("{ layout: sequence }\n", ""),
    ),
    (
        31,
        r#"|line| "GLM-5.2" { data: 35 63, 42 72, 84 75; labels: "Non-Thinking", "High", "Max"; marker: circle }"#,
        Kind::Wrapped("|chart| [\n", "]\n"),
    ),
    (
        33,
        r#"|band| "Inject" { range: 1.4 3.1; axis: time; fill: --rose }"#,
        Kind::Wrapped(
            "|chart| [\n|axis#time| { range: 0 5 }\n|line| { data: 1, 2, 3 }\n",
            "]\n",
        ),
    ),
    (
        34,
        "--rose  --teal  --orange  --sky  --amber  --purple  --green  --blue  --lime  --gray",
        Kind::NotLini("the series palette walk, in order [SPEC 14.6]"),
    ),
    (
        36,
        r#"anchor = id { "." id } [ "." index ] [ ":" point ]"#,
        Kind::NotLini("the drawing anchor grammar [SPEC 15.2]"),
    ),
    (
        37,
        "|rect#plate| { width: 120; height: 70 } [",
        Kind::Wrapped("{ layout: drawing }\n", ""),
    ),
    (
        39,
        "nozzle:left || barrel:right              // abut those faces, flush",
        Kind::Fragment("four alternative mates on one pair — together they over-constrain"),
    ),
    (
        41,
        r#"bolt <- "THRU"                              // arrow lands on the hole's rim"#,
        Kind::Wrapped(
            "{ layout: drawing }\n\
             |rect#face| { width: 60; height: 40 }\n\
             |hole#bolt| { width: 8; translate: 0 -30 }\n\
             |sketch#body| { draw: move(-30, 30) right(60):seat }\n",
            "",
        ),
    ),
    (
        42,
        "|page| { sheet: a4 } [",
        Kind::Fragment("the two views' bodies are elided — '[ … ]'"),
    ),
    (
        43,
        "{ layout: drawing }",
        Kind::Fragment("the profile is elided — 'draw: …'"),
    ),
    (
        44,
        r#"|component#U7| "TMC2300-LA-T" ["#,
        Kind::Wrapped("{ layout: schematic }\n|C#c24|\n", ""),
    ),
    (
        45,
        r#"{ layout: schematic; |vm::label| { symbol: power } [ "VM" ] }"#,
        Kind::Wrapped("", "|C#c24|\n|component#U7| [ |pin#VS| ]\n"),
    ),
    (
        46,
        r#"<svg xmlns="http://www.w3.org/2000/svg""#,
        Kind::NotLini("the emitted SVG document [SPEC 18]"),
    ),
    (
        47,
        r#"<g class="lini-node lini-{type} lini-{base} lini-style-{class}""#,
        Kind::NotLini("the emitted SVG for a box [SPEC 18]"),
    ),
    (
        48,
        r#"<g class="lini-link lini-style-{class}" data-from="A" data-to="B">"#,
        Kind::NotLini("the emitted SVG for a link [SPEC 18]"),
    ),
    (
        49,
        "lini [options] <input.lini>",
        Kind::NotLini("the CLI synopsis [SPEC 20]"),
    ),
    (
        50,
        "file        = [ stylesheet ] { drawn }              # setup block, then drawn statements in source order",
        Kind::NotLini("the language grammar [SPEC 22]"),
    ),
];

/// The `ROUTING.md` ledger — one fence, and it is a file tree.
const ROUTING_LEDGER: &[Row] = &[(0, "src/routing/", Kind::NotLini("the router's module map"))];

/// The `SKILL.md` ledger — one shell transcript; every other fence is a Lini
/// example the agent is meant to copy, so every other fence compiles.
const SKILL_LEDGER: &[Row] = &[(
    0,
    "lini d.lini -o d.svg                 # compile; errors are file:line:col with fixes",
    Kind::NotLini("a shell transcript — the CLI's one-liners"),
)];

/// The `README.md` ledger — the shell transcripts, the page-side CSS, the CLI
/// synopsis, the pipeline arrow, and one example wrapped in the classes its
/// prose hands the reader.
const README_LEDGER: &[Row] = &[
    (
        1,
        "cargo install lini            # or, from a clone: cargo install --path .",
        Kind::NotLini("a shell transcript — installing"),
    ),
    (
        2,
        "lini diagram.lini -o diagram.svg     # compile to SVG",
        Kind::NotLini("a shell transcript — the CLI's one-liners"),
    ),
    (
        6,
        "|icon| .teal { symbol: user }                            // two-tone",
        Kind::Wrapped(
            "{\n  .teal { fill: --teal-wash; stroke: --teal-ink; }\n  \
             .amber { fill: --amber-wash; stroke: --amber-ink; }\n  \
             .purple { fill: --purple-wash; stroke: --purple-ink; }\n}\n",
            "",
        ),
    ),
    (
        13,
        ".lini { --lini-accent: #ff6600; }   /* recolour every diagram on the page */",
        Kind::NotLini("page-side CSS — the host stylesheet, not the language"),
    ),
    (
        14,
        "lini [options] <input.lini>",
        Kind::NotLini("the CLI synopsis [SPEC 20]"),
    ),
    (
        15,
        "lini serve samples/        # browse, edit, and render the bundled examples",
        Kind::NotLini("a shell transcript — the preview server"),
    ),
    (
        16,
        "cargo test                          # full suite: unit, snapshot, routing laws",
        Kind::NotLini("a shell transcript — the contributor's gates"),
    ),
    (
        17,
        "lex → parse → desugar → resolve → layout → route → render",
        Kind::NotLini("the compile pipeline, as one arrow [SPEC 19]"),
    ),
];

#[test]
fn every_spec_fenced_block_compiles() {
    compile_every_block("SPEC.md", SPEC_LEDGER);
}

#[test]
fn every_routing_fenced_block_compiles() {
    compile_every_block("ROUTING.md", ROUTING_LEDGER);
}

#[test]
fn every_skill_fenced_block_compiles() {
    compile_every_block("SKILL.md", SKILL_LEDGER);
}

#[test]
fn every_readme_fenced_block_compiles() {
    compile_every_block("README.md", README_LEDGER);
}
