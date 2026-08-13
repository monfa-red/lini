//! Page scoping [SPEC 18]: two Lini SVGs inlined into one HTML document share
//! that document's global namespaces — the CSS selector space and the id space.
//! Every name Lini writes into either is content-addressed, so two figures
//! collide only on names whose definitions are identical (where sharing is
//! correct) and never on names that mean different things. Counter-based ids
//! and a shared `.lini` selector head silently cross-wired such a page.

use super::*;

/// The scope class the root `<svg>` wears — the head of every selector its
/// `<style>` emits.
fn scope_of(svg: &str) -> String {
    let tag = svg.lines().next().expect("root tag");
    let classes = tag
        .split_once(r#"class=""#)
        .and_then(|(_, r)| r.split_once('"'))
        .expect("root class list")
        .0;
    classes
        .split_whitespace()
        .find(|c| c.starts_with("lini-scope-"))
        .expect("a scope class on the root")
        .to_string()
}

/// Every id the SVG publishes into the document (the leading space keeps
/// `data-id=` — a node's own name, not a document id — out).
fn ids(svg: &str) -> Vec<&str> {
    scrape_to(svg, r#" id=""#, '"')
}

/// Every selector the `<style>` block keys on, one per rule line.
fn selectors(svg: &str) -> Vec<&str> {
    svg.lines()
        .map(str::trim)
        .filter(|l| l.starts_with('.') || l.starts_with(":root"))
        .filter_map(|l| l.split_once(" {"))
        .map(|(sel, _)| sel)
        .collect()
}

#[test]
fn every_emitted_rule_is_scoped_to_its_own_figure() {
    let svg = render_raw("{\n  |svc::box| { fill: --teal-wash; }\n}\n|svc#w| \"W\"\n");
    let scope = scope_of(&svg);
    // Every alternative names the figure's own root — as the subject
    // (`.scope .lini-box`) or as the subject under a theme ancestor
    // (`[data-theme="dark"] .scope`). `:root` is the standalone-file arm of the
    // variable block, which reaches no second figure.
    let head = format!(".{scope}");
    for sel in selectors(&svg) {
        assert!(
            sel.split(',')
                .all(|s| s.contains(&head) || s.trim() == ":root"),
            "selector {sel:?} is not scoped to {scope}: {svg}"
        );
    }
}

#[test]
fn two_figures_on_one_page_do_not_share_a_selector_scope() {
    // The reported bug: figure 2's structural `.lini .lini-block { fill: none }`
    // out-ordered figure 1's template rule for the same node, so every
    // template-styled node in figure 1 went transparent.
    let one = render_raw("{\n  |svc::box| { fill: --teal-wash; }\n}\n|svc#w| \"W\"\n");
    let two = render_raw("|box#b| \"B\"\n");
    assert_ne!(scope_of(&one), scope_of(&two), "{one}\n{two}");
    assert!(
        !two.contains(&scope_of(&one)) && !one.contains(&scope_of(&two)),
        "neither figure names the other's scope"
    );
}

#[test]
fn two_figures_never_publish_the_same_id_for_different_defs() {
    let one = render_raw("|box#a| \"A\" { fill: linear-gradient(135, --sky-wash, --sky-ink); }\n");
    let two =
        render_raw("|box#b| \"B\" { fill: linear-gradient(135, --rose-wash, --rose-ink); }\n");
    let (a, b) = (ids(&one), ids(&two));
    assert!(!a.is_empty() && !b.is_empty(), "{one}\n{two}");
    for id in &a {
        assert!(
            !b.contains(id),
            "id {id:?} names a different def in each figure: {one}\n{two}"
        );
    }
}

#[test]
fn a_shared_id_names_the_same_def_in_both_figures() {
    // The other half of the contract: content-addressing *does* let two figures
    // land on one name — and that is correct exactly when the definitions match,
    // since `url(#…)` then resolves to an equal def either way.
    let grad = "{ fill: linear-gradient(135, --sky-wash, --sky-ink); }";
    let one = render_raw(&format!("|box#a| \"A\" {grad}\n"));
    let two = render_raw(&format!("|box#b| \"B\" {grad}\n"));
    let (a, b) = (ids(&one), ids(&two));
    assert_eq!(a, b, "same gradient, same id: {one}\n{two}");
    let def = |svg: &str| {
        svg.lines()
            .find(|l| l.contains("<linearGradient"))
            .expect("gradient def")
            .trim()
            .to_string()
    };
    assert_eq!(def(&one), def(&two), "…and the same definition behind it");
}

#[test]
fn a_defs_id_is_stable_against_unrelated_edits() {
    // Content-addressing's dividend over a counter: adding a node no longer
    // renumbers the gradient a stylesheet or a host page may reference.
    let grad = "{ fill: linear-gradient(135, --sky-wash, --sky-ink); }";
    let one = render_raw(&format!("|box#a| \"A\" {grad}\n"));
    let two = render_raw(&format!("|box#z| \"Z\"\n|box#a| \"A\" {grad}\n"));
    assert_eq!(ids(&one), ids(&two), "{one}\n{two}");
}
