//! **Label wires** [SPEC 16.5]: the one-ended statement a schematic scope reads
//! as a net tag — `U7.DIAG - "NSTDBY"` mints a `|label|` carrying the net text
//! and wires the pin to it, so resolve only ever sees an ordinary two-ended
//! wire between two declared parts.
//!
//! **Why desugar.** The shape is illegal either side of the resolve gates
//! (`link requires at least two endpoints` outside a drawing, `a leader points
//! back at its feature` inside one), so the scope's reading has to land before
//! them. Desugar already owns the two transforms of exactly this kind —
//! auto-create and capsule hoisting ([`super::capsule`]) — and this runs just
//! after the second, over the same per-scope raw statements.
//!
//! **The marker shapes the tag, and nothing else.** An op's end marker picks
//! the label's `shape:` (`-` plain, `->` right, `-<` left, `-<>` both, `-*`
//! round) exactly as its *line* picks `stroke-style` [SPEC 9]; a settled
//! `shape:` wins ([`settled_shape`]). The marker is then **consumed** off the
//! wire, so no arrowhead is ever drawn where a tag is. Everywhere else in the
//! scope a marker has nothing to shape and errors: a marked part-to-part wire,
//! and a marker at a symbol-form label (`|gnd|` draws its symbol — there is no
//! tag). The op's line part stays free — `--` is a dashed wire.
//!
//! **One law, three spellings.** A net tag can be written three ways — minted
//! from text (`u7.a -> "N"`), referenced (`u7.a -> n1`), or declared inline as
//! a capsule (`u7.a -> |label#n1|`) — and all three shape the same way. That
//! costs nothing because the gather hoists capsules **before** calling this:
//! every terminator reaching [`terminator`] is a plain reference to a declared
//! child, so there is one lookup and one rule. Only the gate's message reaches
//! back for the author's spelling, through the hoist's `from_capsule` residue.

use super::Lower;
use super::schematic::{SchKind, sch_kind};
use crate::ast::{ChainOp, LinkMarker, LinkOp};
use crate::error::{Code, Error};
use crate::span::Span;
use crate::syntax::ast::{
    Child, Decl, Endpoint, EndpointGroup, LabelItem, Link, Node, TextNode, Value,
};
use std::collections::HashSet;

/// Mint every label wire in one schematic scope's raw `links`, and gate the
/// scope's markers. Returns the minted `|label|` declarations, ids stamped, in
/// statement order — the caller seats them among the scope's children (before
/// the pose chooser runs, so a tag is posed like a declared one). `declared`
/// seeds the taken-name set the `lini-label-N` mint skips.
///
/// Idempotent by construction: a lowered scope carries no one-ended wire and no
/// marker, so a re-desugar mints nothing and gates nothing.
pub(super) fn mint(
    cx: &Lower,
    children: &mut [Child],
    links: &mut [Link],
    declared: &HashSet<String>,
) -> Result<Vec<Node>, Error> {
    let mut taken = declared.clone();
    let mut next = 1usize;
    let mut out = Vec::new();
    // The minted tag is a plain `|label|`, so that is the chain its authored
    // `shape:` — an element rule's — is read off.
    let label = [String::from("label")];
    for w in links.iter_mut() {
        let Some(op) = w.op().wire() else { continue };
        // The one-ended form, with its net text: mint the tag and wire to it.
        // Textless, it is no label wire — resolve still owns that diagnosis.
        if w.chain.len() == 1
            && let Some(text) = take_first_text(w)
        {
            let shape = tag_shape(cx, &label, &[], op, w.span, AS_A_TAG)?;
            let id = mint_id(&mut taken, &mut next);
            let span = text.span;
            out.push(tag(&id, text, shape));
            w.chain.push(EndpointGroup {
                endpoints: vec![Endpoint {
                    capsule: None,
                    from_capsule: None,
                    path: vec![id],
                    copy: None,
                    point: None,
                    span,
                }],
            });
            w.ops[0] = plain(op);
            continue;
        }
        gate(cx, children, w)?;
    }
    Ok(out)
}

/// The marker gate over a wire that mints nothing [SPEC 16.5]. A marked hop is
/// legal only where its far end is a **text-form** label, whose tag it then
/// shapes; the marker is consumed either way it is legal.
fn gate(cx: &Lower, children: &mut [Child], w: &mut Link) -> Result<(), Error> {
    for i in 0..w.ops.len() {
        let Some(op) = w.ops[i].wire() else { continue };
        if op.start == LinkMarker::None && op.end == LinkMarker::None {
            continue;
        }
        let far = w
            .chain
            .get(i + 1)
            .filter(|g| g.endpoints.len() == 1)
            .map(|g| &g.endpoints[0]);
        match far.map_or(Terminator::Other, |ep| terminator(cx, children, ep)) {
            Terminator::Text(at) => {
                let shape = match &children[at] {
                    Child::Box(n) => {
                        tag_shape(cx, &cx.authored_chain(n), &n.style, op, w.span, AS_A_WIRE)?
                    }
                    Child::Text(_) => None,
                };
                if let Some(shape) = shape
                    && let Child::Box(n) = &mut children[at]
                {
                    n.style.push(decl("shape", shape));
                }
                w.ops[i] = plain(op);
            }
            Terminator::Symbol(name) => {
                return Err(Error::at(
                    w.span,
                    format!("'{name}' draws its symbol — there is no tag to shape"),
                )
                .code(Code::SCHEMATIC_MARKER));
            }
            // A marked statement with no far end at all is a label wire
            // missing its net name — point at that, not at a plain wire.
            Terminator::Other => {
                let write = if w.chain.len() == 1 {
                    AS_A_TAG
                } else {
                    AS_A_WIRE
                };
                return Err(plain_wire(w.span, write));
            }
        }
    }
    Ok(())
}

/// What a marked hop lands on: a text-form label (the child it names), a
/// symbol-form one (as written, for the message), or anything else.
enum Terminator {
    Text(usize),
    Symbol(String),
    Other,
}

/// Every endpoint reaching here names a **declared child**: the gather hoists
/// capsules first, so `- |gnd|` and `- g1` arrive as the same lookup. Only a
/// direct child can be classified — a deeper path is not desugar's to reach
/// into.
fn terminator(cx: &Lower, children: &[Child], ep: &Endpoint) -> Terminator {
    if ep.path.len() != 1 {
        return Terminator::Other;
    }
    let Some(at) = children
        .iter()
        .position(|c| matches!(c, Child::Box(n) if n.id.as_deref() == Some(ep.path[0].as_str())))
    else {
        return Terminator::Other;
    };
    let Child::Box(n) = &children[at] else {
        return Terminator::Other;
    };
    match is_label(cx, &cx.authored_chain(n), &n.style) {
        Some(true) => Terminator::Symbol(spelling(ep)),
        Some(false) => Terminator::Text(at),
        None => Terminator::Other,
    }
}

/// The endpoint as the **author** wrote it, for the gate's message: a hoisted
/// capsule keeps its bars ([`Endpoint::from_capsule`], the hoist's residue)
/// rather than the id it was rewritten to — with its **authored** id, which the
/// hoist moved to the head of the path. A minted `lini-`prefixed one is the
/// engine's, not the author's, so it stays out.
fn spelling(ep: &Endpoint) -> String {
    match (&ep.from_capsule, ep.path.first()) {
        (Some(ty), Some(id)) if !id.starts_with("lini-") => format!("|{ty}#{id}|"),
        (Some(ty), _) => format!("|{ty}|"),
        (None, _) => ep.path.join("."),
    }
}

/// `Some(draws_a_symbol)` when this chain is a `|label|`, `None` otherwise —
/// the text/symbol split the gate reads [SPEC 16.4].
fn is_label(cx: &Lower, chain: &[String], style: &[Decl]) -> Option<bool> {
    matches!(sch_kind(chain), Some(SchKind::Label))
        .then(|| cx.chain_ident(chain, style, "symbol").is_some())
}

/// The `shape:` this op's end marker asks for, or `None` when the tag keeps the
/// one it has — the marker is plain, or the shape is already settled
/// [SPEC 16.5]. A marker that shapes no tag — any start marker, an ER
/// cardinality end — is the plain-wire error, suggesting `write`.
fn tag_shape(
    cx: &Lower,
    chain: &[String],
    style: &[Decl],
    op: LinkOp,
    span: Span,
    write: &str,
) -> Result<Option<&'static str>, Error> {
    if op.start != LinkMarker::None {
        return Err(plain_wire(span, write));
    }
    let shape = match op.end {
        LinkMarker::None => return Ok(None),
        LinkMarker::Arrow => "right",
        LinkMarker::Crow => "left",
        LinkMarker::Diamond => "both",
        LinkMarker::Dot => "round",
        _ => return Err(plain_wire(span, write)),
    };
    Ok((!settled_shape(cx, chain, style)).then_some(shape))
}

/// Whether a tag's `shape:` is already **settled** — worn by the instance, or
/// stated by an element rule. The marker only ever fills an unsettled one, so
/// this is the single place both halves of the law live: an authored `shape:`
/// outranks every marker, and among markers the **first** to land wins (its
/// push settles the tag, and a later hop reads it here).
///
/// Two decls must not count. [`Lower::chain_decl`] is the wrong reader because
/// it folds the `|label|` template bundle in, and the bundle's own
/// `shape: plain` is precisely the default a marker fills. And a **generated**
/// class def restating that default is the compiler's own echo, not a choice:
/// `lini desugar` emits `.lini-label { shape: plain; … }`, which folds back as
/// an element rule on re-desugar — counting it would make every marker in a
/// lowered file inert (the `is_generated_class` discrimination, [SPEC 18]).
fn settled_shape(cx: &Lower, chain: &[String], style: &[Decl]) -> bool {
    style.iter().any(|d| d.name == "shape")
        || chain.iter().any(|t| {
            cx.rules.get(t).is_some_and(|ds| {
                ds.iter()
                    .any(|d| d.name == "shape" && !restates_default(t, d))
            })
        })
}

/// Whether a rule's decl merely repeats the type's built-in bundle value.
/// Compared as a **single ident**, which is every value `shape:` takes — a decl
/// of any other form restates nothing and counts as the author's.
fn restates_default(ty: &str, d: &Decl) -> bool {
    let Some(v) = lone_ident(d) else { return false };
    crate::ledger::defaults::template_bundle(ty)
        .iter()
        .any(|b| b.name == d.name && lone_ident(b) == Some(v))
}

fn lone_ident(d: &Decl) -> Option<&str> {
    match d.groups.as_slice() {
        [group] => match group.as_slice() {
            [Value::Ident(s)] => Some(s),
            _ => None,
        },
        _ => None,
    }
}

/// The op with every marker consumed — the lowered wire is plain, its line kept
/// (`--` stays a dashed wire).
fn plain(op: LinkOp) -> ChainOp {
    ChainOp::Wire(LinkOp {
        line: op.line,
        start: LinkMarker::None,
        end: LinkMarker::None,
    })
}

/// A marker with nothing to shape, suggesting the statement that was meant.
fn plain_wire(span: Span, write: &str) -> Error {
    Error::at(
        span,
        format!("a schematic wire is plain — markers shape a text label's tag; write '{write}'"),
    )
    .code(Code::SCHEMATIC_MARKER)
}

/// The plain wire a marked part-to-part statement meant.
const AS_A_WIRE: &str = "a - b";
/// The label wire a marker without a tag to shape meant.
const AS_A_TAG: &str = "a -> \"NET\"";

/// The minted tag: a `|label|` whose smart label is the net text [SPEC 16.4],
/// span-seated at that text so `lini desugar` prints it right after the wire
/// that mints it and re-parses to the same order.
fn tag(id: &str, text: TextNode, shape: Option<&str>) -> Node {
    let span = text.span;
    Node {
        id: Some(id.to_string()),
        ty: Some("label".to_string()),
        label: Some(text),
        classes: Vec::new(),
        style: shape.map(|s| vec![decl("shape", s)]).unwrap_or_default(),
        style_span: None,
        children: Vec::new(),
        links: Vec::new(),
        span,
    }
}

fn decl(name: &str, value: &str) -> Decl {
    Decl {
        name: name.to_string(),
        groups: vec![vec![Value::Ident(value.to_string())]],
        span: Span::empty(),
    }
}

/// The reserved `lini-label-N` mint [SPEC 21/23] — 1-based in statement order,
/// skipping taken names, so a re-desugared scope gaining a wire never collides.
fn mint_id(taken: &mut HashSet<String>, next: &mut usize) -> String {
    let mut id = format!("lini-label-{next}");
    while taken.contains(&id) {
        *next += 1;
        id = format!("lini-label-{next}");
    }
    *next += 1;
    taken.insert(id.clone());
    id
}

/// Take the statement's **first** text, in source order (the head label leads
/// its `[ ]` items) — the net name the tag carries. Any further text stays on
/// the wire as its own label, which is exactly what a two-ended wire's net name
/// is [SPEC 16.5].
fn take_first_text(w: &mut Link) -> Option<TextNode> {
    if let Some(t) = w.label.take() {
        return Some(t);
    }
    let at = w
        .labels
        .iter()
        .position(|it| matches!(it, LabelItem::Text(_)))?;
    match w.labels.remove(at) {
        LabelItem::Text(t) => Some(t),
        LabelItem::Node(_) => None,
    }
}
