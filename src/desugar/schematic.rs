//! Schematic type lowering [SPEC 16]: components and their pins (the
//! bilateral split into anonymous side rails, stub + number chrome), the
//! discrete family and `|opamp|` (registry symbol bodies with generated pin
//! nodes at the glyph's ports), `|label|` (net text / symbol / shape), `|J|`'s
//! `pins: N`, and the per-scope display-ref minting. Everything a static pass
//! can know lowers **here** — generated nodes wearing generated classes whose
//! look states once as a CSS rule ([SPEC 18]'s class-diff law); the Phase 4
//! engine adds placement, never structure.

use super::Lower;
use crate::error::Error;
use crate::ledger::consts;
use crate::span::Span;
use crate::syntax::ast::{Child, Decl, Node, TextNode, Value};

/// The schematic family a resolved chain belongs to [SPEC 16] — the
/// scope-tagged twin of [`crate::glyph::drafting_type`]: one dispatch for the
/// lowering here and the scope gates (Phase 5).
#[derive(Clone, Copy, PartialEq)]
pub(super) enum SchKind {
    /// `|component|` / `|J|` — the pin-bearing box (rails, stubs, numbers).
    Component,
    /// `|opamp|` — component lineage, symbol-bodied like a discrete.
    Opamp,
    /// A discrete two/three-terminal part; carries its type name.
    Discrete(&'static str),
    /// `|label|` and its defines (`|gnd|`, `|nc|`, user power flags).
    Label,
}

pub(super) fn sch_kind(chain: &[String]) -> Option<SchKind> {
    if chain.iter().any(|t| t == "opamp") {
        return Some(SchKind::Opamp);
    }
    if chain.iter().any(|t| t == "component") {
        return Some(SchKind::Component);
    }
    if let Some(d) = chain
        .iter()
        .find_map(|t| super::types::DISCRETES.iter().find(|n| **n == t.as_str()))
    {
        return Some(SchKind::Discrete(d));
    }
    if chain.iter().any(|t| t == "label") {
        return Some(SchKind::Label);
    }
    None
}

/// A discrete's symbol table [SPEC 16.3]: type → (default variant, the
/// variant set), each variant naming its glyph and its pin ids in pin order
/// (the glyph's `ports` order).
struct Variant {
    name: &'static str,
    glyph: &'static str,
    pins: &'static [&'static str],
}
const P12: &[&str] = &["p1", "p2"];
const AK: &[&str] = &["a", "k"];

fn variants(ty: &str) -> &'static [Variant] {
    macro_rules! v {
        ($($n:literal $g:literal $p:expr),+ $(,)?) => { &[$(Variant { name: $n, glyph: $g, pins: $p }),+] };
    }
    match ty {
        "R" => v!("plain" "sch-r" P12),
        "C" => v!("plain" "sch-c" P12, "polarized" "sch-c-polarized" P12),
        "L" => v!("plain" "sch-l" P12),
        "D" => v!(
            "plain" "sch-d" AK,
            "zener" "sch-d-zener" AK,
            "tvs" "sch-d-tvs" AK,
            "schottky" "sch-d-schottky" AK,
        ),
        "LED" => v!("plain" "sch-led" AK),
        "Q" => v!(
            "npn" "sch-q-npn" &["b", "c", "e"],
            "pnp" "sch-q-pnp" &["b", "c", "e"],
            "nfet" "sch-q-nfet" &["g", "d", "s"],
            "pfet" "sch-q-pfet" &["g", "d", "s"],
        ),
        "Y" => v!("plain" "sch-y" P12),
        "F" => v!("plain" "sch-f" P12),
        "FB" => v!("plain" "sch-fb" P12),
        "SW" => v!("toggle" "sch-sw-toggle" P12, "push" "sch-sw-push" P12),
        "BT" => {
            v!("cell" "sch-bt-cell" &["plus", "minus"], "battery" "sch-bt-battery" &["plus", "minus"])
        }
        "V" => v!("dc" "sch-v-dc" &["plus", "minus"], "ac" "sch-v-ac" &["plus", "minus"]),
        "I" => v!("dc" "sch-i" &["plus", "minus"], "ac" "sch-i" &["plus", "minus"]),
        _ => &[],
    }
}

/// The `|label|` symbol vocabulary [SPEC 16.4].
const LABEL_SYMBOLS: &[&str] = &["gnd", "earth", "chassis", "power", "nc", "antenna"];

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
    Node {
        id: None,
        ty: Some(ty.into()),
        label: None,
        classes,
        style,
        style_span: None,
        children,
        links: Vec::new(),
        span: Span::empty(),
    }
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
    let mut n = super::lower_node(cx, node, false)?;
    n.classes.insert(0, class.into());
    Ok(Child::Box(n))
}

/// Lower a generated node through the one node path (no chrome class).
fn lowered(cx: &Lower, node: &Node) -> Result<Child, Error> {
    Ok(Child::Box(super::lower_node(cx, node, false)?))
}

/// A part's value readout [SPEC 16.2/16.3] — the smart label as chrome.
pub(super) fn value_readout(cx: &Lower, s: &str, pin: &str, dy: f64) -> Result<Child, Error> {
    lowered_chrome(cx, &readout(s, pin, 0.0, dy), "lini-part-value")
}

/// The one `Line` fragment of a schematic glyph as a `|path|` node wearing
/// `class` — the registry invariant pins the `<path d="…"/>` shape.
fn symbol_path(glyph: &crate::glyph::Glyph) -> Node {
    let frag = glyph.frags[0].1;
    let d = frag
        .strip_prefix(r#"<path d=""#)
        .and_then(|f| f.strip_suffix(r#""/>"#))
        .expect("a schematic glyph fragment is one <path d=…/>");
    bare_node(
        "path",
        Vec::new(),
        vec![decl("path", vec![Value::String(d.into())])],
        Vec::new(),
    )
}

/// A zero-size pin node seated on a glyph port — the wirable terminal
/// (`c24.p1`); Phase 4 reads the registry port for the fixed ordinate.
fn port_node(pin_id: &str, port: (f64, f64)) -> Node {
    let mut node = bare_node(
        "block",
        Vec::new(),
        vec![
            decl(
                "pin",
                vec![Value::Ident("top".into()), Value::Ident("left".into())],
            ),
            pair("translate", port.0, port.1),
        ],
        Vec::new(),
    );
    node.id = Some(pin_id.into());
    node
}

// ───────────────────────── components & pins ─────────────────────────

/// `|J| { pins: N }` [SPEC 16.2]: N numbered, nameless pins, generated ahead
/// of any authored children.
pub(super) fn expand_connector_pins(cx: &Lower, chain: &[String], style: &[Decl]) -> Vec<Node> {
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
pub(super) fn assemble_component(
    cx: &Lower,
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
    let sided: Vec<Option<String>> = pins.iter().map(|p| style_ident(&p.style, "side")).collect();
    let autos = sided.iter().filter(|s| s.is_none()).count();
    let left_take = autos.div_ceil(2);
    let mut auto_seen = 0usize;
    let (mut left, mut right, mut top, mut bottom) = (vec![], vec![], vec![], vec![]);
    for (mut pin, side) in pins.into_iter().zip(sided) {
        let side = side.unwrap_or_else(|| {
            auto_seen += 1;
            if auto_seen <= left_take {
                "left"
            } else {
                "right"
            }
            .into()
        });
        dress_pin(cx, &mut pin, &side)?;
        match side.as_str() {
            "right" => right.push(Child::Box(pin)),
            "top" => top.push(Child::Box(pin)),
            "bottom" => bottom.push(Child::Box(pin)),
            _ => left.push(Child::Box(pin)),
        }
    }
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

/// One pin's chrome [SPEC 16.2]: the displayed name (its label, already a
/// text child — or its id), the outward stub, and the `number:` readout
/// beside it. The stub/number are overlays anchored on the pin's side,
/// shifted past the component padding so the stub spans body edge → tip.
fn dress_pin(cx: &Lower, pin: &mut Node, side: &str) -> Result<(), Error> {
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
        "right" => (
            vec![
                stub_pts(true),
                id("pin", "right"),
                pair("translate", reach, 0.0),
            ],
            ("right", consts::PIN_STUB, -7.0),
        ),
        "top" => (
            vec![
                stub_pts(false),
                id("pin", "top"),
                pair("translate", 0.0, -reach),
            ],
            ("top", 8.0, -consts::PIN_STUB),
        ),
        "bottom" => (
            vec![
                stub_pts(false),
                id("pin", "bottom"),
                pair("translate", 0.0, reach),
            ],
            ("bottom", 8.0, consts::PIN_STUB),
        ),
        _ => (
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

// ───────────────────────── symbol bodies ─────────────────────────

/// A discrete's / opamp's body [SPEC 16.3]: the registry glyph as a `|path|`
/// child plus one wirable zero-size pin node per port, ids per the variant.
/// `wired`: only an **id'd** part generates pin nodes — an anonymous part is
/// scope-transparent [SPEC 9], so its generated `p1` would leak into (and
/// collide in) the parent's scope; it also cannot be wired (no dot-path), so
/// the terminals would be dead weight.
pub(super) fn symbol_body(
    cx: &Lower,
    kind: SchKind,
    chain: &[String],
    style: &[Decl],
    span: Span,
    wired: bool,
    children: &mut Vec<Child>,
) -> Result<(), Error> {
    let (glyph_name, pin_ids): (&str, &[&str]) = match kind {
        SchKind::Opamp => ("sch-opamp", &["out", "inp", "inn"]),
        SchKind::Discrete(ty) => {
            let set = variants(ty);
            let want = cx.chain_ident(chain, style, "symbol");
            let v = match &want {
                None => &set[0],
                Some(name) => set.iter().find(|v| v.name == name).ok_or_else(|| {
                    let names: Vec<&str> = set.iter().map(|v| v.name).collect();
                    Error::at(
                        span,
                        format!(
                            "unknown symbol '{name}' on '|{ty}|' — its variants are {}",
                            names.join(", ")
                        ),
                    )
                })?,
            };
            (v.glyph, v.pins)
        }
        _ => return Ok(()),
    };
    let glyph = crate::glyph::lookup(glyph_name).expect("a registered schematic glyph");
    children.push(lowered_chrome(cx, &symbol_path(glyph), "lini-sch-line")?);
    if wired {
        for (pid, port) in pin_ids.iter().zip(glyph.ports) {
            children.push(lowered(cx, &port_node(pid, *port))?);
        }
    }
    Ok(())
}

/// A `|label|`'s body [SPEC 16.4]: the symbol drawing (when `symbol:` names
/// one) below the net text, and the tag-outline classes for `shape:`.
pub(super) fn label_body(
    cx: &Lower,
    chain: &[String],
    style: &[Decl],
    span: Span,
    classes: &mut Vec<String>,
    children: &mut Vec<Child>,
) -> Result<(), Error> {
    if let Some(sym) = cx.chain_ident(chain, style, "symbol") {
        if !LABEL_SYMBOLS.contains(&sym.as_str()) {
            let near = crate::suggest::nearest(&sym, LABEL_SYMBOLS.iter().copied(), 1);
            let mut msg = format!("unknown symbol '{sym}'");
            msg.push_str(&crate::suggest::did_you_mean(&near));
            return Err(Error::at(span, msg));
        }
        let glyph = crate::glyph::lookup(&format!("sch-{sym}")).expect("a label symbol");
        children.push(lowered_chrome(
            cx,
            &symbol_path(glyph),
            "lini-sch-tag-line",
        )?);
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
        // The pointed flag ends (`left` / `right` / `both`) draw as the plain
        // outline until Phase 5's marker-driven shapes land the tag path.
        "left" | "right" | "both" => classes.insert(0, "lini-tag-outline".into()),
        other => {
            return Err(Error::at(
                span,
                format!("'shape' takes plain, left, right, both, or round — not '{other}'"),
            ));
        }
    }
    Ok(())
}

// ───────────────────────── display refs ─────────────────────────

/// Mint per-scope display refs [SPEC 16.2]: every part (component lineage or
/// discrete) gains a `.lini-ref` readout — its id verbatim, or a minted
/// `prefix + N` (declaration order, skipping taken names). Display-only:
/// minted refs never become ids, so wiring one stays an unknown endpoint.
/// Idempotent — a part already carrying its readout is skipped, so re-desugar
/// (and hand-mixed lowered sources) never double-mint.
pub(super) fn mint_refs(cx: &Lower, children: &mut [Child]) {
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
        let chain: Vec<String> = part
            .classes
            .iter()
            .filter_map(|k| k.strip_prefix("lini-").map(str::to_string))
            .collect();
        let Some(kind) = sch_kind(&chain) else {
            continue;
        };
        if matches!(kind, SchKind::Label) || chain.iter().any(|t| t == "pin") {
            continue;
        }
        if has_ref(part) {
            continue;
        }
        let (anchor, dy) = match kind {
            SchKind::Component => ("top", -30.0),
            _ => ("top", -12.0),
        };
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
        let readout = readout(&text, anchor, 0.0, dy);
        if let Ok(child) = lowered_chrome(cx, &readout, "lini-ref") {
            part.children.push(child);
        }
    }
}

fn has_ref(part: &Node) -> bool {
    part.children
        .iter()
        .any(|c| matches!(c, Child::Box(b) if b.classes.iter().any(|k| k == "lini-ref")))
}

// ───────────────────────── style readers ─────────────────────────

fn style_ident(style: &[Decl], name: &str) -> Option<String> {
    style
        .iter()
        .rev()
        .find(|d| d.name == name)
        .and_then(|d| match d.groups.first()?.first()? {
            Value::Ident(s) => Some(s.clone()),
            _ => None,
        })
}
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
