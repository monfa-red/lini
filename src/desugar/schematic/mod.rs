//! Schematic type lowering [SPEC 16]: components and their pins (the
//! bilateral split into anonymous side rails, stub + number chrome), the
//! discrete family and `|opamp|` (registry symbol bodies with generated pin
//! nodes at the glyph's ports), `|label|` (net text / symbol / shape), `|J|`'s
//! `pins: N`, and the per-scope display-ref minting. Everything a static pass
//! can know lowers **here** — generated nodes wearing generated classes whose
//! look states once as a CSS rule ([SPEC 18]'s class-diff law); the Phase 4
//! engine adds placement, never structure.

pub(crate) mod arity;
pub(crate) mod chain;
mod family;
mod pins;

pub(crate) use arity::authored_terminal_ids;
pub(crate) use chain::chains;
use family::{LABEL_SYMBOLS, variant_names};
pub(crate) use family::{
    PartNode, Role, SchKind, part_glyph, part_pin_ids, role, sch_kind, schematic_type,
    terminal_facing, terminal_ids, walk_pins,
};
pub(super) use pins::{
    assemble_component, authored_side, expand_connector_pins, pin_sides, pins_of,
};

use super::Lower;
use super::pose::Pose;
use crate::error::Error;
use crate::ledger::consts;
use crate::span::Span;
use crate::syntax::ast::{Child, Decl, Node, TextNode, Value};

/// A **lowered** node's type chain, **base→derived**: its `lini-*` classes
/// with the prefix stripped, reversed. Lowering leaves the chain behind as
/// classes [SPEC 16.7] — worn most-derived first — so the reversal is what
/// makes this the same order [`crate::desugar::types`] hands the authored
/// walk, which is the order [`Lower::chain_decl`] reads (it walks `.rev()`,
/// derived tier first). Un-reversed, a define's `prefix:` or `side:` lost to
/// its own base.
pub(super) fn lowered_chain(node: &Node) -> Vec<String> {
    node.classes
        .iter()
        .rev()
        .filter_map(|k| k.strip_prefix("lini-").map(str::to_string))
        .collect()
}

// ───────────────────────── shared builders ─────────────────────────

fn decl(name: &str, values: Vec<Value>) -> Decl {
    Decl {
        name: name.into(),
        groups: vec![values],
        span: Span::empty(),
    }
}
fn n(name: &str, v: f64) -> Decl {
    decl(name, vec![Value::Number(v)])
}
fn id(name: &str, v: &str) -> Decl {
    decl(name, vec![Value::Ident(v.into())])
}
fn pair(name: &str, a: f64, b: f64) -> Decl {
    decl(name, vec![Value::Number(a), Value::Number(b)])
}

fn bare_node(ty: &str, classes: Vec<String>, style: Vec<Decl>, children: Vec<Child>) -> Node {
    let mut n = super::synth::node(ty, Span::empty());
    n.classes = classes;
    n.style = style;
    n.children = children;
    n
}

fn text(s: &str) -> Child {
    Child::Text(TextNode {
        text: s.into(),
        classes: Vec::new(),
        style: Vec::new(),
        style_span: None,
        span: Span::empty(),
    })
}

/// A block-wrapped overlay text (a readout) — text cannot `pin`, so the block
/// carries the anchor and the generated class carries the look [SPEC 18].
fn readout(s: &str, pin: &str, dx: f64, dy: f64) -> Node {
    bare_node(
        "block",
        Vec::new(),
        vec![id("pin", pin), pair("translate", dx, dy)],
        vec![text(s)],
    )
}

/// Lower a generated node through the one node path, then seat its chrome
/// class **first** in the worn list — the most-derived position, so the
/// class's one CSS rule wins the type-tier fold and the element carries no
/// inline `style=` diff [SPEC 18]. Idempotent: the node round-trips as
/// already-lowered with the order intact.
fn lowered_chrome(cx: &Lower, node: &Node, class: &str) -> Result<Child, Error> {
    let mut n = super::lower_node(cx, node, super::Nest::NONE)?;
    n.classes.insert(0, class.into());
    Ok(Child::Box(n))
}

/// Lower a generated node through the one node path (no chrome class).
fn lowered(cx: &Lower, node: &Node) -> Result<Child, Error> {
    Ok(Child::Box(super::lower_node(cx, node, super::Nest::NONE)?))
}

/// A part's value readout [SPEC 16.2/16.3] — the smart label as chrome, at the
/// seat [`readout_at`] gives its family.
pub(super) fn value_readout(
    cx: &Lower,
    s: &str,
    kind: SchKind,
    pose: Pose,
) -> Result<Child, Error> {
    let (pin, dx, dy) = readout_at(kind, pose, true);
    lowered_chrome(cx, &readout(s, pin, dx, dy), "lini-part-value")
}

/// Where a part's **ref** and **value** readouts sit [SPEC 16.2]: above a
/// component — the ref on top, the value under it — and beside a symbol-bodied
/// part's drawing, ref above and value below. The one seat table, so the two
/// readouts of one part can never drift apart.
///
/// A **turned** part stands on its wire, which runs straight down the column
/// above and below it: the pair moves **beside** the symbol instead, stacked
/// about its middle [`consts::READOUT_OFFSET`] off the axis — far enough that
/// the wire's own corridor stays clear. `translate:` on the styled label
/// overrides either, as ever.
///
/// **The offsets carry the text's own height.** `pin:` aligns *edges*, so a
/// readout pinned to the part's top edge and nudged by `d` stands `d − line`
/// clear of it: the seats below add the line back, and the gap that remains is
/// [`consts::READOUT_GAP`] off the part and [`consts::READOUT_STACK`] between
/// the two readouts — the numbers the eye actually reads.
fn readout_at(kind: SchKind, pose: Pose, value: bool) -> (&'static str, f64, f64) {
    let line = consts::REF_FONT;
    let out = line + consts::READOUT_GAP;
    match kind {
        // A component is a box whichever way its pins landed: its readouts
        // stack above it, the ref one line clear of the value.
        SchKind::Component => (
            "top",
            0.0,
            if value {
                -out
            } else {
                -(out + line + consts::READOUT_STACK)
            },
        ),
        _ if pose.is_turned() => (
            "center",
            consts::READOUT_OFFSET,
            (line + consts::READOUT_STACK) / 2.0 * if value { 1.0 } else { -1.0 },
        ),
        _ if value => ("bottom", 0.0, out),
        _ => ("top", 0.0, -out),
    }
}

/// One fragment of a schematic glyph as a `|path|` node. The pose **re-lays**
/// it [SPEC 16.1]: the turned `d` is real geometry, so the part sizes,
/// obstructs and renders turned with no transform in sight.
///
/// Every fragment is drawn **over the glyph's own box**, not over its ink: the
/// data is prefixed with the box's two corners (moves, which draw nothing but
/// bound the path), so the linework and the solid detail lay out as one
/// rectangle each, exactly on top of one another, and a fragment carrying only
/// the leads still sizes the part. It is the registry's `width`/`height` that
/// says how big a symbol is, and this is where that is enforced.
fn symbol_path(glyph: &crate::glyph::Glyph, frag: &str, pose: Pose, overlay: bool) -> Node {
    let d = frag
        .strip_prefix(r#"<path d=""#)
        .and_then(|f| f.strip_suffix(r#""/>"#))
        .expect("a schematic glyph fragment is one <path d=…/>");
    let (w, h) = (glyph.width, glyph.height);
    let d = pose.path(&format!("M 0 0 M {w} {h} {d}"), w, h);
    let mut style = vec![decl("path", vec![Value::String(d)])];
    if overlay {
        style.push(id("pin", "center"));
    }
    bare_node("path", Vec::new(), style, Vec::new())
}

/// Seat a part's glyph **ahead of** everything the author wrote — the one
/// place either symbol lowering ([`symbol_body`], [`label_body`]) adds it, so
/// a discrete, an opamp and a label can never drift apart on the rule below.
///
/// A glyph is one stroked `Line` fragment and, where its drawing has filled
/// detail — a capacitor's plates, a transistor's base bar and arrowhead — one
/// `Solid` fragment beside it [SPEC 16.3]. The linework lays out in flow and
/// gives the part its box; the solid rides as a `pin: center` **overlay** over
/// the same box, so it never stacks under the linework and never grows the
/// part.
///
/// The linework must be **inserted, never appended**. A part's readouts (the
/// ref, the value, the ports) are `pin:` overlays, so their position in the
/// body is free; the glyph is **in flow**, and so is any text the author wrote
/// as content (`|R#r1| [ "1k" ]`), so *their* order is layout. A generated node
/// carries an empty span and `fmt` emits a body in span order, which puts the
/// glyph first no matter where it sits in the tree — so appending made the AST
/// stack the text over the glyph while the lowered source stacked the glyph
/// over the text. Two programs, one source: `tests/oracle.rs`'s fixed point.
fn seat_glyph(
    cx: &Lower,
    children: &mut Vec<Child>,
    glyph: &crate::glyph::Glyph,
    pose: Pose,
    classes: (&str, &str),
) -> Result<(), Error> {
    for (role, frag) in glyph.frags.iter().rev() {
        let solid = *role == crate::icon::Role::Solid;
        let class = if solid { classes.1 } else { classes.0 };
        let node = symbol_path(glyph, frag, pose, solid);
        children.insert(0, lowered_chrome(cx, &node, class)?);
    }
    Ok(())
}

/// A zero-size pin node seated on a glyph port — the wirable terminal
/// (`c24.p1`); Phase 4 reads the registry port for the fixed ordinate.
///
/// Seated `pin: center` with a **centre-relative** offset: the glyph linework
/// is in flow and centres in the part's box, and the box's centre is invariant
/// under the glyph stroke's symmetric bbox inflation — a corner anchor is not,
/// and skewed every port by the half-stroke.
fn port_node(pin_id: &str, port: (f64, f64)) -> Node {
    let mut node = bare_node(
        "block",
        Vec::new(),
        vec![id("pin", "center"), pair("translate", port.0, port.1)],
        Vec::new(),
    );
    node.id = Some(pin_id.into());
    node
}
// ───────────────────────── symbol bodies ─────────────────────────

/// A discrete's / opamp's body [SPEC 16.3]: the registry glyph as a `|path|`
/// child — seated ahead of any authored content by [`seat_glyph`] — plus one
/// wirable zero-size pin node per port, ids per the variant.
/// `wired`: only an **id'd** part generates pin nodes — an anonymous part is
/// scope-transparent [SPEC 9], so its generated `p1` would leak into (and
/// collide in) the parent's scope; it also cannot be wired (no dot-path), so
/// the terminals would be dead weight.
pub(super) fn symbol_body(
    cx: &Lower,
    kind: SchKind,
    pose: Pose,
    chain: &[String],
    node: &Node,
    children: &mut Vec<Child>,
) -> Result<(), Error> {
    // Only an **id'd** part generates pin nodes (`wired`).
    let (style, span, wired) = (&node.style, node.span, node.id.is_some());
    if !matches!(kind, SchKind::Opamp | SchKind::Discrete(_)) {
        return Ok(());
    }
    let want = cx.chain_ident(chain, style, "symbol");
    let glyph = part_glyph(chain, want.as_deref()).ok_or_else(|| {
        let SchKind::Discrete(ty) = kind else {
            unreachable!("|opamp| has one glyph")
        };
        Error::at(
            span,
            format!(
                "unknown symbol '{}' on '|{ty}|' — its variants are {}",
                want.as_deref().unwrap_or_default(),
                variant_names(ty).join(", ")
            ),
        )
    })?;
    let pin_ids = part_pin_ids(chain, want.as_deref());
    seat_glyph(
        cx,
        children,
        glyph,
        pose,
        ("lini-sch-line", "lini-sch-solid"),
    )?;
    if wired {
        let (pw, ph) = if pose.swaps_axes() {
            (glyph.height, glyph.width)
        } else {
            (glyph.width, glyph.height)
        };
        for (pid, port) in pin_ids.iter().zip(glyph.ports) {
            let port = pose.point(*port, glyph.width, glyph.height);
            children.push(lowered(
                cx,
                &port_node(pid, (port.0 - pw / 2.0, port.1 - ph / 2.0)),
            )?);
        }
    }
    Ok(())
}

/// A `|label|`'s body [SPEC 16.4]: the symbol drawing (when `symbol:` names
/// one) ahead of the net text — "text beside it like an icon's", seated by
/// [`seat_glyph`] — and the tag-outline classes for `shape:`.
pub(super) fn label_body(
    cx: &Lower,
    pose: Pose,
    chain: &[String],
    node: &Node,
    style_out: &mut Vec<Decl>,
    classes: &mut Vec<String>,
    children: &mut Vec<Child>,
) -> Result<(), Error> {
    let (style, span) = (&node.style, node.span);
    if let Some(sym) = cx.chain_ident(chain, style, "symbol") {
        if !LABEL_SYMBOLS.contains(&sym.as_str()) {
            let near = crate::suggest::nearest(&sym, LABEL_SYMBOLS.iter().copied(), 1);
            let mut msg = format!("unknown symbol '{sym}'");
            msg.push_str(&crate::suggest::did_you_mean(&near));
            return Err(Error::at(span, msg));
        }
        let glyph = part_glyph(chain, Some(&sym)).expect("a label symbol");
        seat_glyph(
            cx,
            children,
            glyph,
            pose,
            ("lini-sch-tag-line", "lini-sch-tag-solid"),
        )?;
        // The text stands **beside** the drawing, never under it [SPEC 16.4]:
        // the symbol's own edge is the label's connection point, so text below
        // it would sit on the wire arriving there. The author's own
        // `direction:` still wins.
        if !style_out.iter().any(|d| d.name == "direction") {
            style_out.insert(0, id("direction", "row"));
        }
    }
    match cx
        .chain_ident(chain, style, "shape")
        .as_deref()
        .unwrap_or("plain")
    {
        "plain" => {}
        "round" => {
            classes.insert(0, "lini-tag-round".into());
            classes.insert(0, "lini-tag-outline".into());
        }
        // A **flag** — one or both ends drawn to a point [SPEC 16.4]. The
        // point's span is the tag's own box, which nothing knows until the
        // text is measured, so the outline lowers as a `|path|` placeholder
        // and the layout fills it ([`crate::layout::schematic::tag`]); the
        // class here only buys the room the point sits in.
        shape @ ("left" | "right" | "both") => {
            classes.insert(0, "lini-tag-flag".into());
            children.push(lowered_chrome(cx, &tag_flag(shape), "lini-sch-tag-line")?);
        }
        other => {
            return Err(Error::at(
                span,
                format!("'shape' takes plain, left, right, both, or round — not '{other}'"),
            ));
        }
    }
    Ok(())
}

/// A shaped tag's outline as a chrome placeholder [SPEC 16.4]: an out-of-flow
/// `|path|` the layout redraws from the sized label. Its `path:` is a stub —
/// a `|path|` needs one to lay out at all, exactly as a page's `|tick|` needs
/// its `points:`.
fn tag_flag(shape: &str) -> Node {
    bare_node(
        "path",
        Vec::new(),
        vec![
            decl(
                "chrome",
                vec![Value::Ident("tag".into()), Value::Ident(shape.into())],
            ),
            id("pin", "center"),
            decl("path", vec![Value::String("M 0 0".into())]),
        ],
        Vec::new(),
    )
}

// ───────────────────────── display refs ─────────────────────────

/// Mint per-scope display refs [SPEC 16.2]: every part (component lineage or
/// discrete) gains a `.lini-ref` readout — its id verbatim, or a minted
/// `prefix + N` (declaration order, skipping taken names). Display-only:
/// minted refs never become ids, so wiring one stays an unknown endpoint.
/// Idempotent — a part already carrying its readout is skipped, so re-desugar
/// (and hand-mixed lowered sources) never double-mint.
pub(super) fn mint_refs(cx: &Lower, children: &mut [Child]) -> Result<(), Error> {
    let taken: std::collections::HashSet<String> = children
        .iter()
        .filter_map(|c| match c {
            Child::Box(b) => b.id.clone(),
            _ => None,
        })
        .collect();
    let mut counters: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for c in children.iter_mut() {
        let Child::Box(part) = c else { continue };
        let chain = lowered_chain(part);
        let Some(kind) = sch_kind(&chain) else {
            continue;
        };
        if matches!(kind, SchKind::Label) || chain.iter().any(|t| t == "pin") {
            continue;
        }
        if has_ref(part) {
            continue;
        }
        let (anchor, dx, dy) = readout_at(kind, Pose::of_chain(&chain), false);
        let text = match &part.id {
            Some(pid) => pid.clone(),
            None => {
                let prefix = cx
                    .chain_str(&chain, &part.style, "prefix")
                    .or_else(|| match kind {
                        SchKind::Discrete(ty) => Some(ty.to_string()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "U".into());
                let count = counters.entry(prefix.clone()).or_insert(0);
                loop {
                    *count += 1;
                    let candidate = format!("{prefix}{count}");
                    if !taken.contains(&candidate) {
                        break candidate;
                    }
                }
            }
        };
        let readout = readout(&text, anchor, dx, dy);
        part.children
            .push(lowered_chrome(cx, &readout, "lini-ref")?);
    }
    Ok(())
}

fn has_ref(part: &Node) -> bool {
    part.children
        .iter()
        .any(|c| matches!(c, Child::Box(b) if b.classes.iter().any(|k| k == "lini-ref")))
}

// ───────────────────────── style readers ─────────────────────────

fn style_number(style: &[Decl], name: &str) -> Option<f64> {
    style
        .iter()
        .rev()
        .find(|d| d.name == name)
        .and_then(|d| match d.groups.first()?.first()? {
            Value::Number(v) => Some(*v),
            _ => None,
        })
}

fn trim_number(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}
