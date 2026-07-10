//! Drawing-scope sugar [SPEC 15.7] — the generated chrome. Drafting's always-
//! drawn lines become **real children** here, in desugar, so they run the full
//! cascade like any node (`|sketch| |centerline| { stroke: none }` removes
//! them). Each carries a `chrome:` marker instead of geometry — the parent's
//! folded shape decides that, at layout (`layout::drawing::chrome`):
//!
//! | Producer | Generates |
//! |---|---|
//! | a fused `mirror:` (an **open** subpath + `mirror:`) | the axis `\|centerline\|` |
//! | a `revolve:` | the axis `\|centerline\|` + the `\|shoulder\|` edge-line seed |
//! | a `thread:` on a revolved sketch | the `\|threadline\|` minor-line seed |
//! | a `thread:` on a round feature | the `\|threadline\|` ¾ arc — internal on a `\|hole\|`, external on plain round geometry |
//! | a `\|hole\|` | its centre-mark crosshair — two axis `\|centerline\|`s |
//! | `pattern: radial(…)` | the `\|pitch-circle\|` ring through the copies |
//! | a `break:` | the `\|breakline\|` pair per group — zigzag / round-stock S |
//!
//! Openness is judged **syntactically** (no `close()` before the next `move()`
//! or the end) — the same rule the pen's `mirror` fuses by; the tests assert
//! the two judgements agree.

use crate::resolve::NodeKind;
use crate::syntax::ast::{Decl, Node, Value};

/// The chrome children a node in a drawing scope generates, parser-shaped —
/// the caller lowers them like authored children.
pub(super) fn chrome_children(node: &Node, kind: NodeKind, chain: &[String]) -> Vec<Node> {
    let mut out = Vec::new();
    if kind == NodeKind::Sketch
        && let Some(axis) = fused_mirror_axis(&node.style)
    {
        out.push(chrome_node("centerline", axis, node));
    }
    // A `revolve:` always draws its axis, and seeds the `|shoulder|` edge
    // lines — the pen clones the seed per sharp diameter change [SPEC 15.3];
    // a `thread:` on it seeds the `|threadline|` minors the same way.
    if kind == NodeKind::Sketch
        && let Some(axis) = revolve_axis(&node.style)
    {
        out.push(chrome_node("centerline", axis, node));
        out.push(chrome_node("shoulder", Value::Ident("edges".into()), node));
        if node.style.iter().any(|d| d.name == "thread") {
            out.push(chrome_node(
                "threadline",
                Value::Ident("thread".into()),
                node,
            ));
        }
    }
    if chain.iter().any(|t| t == "hole") {
        out.push(chrome_node(
            "centerline",
            Value::Ident("x-axis".into()),
            node,
        ));
        out.push(chrome_node(
            "centerline",
            Value::Ident("y-axis".into()),
            node,
        ));
    }
    // A round feature's `thread: pitch` — the ¾ arc, its sense from the type:
    // a `|hole|` is internal, plain round geometry external [SPEC 15.4].
    if kind == NodeKind::Oval
        && !chain
            .iter()
            .any(|t| matches!(t.as_str(), "pitch-circle" | "balloon"))
        && let Some(pitch) = thread_pitch(&node.style)
    {
        let sense = if chain.iter().any(|t| t == "hole") {
            "internal"
        } else {
            "external"
        };
        out.push(chrome_group(
            "threadline",
            vec![
                Value::Ident("thread-arc".into()),
                Value::Ident(sense.into()),
                Value::Number(pitch),
            ],
            node,
        ));
    }
    if has_radial_pattern(&node.style) {
        out.push(chrome_node(
            "pitch-circle",
            Value::Ident("ring".into()),
            node,
        ));
    }
    if kind == NodeKind::Sketch
        && let Some(breaks) = node.style.iter().find(|d| d.name == "break")
    {
        // Two cut edges per comma group, indexed in authored order — the pen
        // fills their geometry from the clipped profile [SPEC 15.3].
        for idx in 0..breaks.groups.len() * 2 {
            out.push(chrome_group(
                "breakline",
                vec![Value::Ident("break".into()), Value::Number(idx as f64)],
                node,
            ));
        }
    }
    out
}

/// The axis a fused `mirror:` draws its centerline on [SPEC 15.7]: the first
/// mirror item — fusing closes **every** open subpath, so later items only
/// duplicate — present iff the `draw:` leaves a subpath open.
fn fused_mirror_axis(style: &[Decl]) -> Option<Value> {
    let axis = style
        .iter()
        .find(|d| d.name == "mirror")?
        .groups
        .first()?
        .first()?
        .clone();
    let draw = style.iter().find(|d| d.name == "draw")?;
    has_open_subpath(draw).then_some(axis)
}

/// Whether a `draw:` leaves any subpath open — drawn calls since the last
/// `move()` with no `close()` before the next `move()` or the end. `circle()`
/// is its own closed subpath and never opens one.
fn has_open_subpath(draw: &Decl) -> bool {
    let mut open = false;
    for v in draw.groups.iter().flatten() {
        let name = match v {
            Value::Call(c) => c.name.as_str(),
            Value::NamedCall(c, _) => c.name.as_str(),
            _ => continue,
        };
        match name {
            "move" | "close" => {
                if name == "move" && open {
                    return true;
                }
                open = false;
            }
            "left" | "right" | "up" | "down" | "line" | "angle" | "arc" | "curve" => open = true,
            _ => {}
        }
    }
    open
}

/// A round feature's `thread: pitch` — one positive number; layout validates
/// the malformed forms [SPEC 15.4].
fn thread_pitch(style: &[Decl]) -> Option<f64> {
    match style
        .iter()
        .find(|d| d.name == "thread")?
        .groups
        .first()?
        .as_slice()
    {
        [Value::Number(p)] if *p > 0.0 => Some(*p),
        _ => None,
    }
}

/// A `revolve:`'s axis — the first value; the pen validates it [SPEC 15.3].
fn revolve_axis(style: &[Decl]) -> Option<Value> {
    style
        .iter()
        .find(|d| d.name == "revolve")?
        .groups
        .first()?
        .first()
        .cloned()
}

fn has_radial_pattern(style: &[Decl]) -> bool {
    style.iter().any(|d| {
        d.name == "pattern"
            && matches!(
                d.groups.first().and_then(|g| g.first()),
                Some(Value::Call(c)) if c.name == "radial"
            )
    })
}

fn chrome_node(ty: &str, chrome: Value, at: &Node) -> Node {
    chrome_group(ty, vec![chrome], at)
}

fn chrome_group(ty: &str, chrome: Vec<Value>, at: &Node) -> Node {
    // Generated children sit *after* the authored ones, so they carry the
    // parent's tail as their span — the printer sorts a body by span, and a
    // parent-headed span would hoist the chrome above the parent's `[ ]`.
    let tail = crate::span::Span::new(at.span.end, at.span.end);
    super::chrome::node(
        ty,
        vec![Decl {
            name: "chrome".into(),
            groups: vec![chrome],
            span: tail,
        }],
        tail,
    )
}

#[cfg(test)]
mod tests {
    use crate::syntax::ast::{Child, Node, Value};

    fn lower(src: &str) -> crate::syntax::ast::File {
        let toks = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(src, &toks).expect("parse");
        crate::desugar::desugar(&file).expect("desugar")
    }

    fn node<'a>(f: &'a crate::syntax::ast::File, id: &str) -> &'a Node {
        f.instances
            .iter()
            .find_map(|c| match c {
                Child::Box(n) if n.id.as_deref() == Some(id) => Some(n),
                _ => None,
            })
            .expect("node")
    }

    /// The lowered chrome children as (worn chrome class, `chrome:` value).
    fn chrome_of(n: &Node) -> Vec<(String, String)> {
        n.children
            .iter()
            .filter_map(|c| match c {
                Child::Box(b) => b.style.iter().find(|d| d.name == "chrome").map(|d| {
                    let v = match d.groups[0].first() {
                        Some(Value::Ident(s)) => s.clone(),
                        Some(Value::Number(x)) => x.to_string(),
                        _ => "?".into(),
                    };
                    let class = b
                        .classes
                        .iter()
                        .find(|c| *c == "lini-centerline" || *c == "lini-pitch-circle")
                        .cloned()
                        .unwrap_or_default();
                    (class, v)
                }),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_fused_mirror_generates_one_centerline_a_closed_one_none() {
        let open = lower(
            "{ layout: drawing }\n|sketch#s| { draw: move(-10, 0) up(5) right(20) down(5); mirror: x-axis; }\n",
        );
        assert_eq!(
            chrome_of(node(&open, "s")),
            vec![("lini-centerline".to_string(), "x-axis".to_string())]
        );
        let closed = lower(
            "{ layout: drawing }\n|sketch#s| { draw: move(0, -8) circle(3); mirror: x-axis y-axis; }\n",
        );
        assert!(
            chrome_of(node(&closed, "s")).is_empty(),
            "duplicated, no axis"
        );
    }

    #[test]
    fn a_hole_gets_its_crosshair_and_radial_its_ring() {
        let f = lower("{ layout: drawing }\n|hole#h| { width: 10; pattern: radial(4, 20) }\n");
        assert_eq!(
            chrome_of(node(&f, "h")),
            vec![
                ("lini-centerline".to_string(), "x-axis".to_string()),
                ("lini-centerline".to_string(), "y-axis".to_string()),
                ("lini-pitch-circle".to_string(), "ring".to_string()),
            ]
        );
    }

    #[test]
    fn a_break_generates_a_breakline_pair_per_group() {
        let f = lower(
            "{ layout: drawing }\n|sketch#s| { draw: move(-90, 0) up(10) right(180) down(10); mirror: x-axis; break: -60 -20, 20 60; }\n",
        );
        // Indexed `chrome: break N` in authored order — the pen fills them;
        // they wear the breakline chrome class like all generated chrome.
        let idx: Vec<f64> = node(&f, "s")
            .children
            .iter()
            .filter_map(|c| match c {
                Child::Box(b) if b.classes.iter().any(|c| c == "lini-breakline") => Some(b),
                _ => None,
            })
            .filter_map(|b| {
                b.style.iter().find(|d| d.name == "chrome").and_then(|d| {
                    match d.groups[0].as_slice() {
                        [Value::Ident(k), Value::Number(i)] if k == "break" => Some(*i),
                        _ => None,
                    }
                })
            })
            .collect();
        assert_eq!(idx, vec![0.0, 1.0, 2.0, 3.0], "two cut edges per group");
    }

    #[test]
    fn chrome_stays_in_the_drawing_scope() {
        // The same hole in a flow gets none — the chrome is drawing-only
        // [SPEC 15]; and re-desugar never duplicates it.
        let flow = lower("|hole#h| { width: 10 }\n");
        assert!(chrome_of(node(&flow, "h")).is_empty());
        let f = lower("{ layout: drawing }\n|hole#h| { width: 10 }\n");
        let twice = crate::desugar::desugar(&f).expect("re-desugar");
        assert_eq!(chrome_of(node(&twice, "h")).len(), 2, "idempotent");
    }

    #[test]
    fn desugar_openness_agrees_with_the_pen_fuse() {
        // The chrome keys on syntactic openness; the pen fuses the same
        // subpaths — the two judgements must never drift [SPEC 15.7].
        for (draw, expect_fused) in [
            ("move(-10, 0) up(5) right(20) down(5)", true),
            ("move(0, 0) right(10) down(10) close()", false),
            ("move(0, -8) circle(3)", false),
            ("move(0, 0) right(8) close() move(0, -20) right(6)", true),
        ] {
            let src = format!("|sketch#s| {{ draw: {draw}; mirror: x-axis; }}\n");
            let toks = crate::lexer::lex(&src).expect("lex");
            let file = crate::syntax::parser::parse(&src, &toks).expect("parse");
            let lowered = crate::desugar::desugar(&file).expect("desugar");
            let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
            let folded =
                crate::layout::drawing::pen::fold(&program.scene.nodes[0], 1.0).expect("fold");
            let parsed = crate::syntax::parser::parse(&src, &toks).expect("parse");
            let style = match &parsed.instances[0] {
                Child::Box(n) => &n.style,
                _ => panic!("a box"),
            };
            let draw_decl = style.iter().find(|d| d.name == "draw").expect("draw");
            assert_eq!(super::has_open_subpath(draw_decl), expect_fused, "{draw}");
            assert_eq!(folded.fused, expect_fused, "the pen agrees on: {draw}");
        }
    }
}
