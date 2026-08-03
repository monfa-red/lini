//! Components and pins [SPEC 16.2]: `|J|`'s generated `pins: N`, the
//! bilateral split into anonymous side rails under the part's pose, each
//! pin's `translate:` slide along its side, and the per-pin chrome (stub,
//! displayed name, number readout).

use super::super::Lower;
use super::super::pose::{Pose, Side};
use super::{
    bare_node, id, lowered, lowered_chain, lowered_chrome, n, pair, readout, style_number, text,
    trim_number,
};
use crate::error::{Code, Error};
use crate::ledger::consts;
use crate::span::Span;
use crate::syntax::ast::{Child, Decl, Node, Value};

/// `|J| { pins: N }` [SPEC 16.2]: N numbered, nameless pins, generated ahead
/// of any authored children.
pub(in crate::desugar) fn expand_connector_pins(
    cx: &Lower,
    chain: &[String],
    style: &[Decl],
) -> Vec<Node> {
    if !chain.iter().any(|t| t == "J") {
        return Vec::new();
    }
    let Some(count) = cx.chain_number(chain, style, "pins") else {
        return Vec::new();
    };
    (1..=count as usize)
        .map(|i| {
            let mut pin = bare_node("pin", Vec::new(), vec![n("number", i as f64)], Vec::new());
            pin.id = Some(format!("p{i}"));
            pin
        })
        .collect()
}

/// The bilateral split + rails [SPEC 16.2], on the **lowered** pin children:
/// pins without a `side:` split first ⌈n/2⌉ left / rest right in declaration
/// order (explicitly-sided pins are excluded from the count); each side's
/// pins wrap in an anonymous rail (scope-transparent — `U7.VS` resolves with
/// no rail in any path), and each pin gains its chrome: the stub, the
/// `number:` readout, and its id as the displayed name when unlabelled.
///
/// The `pose` re-sides them [SPEC 16.1]: the split is authored-frame, then
/// every pin rides the turn to the side it physically lands on — rigidly, so
/// a rail whose reading direction reversed reverses with it. The chrome is
/// dressed for the **landed** side, and every text stays upright.
pub(in crate::desugar) fn assemble_component(
    cx: &Lower,
    pose: Pose,
    style: &mut Vec<Decl>,
    children: &mut Vec<Child>,
) -> Result<(), Error> {
    // Rails read as rows; the body arranges [top / (left · right) / bottom].
    if !style.iter().any(|d| d.name == "direction") {
        style.push(id("direction", "column"));
    }
    let mut pins: Vec<Node> = Vec::new();
    let mut rest: Vec<Child> = Vec::new();
    for c in std::mem::take(children) {
        match c {
            Child::Box(b) if b.classes.iter().any(|k| k == "lini-pin") => pins.push(b),
            other => rest.push(other),
        }
    }
    // The split [SPEC 16.2]: autos only; explicit sides keep theirs.
    let authored: Vec<Option<Side>> = pins
        .iter()
        .map(|p| authored_side(cx, &lowered_chain(p), &p.style))
        .collect();
    let sided = pin_sides(&authored, pose);
    let mut rails: [Vec<Child>; 4] = Default::default();
    for (mut pin, (authored, side, landed)) in pins.into_iter().zip(sided) {
        // A turned pin says where it **landed**: the lowered tree is what the
        // engine reads a forced side back off [SPEC 16.7], so a `side:` left
        // saying `top` would contradict its own rail and stub.
        if authored.is_some() && landed != side {
            set_own(&mut pin, id("side", landed.as_str()));
        }
        slide_pin(cx, &mut pin, landed, pose)?;
        dress_pin(cx, &mut pin, landed)?;
        rails[landed.index()].push(Child::Box(pin));
    }
    for side in Side::ALL {
        if pose.flips(side) {
            reverse_rail(&mut rails[pose.side(side).index()]);
        }
    }
    let [left, right, top, bottom] = rails;
    let rail = |dir: &str, align: &str, kids: Vec<Child>| -> Result<Child, Error> {
        lowered(
            cx,
            &bare_node(
                dir,
                Vec::new(),
                vec![n("gap", 0.0), id("align", align)],
                kids,
            ),
        )
    };
    if !top.is_empty() {
        children.push(rail("row", "center", top)?);
    }
    let mut middle = Vec::new();
    if !left.is_empty() {
        middle.push(rail("column", "start", left)?);
    }
    if !right.is_empty() {
        middle.push(rail("column", "end", right)?);
    }
    match middle.len() {
        0 => {}
        1 => children.extend(middle),
        _ => children.push(lowered(
            cx,
            &bare_node(
                "row",
                Vec::new(),
                vec![n("gap", 24.0), id("align", "center")],
                middle,
            ),
        )?),
    }
    if !bottom.is_empty() {
        children.push(rail("row", "center", bottom)?);
    }
    children.extend(rest);
    Ok(())
}

/// Every pin a `|component|` carries, in the order [`super::super::lower_node`]
/// builds them: the `pins: N` connector's generated ones lead, then the
/// authored `|pin|` children. Read on the **authored** tree — the pose
/// chooser's view of a part it has not lowered yet.
pub(in crate::desugar) fn pins_of(cx: &Lower, node: &Node, chain: &[String]) -> Vec<Node> {
    let mut out = expand_connector_pins(cx, chain, &node.style);
    out.extend(node.children.iter().filter_map(|c| match c {
        Child::Box(b) if cx.authored_chain(b).iter().any(|t| t == "pin") => Some(b.clone()),
        _ => None,
    }));
    out
}

/// A pin's **authored** side: its own `side:` decl, else the rules its chain
/// carries (the one cascade slice this file reads). `None` is an *auto* pin —
/// one the bilateral split places.
pub(in crate::desugar) fn authored_side(
    cx: &Lower,
    chain: &[String],
    style: &[Decl],
) -> Option<Side> {
    cx.chain_ident(chain, style, "side")
        .map(|s| Side::parse(&s).unwrap_or(Side::Left))
}

/// Where every pin of a part lives [SPEC 16.2], in declaration order:
/// `(authored, split, landed)`. The **bilateral split** places the autos —
/// first ⌈n/2⌉ left, the rest right, explicitly-sided pins excluded from the
/// count — and the part's `pose` then re-sides the lot rigidly [SPEC 16.1].
///
/// **The one answer.** `assemble_component` builds its rails from it and
/// `autopose::choose` reads it to learn which way the pin a satellite hangs
/// off points; a second copy of the split would drift a satellite's pose away
/// from the pin it is supposed to face.
pub(in crate::desugar) fn pin_sides(
    authored: &[Option<Side>],
    pose: Pose,
) -> Vec<(Option<Side>, Side, Side)> {
    let left_take = authored.iter().filter(|s| s.is_none()).count().div_ceil(2);
    let mut auto_seen = 0usize;
    authored
        .iter()
        .map(|&a| {
            let side = a.unwrap_or_else(|| {
                auto_seen += 1;
                if auto_seen <= left_take {
                    Side::Left
                } else {
                    Side::Right
                }
            });
            (a, side, pose.side(side))
        })
        .collect()
}

/// Reverse a rail — the pins **and their spans**, because a body prints in
/// span order [SPEC 3]: without the second half the lowered *source* would
/// print the old order and stop being the same program. The spans stay the
/// rail's own, just walked backwards.
fn reverse_rail(rail: &mut [Child]) {
    let mut spans: Vec<Span> = rail.iter().map(|c| c.span()).collect();
    spans.sort_by_key(|s| s.start);
    rail.reverse();
    for (c, span) in rail.iter_mut().zip(spans) {
        if let Child::Box(n) = c {
            n.span = span;
        }
    }
}

/// A **lowered** pin's effective decl for `name` — its own, else the rules its
/// `lini-*` classes carry ([`Lower::chain_decl`], read through the pin's
/// lowered chain). The one cascade slice this file reads, the same one the
/// pose reads: a `side:` or a `translate:` off a define
/// (`{ |sig::pin| { translate: 0 6 } }`) is a pin's side and a pin's slide
/// exactly as an authored one is.
fn pin_decl(cx: &Lower, pin: &Node, name: &str) -> Option<Decl> {
    cx.chain_decl(&lowered_chain(pin), &pin.style, name)
}

/// Write a decl onto the pin itself, replacing its own — an own decl beats the
/// class rule the value may have come from, which is how a turn *lands* on a
/// pin whose side or slide was stated by a define.
fn set_own(pin: &mut Node, decl: Decl) {
    pin.style.retain(|d| d.name != decl.name);
    pin.style.push(decl);
}

/// A pin's `translate:` slides it **along its side** [SPEC 16.2] — a pin
/// lives on its side, so a cross-axis component is an error, wherever the
/// slide was stated. The slide is written in the part's own frame, so the pose
/// turns it with everything else, and the axis is read on the side the pin
/// landed on.
fn slide_pin(cx: &Lower, pin: &mut Node, landed: Side, pose: Pose) -> Result<(), Error> {
    let Some(d) = pin_decl(cx, pin, "translate") else {
        return Ok(());
    };
    let (Some(Value::Number(x)), Some(Value::Number(y))) = (
        d.groups.first().and_then(|g| g.first()),
        d.groups.first().and_then(|g| g.get(1)),
    ) else {
        return Ok(()); // malformed — the validator's to report
    };
    let (x, y) = pose.vector((*x, *y));
    let (across, axis) = if landed.is_vertical() {
        (x, 'x')
    } else {
        (y, 'y')
    };
    if across != 0.0 {
        return Err(Error::at(
            d.span,
            format!(
                "a pin lives on its side — 'translate' slides it along the {} edge; drop the {axis} component",
                landed.as_str()
            ),
        )
        .code(Code::PIN_SLIDE));
    }
    if pose.is_turned() {
        set_own(
            pin,
            Decl {
                name: "translate".into(),
                groups: vec![vec![Value::Number(x), Value::Number(y)]],
                span: d.span,
            },
        );
    }
    Ok(())
}

/// One pin's chrome [SPEC 16.2]: the displayed name (its label, already a
/// text child — or its id), the outward stub, and the `number:` readout
/// beside it. The stub/number are overlays anchored on the pin's side,
/// shifted past the component padding so the stub spans body edge → tip.
fn dress_pin(cx: &Lower, pin: &mut Node, side: Side) -> Result<(), Error> {
    let has_name = pin.children.iter().any(|c| matches!(c, Child::Text(_)));
    if !has_name && let Some(pid) = &pin.id {
        pin.children.insert(0, text(&pid.clone()));
    }
    let pad = 8.0; // the component's body padding [SPEC 8]
    let reach = pad + consts::PIN_STUB;
    let stub_pts = |horiz: bool| {
        let (x, y) = if horiz {
            (consts::PIN_STUB, 0.0)
        } else {
            (0.0, consts::PIN_STUB)
        };
        Decl {
            name: "points".into(),
            groups: vec![
                vec![Value::Number(0.0), Value::Number(0.0)],
                vec![Value::Number(x), Value::Number(y)],
            ],
            span: Span::empty(),
        }
    };
    let (stub_style, num_at) = match side {
        Side::Right => (
            vec![
                stub_pts(true),
                id("pin", "right"),
                pair("translate", reach, 0.0),
            ],
            ("right", consts::PIN_STUB, -7.0),
        ),
        Side::Top => (
            vec![
                stub_pts(false),
                id("pin", "top"),
                pair("translate", 0.0, -reach),
            ],
            ("top", 8.0, -consts::PIN_STUB),
        ),
        Side::Bottom => (
            vec![
                stub_pts(false),
                id("pin", "bottom"),
                pair("translate", 0.0, reach),
            ],
            ("bottom", 8.0, consts::PIN_STUB),
        ),
        Side::Left => (
            vec![
                stub_pts(true),
                id("pin", "left"),
                pair("translate", -reach, 0.0),
            ],
            ("left", -consts::PIN_STUB, -7.0),
        ),
    };
    pin.children.push(lowered_chrome(
        cx,
        &bare_node("line", Vec::new(), stub_style, Vec::new()),
        "lini-pin-stub",
    )?);
    if let Some(num) = style_number(&pin.style, "number") {
        let (anchor, dx, dy) = num_at;
        pin.children.push(lowered_chrome(
            cx,
            &readout(&trim_number(num), anchor, dx, dy),
            "lini-pin-number",
        )?);
    }
    Ok(())
}
