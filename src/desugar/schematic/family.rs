//! What a schematic **family** is [SPEC 16.2/16.3/16.4]: the type dispatch,
//! the discrete symbol table, and everything read off it — a part's wirable
//! pin ids, the registry glyph its body draws, which way a terminal faces, and
//! a scope child's placement role.
//!
//! One table, two readers: the lowering next door builds a part from it, and
//! the schematic engine classifies and seats the lowered part from the same
//! rows — an authored chain and a lowered node's `lini-*` classes answer alike.

use super::super::pose::{Side, facing};
use super::super::types::DISCRETES;

/// The schematic family a resolved chain belongs to [SPEC 16] — the
/// scope-tagged twin of [`crate::glyph::drafting_type`]: one dispatch for the
/// lowering here and the scope gates (Phase 5).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum SchKind {
    /// `|component|` / `|J|` — the pin-bearing box (rails, stubs, numbers).
    Component,
    /// `|opamp|` — component lineage, symbol-bodied like a discrete.
    Opamp,
    /// A discrete two/three-terminal part; carries its type name.
    Discrete(&'static str),
    /// `|label|` and its defines (`|gnd|`, `|nc|`, user power flags).
    Label,
}

/// The family of a type chain — the authored one before lowering, or a lowered
/// node's `lini-*` classes with the prefix stripped (the schematic engine's
/// reader: lowering leaves the family behind as a class [SPEC 16.7]).
pub(crate) fn sch_kind<S: AsRef<str>>(chain: &[S]) -> Option<SchKind> {
    let has = |name: &str| chain.iter().any(|t| t.as_ref() == name);
    if has("opamp") {
        return Some(SchKind::Opamp);
    }
    if has("component") {
        return Some(SchKind::Component);
    }
    if let Some(d) = chain
        .iter()
        .find_map(|t| DISCRETES.iter().find(|n| **n == t.as_ref()).copied())
    {
        return Some(SchKind::Discrete(d));
    }
    if has("label") {
        return Some(SchKind::Label);
    }
    None
}

/// The schematic types no *family* answers for [SPEC 16.2/16.6]: a component's
/// terminal and the generated junction dot. Every other schematic type carries
/// `component`, `label` or a discrete in its chain, so [`sch_kind`] already
/// names it — `|J|`, `|opamp|`, `|gnd|` and `|nc|` included.
const BARE_TYPES: &[&str] = &["pin", "junction"];

/// The schematic type a chain wears, as the **author** spelled it — the
/// out-of-scope gate's subject [SPEC 21]. `None` outside the family; the
/// most-derived name for the message, so a `|gnd|` says `'|gnd|'` and a
/// `|myres::R|` says `'|myres|'`. `|schematic|` is deliberately not a member:
/// it *creates* the scope.
///
/// Takes the chain **most-derived first** — a lowered node's `type_chain`, the
/// order the layout gate reads it in.
pub(crate) fn schematic_type<S: AsRef<str>>(chain: &[S]) -> Option<&str> {
    let member = |t: &str| sch_kind(&[t]).is_some() || BARE_TYPES.contains(&t);
    let written = chain.first()?.as_ref();
    chain.iter().any(|t| member(t.as_ref())).then_some(written)
}

/// A schematic scope child's role [SPEC 16.1] — **the** rule, in one place:
/// the schematic engine classifies a placed child through it and the pose
/// chooser an authored one, so a part cannot be posed as a satellite and then
/// seated as an anchor. Arity and placement, never the type: `cell:` promotes
/// whatever it is to an anchor (`translate:` does **not** — it only nudges a
/// child off the seat it already has), a `|label|` seats because its terminal
/// is a connection point, and a part with 1–2 pins seats — so an authored
/// two-pin `|component|` (a jumper) seats like a discrete. A non-part (a
/// `|group|`, a nested `|row|`, a sheet note) has no pin to seat at, so it
/// takes a track.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Role {
    Anchor,
    Satellite,
    /// `pin:` — an out-of-flow overlay on the finished scope box, in neither
    /// the tracks nor the seats (the drawing precedent [SPEC 5/15.8]).
    Pinned,
}

pub(crate) fn role(pinned: bool, placed: bool, kind: Option<SchKind>, pins: usize) -> Role {
    if pinned {
        return Role::Pinned;
    }
    if placed {
        return Role::Anchor;
    }
    match kind {
        Some(SchKind::Label) => Role::Satellite,
        Some(_) if (1..=2).contains(&pins) => Role::Satellite,
        _ => Role::Anchor,
    }
}

/// The wirable pin ids a **symbol-bodied** part carries [SPEC 16.2/16.3] — the
/// variant's ports in glyph-port order. Empty for a `|component|` (its pins are
/// authored `|pin|` children), for a `|label|` (its one connection point is the
/// symbol's), and outside the family. Read by the lowering next door *and* by
/// the schematic engine's pin-arity classifier, which must count an
/// **anonymous** part's pins too — those generate no port nodes at all.
pub(crate) fn part_pin_ids<S: AsRef<str>>(
    chain: &[S],
    symbol: Option<&str>,
) -> &'static [&'static str] {
    match sch_kind(chain) {
        Some(SchKind::Opamp) => OPAMP_PINS,
        Some(SchKind::Discrete(ty)) => variant(ty, symbol).map_or(&[][..], |v| v.pins),
        _ => &[],
    }
}

/// A lowered part as the **pin walk** sees it — the one shape the two trees a
/// part lives in after desugar both offer: resolve's `ResolvedInst` (where
/// arity resolves a pinless landing, [SPEC 16.5]) and layout's `PlacedNode`
/// (where the ports and the role classifier read it). It exists so
/// [`terminal_ids`] is written **once**: a component's terminals are read off
/// its lowered children, and a second copy of that descent would let one stage
/// count a part's pins differently from another.
pub(crate) trait PartNode {
    fn type_chain(&self) -> &[String];
    fn attrs(&self) -> &crate::resolve::AttrMap;
    fn node_id(&self) -> Option<&str>;
    fn kids(&self) -> &[Self]
    where
        Self: Sized;
}

/// The terminals a part offers, **in pin order** [SPEC 16.2/16.3] — `None` for
/// an anonymous one, which shapes the part but can never be named [SPEC 9].
///
/// **The one pin walk.** A `|component|`'s pins are its authored `|pin|`
/// children, found through the anonymous rails desugar wrapped them in; every
/// other part's are its variant's glyph ports, straight off the table above —
/// an **anonymous** symbol part generates no port nodes at all, so the table is
/// the only source that can answer for it. Empty outside the family, and for a
/// `|label|` (its one connection point is the part itself).
pub(crate) fn terminal_ids<P: PartNode>(part: &P) -> Vec<Option<String>> {
    match sch_kind(part.type_chain()) {
        Some(SchKind::Component) => {
            let mut out = Vec::new();
            walk_pins(
                part.kids(),
                &|n: &P| n.type_chain().iter().any(|t| t == "pin"),
                &|n: &P| n.kids(),
                &mut out,
            );
            out.iter()
                .map(|p| p.node_id().map(str::to_string))
                .collect()
        }
        Some(_) => part_pin_ids(part.type_chain(), symbol_of(part).as_deref())
            .iter()
            .map(|p| Some((*p).to_string()))
            .collect(),
        None => Vec::new(),
    }
}

/// **The pin descent**, in pin order: every `|pin|` under these nodes, found
/// *through* whatever wraps it — the anonymous rails desugar builds, an authored
/// `|row|`, any container a body puts its pins in.
///
/// Written once because two trees read it — the authored AST (desugar's pose
/// chooser and arity) and the lowered [`PartNode`] (resolve and layout) — and a
/// shallower copy on either side breaks the invariant both cite: the chooser
/// would count no pins where layout counts two, calling a part an anchor that
/// the engine seats as a satellite. Each caller supplies only its own reading of
/// "this is a pin" and "these are its children".
pub(crate) fn walk_pins<'a, T>(
    nodes: &'a [T],
    is_pin: &dyn Fn(&T) -> bool,
    kids: &dyn Fn(&'a T) -> &'a [T],
    out: &mut Vec<&'a T>,
) {
    for n in nodes {
        if is_pin(n) {
            out.push(n);
        }
        walk_pins(kids(n), is_pin, kids, out);
    }
}

/// The `symbol:` variant a part wears, off its resolved attrs.
fn symbol_of<P: PartNode>(part: &P) -> Option<String> {
    match part.attrs().get("symbol") {
        Some(crate::resolve::ResolvedValue::Ident(s)) => Some(s.clone()),
        _ => None,
    }
}

/// The registry glyph a part's body draws [SPEC 16.3/16.4] — a discrete's
/// variant, `|opamp|`'s triangle, a `|label|`'s symbol. `None` for a
/// `|component|` (it draws no glyph), for a symbol-less label, for an unknown
/// variant name (the lowerings word that error), and outside the family.
pub(crate) fn part_glyph<S: AsRef<str>>(
    chain: &[S],
    symbol: Option<&str>,
) -> Option<&'static crate::glyph::Glyph> {
    let name = match sch_kind(chain)? {
        SchKind::Opamp => "sch-opamp".to_string(),
        SchKind::Discrete(ty) => variant(ty, symbol)?.glyph.to_string(),
        SchKind::Label => format!("sch-{}", symbol?),
        SchKind::Component => return None,
    };
    crate::glyph::lookup(&name)
}

/// Which side of its own body a part's terminal faces [SPEC 16.1] — the
/// connection geometry a satellite chain grows away from, and what the pose
/// chooser turns to face an anchor. Read off the registry in the part's
/// **unposed** frame, so a caller turns it with `Pose::side`. `terminal` is
/// the pin id an endpoint names (`None` — a `|label|`, which has no dot-path
/// terminal — takes the glyph's one port). `None` for a `|component|` (its
/// pins are authored children, sided by the bilateral split) and for a part
/// with no glyph at all.
pub(crate) fn terminal_facing<S: AsRef<str>>(
    chain: &[S],
    symbol: Option<&str>,
    terminal: Option<&str>,
) -> Option<Side> {
    let glyph = part_glyph(chain, symbol)?;
    let index = match terminal {
        Some(t) => part_pin_ids(chain, symbol).iter().position(|p| *p == t)?,
        None => 0,
    };
    let port = *glyph.ports.get(index)?;
    facing(port, glyph.width, glyph.height)
}

/// The variant names a type offers, for the unknown-`symbol:` error.
pub(super) fn variant_names(ty: &str) -> Vec<&'static str> {
    variants(ty).iter().map(|v| v.name).collect()
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
/// `|opamp|`'s ports — it has no variants, so its pins state here.
const OPAMP_PINS: &[&str] = &["out", "inp", "inn"];

/// The variant a part wears: the one `symbol:` names, or the family default.
/// `None` only for an unknown name — the lowering turns that into the error;
/// [`part_pin_ids`] falls back to no pins.
fn variant(ty: &str, want: Option<&str>) -> Option<&'static Variant> {
    let set = variants(ty);
    match want {
        None => set.first(),
        Some(name) => set.iter().find(|v| v.name == name),
    }
}

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
            "nfet-circled" "sch-q-nfet-circled" &["g", "d", "s"],
            "pfet-circled" "sch-q-pfet-circled" &["g", "d", "s"],
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
pub(super) const LABEL_SYMBOLS: &[&str] = &["gnd", "earth", "chassis", "power", "nc", "antenna"];
