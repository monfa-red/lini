//! **The emitted hooks are the documented hooks** — every `lini-*` class a
//! figure writes is one SPEC 18 names [PLAN-PRE-V1 chunk 4]. The class set is
//! compat surface: host CSS keys on it, so a class that ships undocumented is
//! frozen API nobody wrote down, and a documented class nothing wears is a
//! promise the engine forgot.
//!
//! **The doc is the fixture.** The accept set is parsed out of `SPEC.md`
//! section 18 itself — the hook-family table and the skeleton SVG's own
//! `class=` attributes — so it cannot drift from the prose the way a
//! hand-copied list would (the same spirit as `tests/spec_blocks.rs`). Two
//! shorthands the table uses are expanded here, once:
//!
//! - a **suffix** (`lini-link-dashed` / `-dotted`) reads against the last full
//!   name's prefix, so `-dotted` is `lini-link-dotted`;
//! - a **parameter** (`lini-marker-{kind}`) is filled from the bare words the
//!   row lists beside it (`arrow`·`dot`·…).
//!
//! What is left is a handful of **parametric families** — `lini-{type}`,
//! `lini-style-{class}`, `lini-hue-{name}`, `lini-level-N`, the `*` wildcards.
//! Each gets one recogniser below, keyed on the placeholder the doc writes, and
//! the recognisers check the *parameter*: a type class must name a real type
//! (a primitive, a built-in template, or one of the file's own `define`s), a
//! hue must be one the palette walk mints. A family the doc grows that the test
//! has no recogniser for fails loudly rather than waving the class through.
//!
//! The **ids** get the same treatment, one law shorter: SPEC 18 reserves the
//! `lini-` prefix for generated names, so every id a figure writes carries it —
//! in both output modes, `--static`'s glyph defs included.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// SPEC 18's accept set: the literal class names, plus the parametric families
/// keyed by their placeholder (`lini-marker-{kind}` → `("lini-marker-", "kind")`).
struct Documented {
    literal: BTreeSet<String>,
    /// `(prefix, placeholder)` — the placeholder is `{…}`'s inside, `N`, or `*`.
    parametric: Vec<(String, String)>,
}

/// The SPEC 18 section's text — from its heading to the next one.
fn section_18() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("SPEC.md");
    let src = std::fs::read_to_string(&path).expect("read SPEC.md");
    let start = src
        .find("\n## 18. SVG Output\n")
        .expect("SPEC.md has a section 18");
    let rest = &src[start + 1..];
    let end = rest[1..].find("\n## ").map(|i| i + 2).unwrap_or(rest.len());
    rest[..end].to_string()
}

/// Every backticked token in `text`, in order.
fn backticked(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else { break };
        out.push(after[..close].to_string());
        rest = &after[close + 1..];
    }
    out
}

/// A single lowercase word — the shape a parameter value takes in the table
/// (`arrow`, `dot`, …), never a declaration or a punctuation token.
fn is_word(t: &str) -> bool {
    is_suffix(t) && t.starts_with(|c: char| c.is_ascii_lowercase())
}

/// …and the shape a `/ -suffix` continuation takes, which may also be a number
/// (`lini-pose-90` / `-180`).
fn is_suffix(t: &str) -> bool {
    !t.is_empty()
        && t.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Parse SPEC 18 into the documented accept set.
fn documented() -> Documented {
    let section = section_18();
    let mut doc = Documented {
        literal: BTreeSet::new(),
        parametric: Vec::new(),
    };

    // The skeleton SVG — the section's first fenced block — names the
    // structural classes as real markup: the root's `lini` + scope class and
    // the two layer groups. Only that fence: the later prose fences show
    // *example* classes (`lini-style-quiet`), not the contract.
    let skeleton = {
        let open = section.find("```").expect("section 18 opens with a fence");
        let body = &section[open + 3..];
        &body[..body.find("```").expect("the fence closes")]
    };
    for attr in skeleton.match_indices("class=\"").map(|(i, _)| {
        let rest = &skeleton[i + 7..];
        rest[..rest.find('"').unwrap_or(0)].to_string()
    }) {
        for class in attr.split_whitespace() {
            if let Some(bare) = class.strip_prefix("lini-scope-") {
                assert_eq!(bare, "HHHHHHHH", "the scope class is a hash form");
                doc.parametric
                    .push(("lini-scope-".to_string(), "HHHHHHHH".to_string()));
            } else if !class.contains('{') {
                doc.literal.insert(class.to_string());
            }
        }
    }

    // …and the hook-family table names the rest, one family per row.
    let mut last_full: Option<String> = None;
    let mut last_param: Option<String> = None;
    for row in section
        .lines()
        .filter(|l| l.starts_with('|') && !l.starts_with("|---"))
        .skip(1)
    {
        let Some(classes) = row.split('|').nth(2) else {
            continue;
        };
        for token in backticked(classes) {
            if let Some(rest) = token.strip_prefix("lini-") {
                if let Some(open) = rest.find('{') {
                    let placeholder = rest[open + 1..].trim_end_matches('}').to_string();
                    let prefix = format!("lini-{}", &rest[..open]);
                    last_param = Some(prefix.clone());
                    doc.parametric.push((prefix, placeholder));
                } else if let Some(prefix) = rest.strip_suffix('*') {
                    doc.parametric
                        .push((format!("lini-{prefix}"), "*".to_string()));
                } else if let Some(prefix) = rest.strip_suffix("-N") {
                    doc.parametric
                        .push((format!("lini-{prefix}-"), "N".to_string()));
                } else {
                    last_full = Some(token.clone());
                    doc.literal.insert(token);
                }
            } else if token.starts_with('-') && !token.starts_with("--") && is_suffix(&token[1..]) {
                // A suffix reads against the last full name's prefix.
                let base = last_full.as_deref().expect("a suffix follows a full name");
                let cut = base.rfind('-').expect("a lini- name has a dash");
                doc.literal
                    .insert(format!("{}{}", &base[..=cut], &token[1..]));
            } else if is_word(&token)
                && let Some(prefix) = &last_param
            {
                // A bare word fills the row's parameter (`lini-marker-{kind}`).
                doc.literal.insert(format!("{prefix}{token}"));
            }
        }
    }
    doc
}

/// Whether `class` is covered by a parametric family — the placeholder decides
/// what its parameter may be.
fn parametric_match(doc: &Documented, class: &str, types: &BTreeSet<String>) -> bool {
    doc.parametric.iter().any(|(prefix, placeholder)| {
        let Some(param) = class.strip_prefix(prefix.as_str()) else {
            return false;
        };
        match placeholder.as_str() {
            // A node's type chain: a primitive, a built-in template, or one of
            // the file's own defines [SPEC 8/18].
            "type" | "base" => types.contains(param),
            // A worn `.style` class — any name the file defined (resolve
            // already rejected an undefined one, [SPEC 21]).
            "class" => !param.is_empty(),
            // The mindmap palette walk [SPEC 8].
            "name" => lini::testing::hue_class_names().iter().any(|h| h == param),
            // A numbered series / tier / level.
            "N" => !param.is_empty() && param.chars().all(|c| c.is_ascii_digit()),
            // The content-addressed scope class [SPEC 18].
            "HHHHHHHH" => param.len() == 8 && param.chars().all(|c| c.is_ascii_hexdigit()),
            // A layout marker's open end (`lini-align-*`).
            "*" => !param.is_empty(),
            other => panic!(
                "SPEC 18 documents a '{{{other}}}' family this test has no recogniser for — \
                 add one beside the others in tests/hooks.rs"
            ),
        }
    })
}

/// Every class token on every element of an SVG document, `<style>` excluded
/// (a rule's selector is the sheet's business — this is what the *elements*
/// wear).
fn emitted_classes(svg: &str) -> BTreeSet<String> {
    let body = match svg.split_once("</style>") {
        Some((_, rest)) => rest,
        None => svg,
    };
    let mut out = BTreeSet::new();
    for (i, _) in body.match_indices("class=\"") {
        let rest = &body[i + 7..];
        let Some(end) = rest.find('"') else { continue };
        out.extend(rest[..end].split_whitespace().map(str::to_string));
    }
    // The root `<svg class="lini lini-scope-…">` sits before the `<style>`.
    if let Some(i) = svg.find("class=\"") {
        let rest = &svg[i + 7..];
        if let Some(end) = rest.find('"') {
            out.extend(rest[..end].split_whitespace().map(str::to_string));
        }
    }
    out
}

/// The showroom is the kitchen sink: core, links, charts, sequences, trees and
/// mindmaps, drawings, sheets, and schematics all ship a sample, so the sweep
/// covers every hook family without a scene written only for the test.
#[test]
fn every_emitted_class_is_documented() {
    let doc = documented();
    // The parse is only worth something if it found the families; a table that
    // moved or lost its backticks would otherwise silently accept nothing.
    assert!(
        doc.literal.len() > 40 && doc.parametric.len() >= 6,
        "SPEC 18's hook table did not parse: {} literals, {} families",
        doc.literal.len(),
        doc.parametric.len()
    );

    let mut undocumented: BTreeMap<String, String> = BTreeMap::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for path in lini::testing::samples() {
        let src = lini::testing::read_sample(&path);
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let Ok(svg) = lini::compile_str_with(&src, &lini::testing::sample_opts()) else {
            continue; // the routing fixtures compile; a real break is the conformance sweep's
        };
        let types: BTreeSet<String> = lini::testing::type_class_names(&src).into_iter().collect();
        for class in emitted_classes(&svg) {
            seen.insert(class.clone());
            if doc.literal.contains(&class) || parametric_match(&doc, &class, &types) {
                continue;
            }
            undocumented.entry(class).or_insert(name.clone());
        }
    }

    assert!(
        undocumented.is_empty(),
        "{} emitted class(es) SPEC 18 does not document — document the hook, or \
         rename it to one that is:\n{}",
        undocumented.len(),
        undocumented
            .iter()
            .map(|(c, s)| format!("  {c}  (first seen in {s})"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    // …and the showroom actually exercises the table, so the ⊆ above is not
    // vacuous: every family is worn by something.
    assert!(
        seen.len() > 100,
        "the sweep saw only {} classes — did the samples stop compiling?",
        seen.len()
    );
}

/// **Every generated id carries the `lini-` prefix** [SPEC 18] — the reservation
/// an author's own id is refused for ("an id may not begin 'lini-'",
/// [SPEC 21](SPEC.md)) is only worth something if the engine spends it. Both
/// output modes: `--static` mints the glyph defs, by far the most numerous
/// family, and they are the ones a short spelling tempts.
#[test]
fn every_generated_id_carries_the_prefix() {
    let mut bare: BTreeMap<String, String> = BTreeMap::new();
    for path in lini::testing::samples() {
        let src = lini::testing::read_sample(&path);
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        for static_mode in [false, true] {
            let opts = lini::Options {
                static_mode,
                ..lini::testing::sample_opts()
            };
            let Ok(svg) = lini::compile_str_with(&src, &opts) else {
                continue;
            };
            for (i, _) in svg.match_indices(" id=\"") {
                let rest = &svg[i + 5..];
                let Some(end) = rest.find('"') else { continue };
                let id = &rest[..end];
                if !id.starts_with("lini-") {
                    bare.insert(id.to_string(), name.clone());
                }
            }
        }
    }
    assert!(
        bare.is_empty(),
        "{} generated id(s) go without the reserved 'lini-' prefix:\n{}",
        bare.len(),
        bare.iter()
            .map(|(id, s)| format!("  {id}  (first seen in {s})"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The showroom does not happen to wear every documented class; each one it
/// misses is listed here with the scene that *would* wear it, so a row nothing
/// can reach stands out as a stale promise rather than hiding in the gap.
/// Every entry compiles below — the reachability check is the render itself,
/// not a grep.
/// A wall with one door and one window — every opening hook in one scene.
const FLOORPLAN_OPENINGS: &str = "{ layout: floorplan; unit: m; scale: 0.05 }\n\
     |wall#w| { draw: move(0, 0) right(6):run; } [\n\
       |door| { on: run; at: 1 }\n\
       |window| { on: run; at: 3.5 }\n\
     ]\n";

const UNSAMPLED: &[(&str, &str)] = &[
    (
        "lini-stray",
        // A link with no legal route [ROUTING.md]: `w` sits inside `a`'s
        // clearance, so the forced left side is walled and the wire is drawn
        // as the honest stray.
        "{ layout: grid; columns: repeat(2, 60); rows: repeat(2, 60); gap: 8; clearance: 10 }\n\
         |box#w| { cell: 1 1; width: 60; height: 60 }\n\
         |box#a| { cell: 2 1; width: 60; height: 60 }\n\
         |box#b| { cell: 2 2; width: 60; height: 60 }\n\
         a:left -> b\n",
    ),
    ("lini-marker-circle", "a -> b { marker-end: circle }\n"),
    (
        "lini-pose-270",
        // The fourth quadrant — the samples turn parts 90° and 180° only.
        "{ layout: schematic }\n|R#r1| { rotate: 270 }\n|C#c1|\nr1 - c1\n",
    ),
    (
        // A floorplan's openings [SPEC 15.11] — the leaf and its quarter swing
        // arc off a door, the sill pair off a window. One scene wears all
        // three; `samples/floorplan.lini` will too, once it lands.
        "lini-door-leaf",
        FLOORPLAN_OPENINGS,
    ),
    ("lini-door-swing", FLOORPLAN_OPENINGS),
    ("lini-window-sill", FLOORPLAN_OPENINGS),
    (
        "lini-net-run-turned",
        // A net run stood on end [SPEC 16.4] — the label's own pose turns it.
        "{ layout: schematic }\n|R#r1|\n|label#n1| \"VM\" { rotate: 90 }\nr1 - n1\n",
    ),
];

#[test]
fn every_documented_class_is_worn() {
    let doc = documented();
    let worn = classes_the_showroom_wears();
    let mut missing = Vec::new();
    for class in &doc.literal {
        if worn.contains(class) || UNSAMPLED.iter().any(|(c, _)| c == class) {
            continue;
        }
        missing.push(class.clone());
    }
    assert!(
        missing.is_empty(),
        "SPEC 18 documents {} class(es) nothing emits — drop the row, fix the \
         name, or add the scene that wears it to this test's ledger:\n  {}",
        missing.len(),
        missing.join("\n  ")
    );
}

#[test]
fn the_unsampled_ledger_still_reaches_its_classes() {
    for (class, src) in UNSAMPLED {
        let svg = lini::compile_str(src).unwrap_or_else(|e| panic!("{class}: {}", e.message));
        assert!(
            emitted_classes(&svg).contains(*class),
            "the ledger's scene for '{class}' no longer emits it:\n{src}"
        );
    }
}

/// The other half of the hook contract: the `data-*` attributes. SPEC 18's
/// fenced blocks show them on the elements that carry them (`data-id` on a
/// box, `data-from` / `data-to` on a wire, `data-theme` in the scope
/// selectors) — an attribute a figure writes that the section never shows is
/// the same undocumented compat surface a stray class is.
#[test]
fn every_emitted_data_attribute_is_documented() {
    let section = section_18();
    let documented: BTreeSet<&str> = section
        .match_indices("data-")
        .map(|(i, _)| {
            let rest = &section[i..];
            let end = rest
                .find(|c: char| !(c.is_ascii_lowercase() || c == '-'))
                .unwrap_or(rest.len());
            &rest[..end]
        })
        .collect();
    assert!(
        documented.len() >= 3,
        "SPEC 18 shows no data attributes: {documented:?}"
    );

    let mut undocumented: BTreeMap<String, String> = BTreeMap::new();
    for path in lini::testing::samples() {
        let src = lini::testing::read_sample(&path);
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let Ok(svg) = lini::compile_str_with(&src, &lini::testing::sample_opts()) else {
            continue;
        };
        for (i, _) in svg.match_indices("data-") {
            let rest = &svg[i..];
            let end = rest
                .find(|c: char| !(c.is_ascii_lowercase() || c == '-'))
                .unwrap_or(rest.len());
            let attr = &rest[..end];
            if !documented.contains(attr) {
                undocumented.insert(attr.to_string(), name.clone());
            }
        }
    }
    assert!(
        undocumented.is_empty(),
        "{} emitted data attribute(s) SPEC 18 does not show:\n{}",
        undocumented.len(),
        undocumented
            .iter()
            .map(|(a, s)| format!("  {a}  (first seen in {s})"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Every class the showroom's figures actually wear.
fn classes_the_showroom_wears() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for path in lini::testing::samples() {
        let src = lini::testing::read_sample(&path);
        if let Ok(svg) = lini::compile_str_with(&src, &lini::testing::sample_opts()) {
            out.extend(emitted_classes(&svg));
        }
    }
    out
}
