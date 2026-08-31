//! **The scopes a lowering child is nested in** [SPEC 15/16], and the four
//! readings that decide them: which container *opens* a scope, and which one
//! *seals* it against the enclosing one.
//!
//! Two scopes reach past the container that opens them, so each is carried
//! down the walk ([`Nest`]) rather than re-read per node; a scope is opened by
//! a container's own reading of desugar's cascade slice ([`is_drawing_body`],
//! [`is_schematic_body`]) and closed by the seal its kind answers to
//! ([`seals_drawing_scope`], [`seals_schematic_scope`]).

use super::Lower;
use crate::syntax::ast::{Decl, layout_of};

/// The **scopes a lowering child is nested in** — the two whose laws reach
/// past the container that opens them, so each one has to be carried down the
/// walk rather than re-read per node:
///
/// - `drawing` [SPEC 15]: the gate for the generated chrome. Opened by a
///   drawing node, carried through its parts and features, **sealed** by a
///   child that owns its own layout ([`seals_drawing_scope`]).
/// - `schematic` [SPEC 16]: the **link-law carrier**. A schematic scope's
///   reading of a statement — a one-ended wire is a label wire, a bare unknown
///   id is an error rather than a new box — reaches every statement written
///   inside it, nested ordinary containers (`|row|`, `|group|`, an anonymous
///   wrapper) included. Sealed only by another engine that reads its own
///   body's statements ([`seals_schematic_scope`]), and never confused with
///   **placement**, which does not cascade at all: seating and auto-posing are
///   the immediate container's ([`is_schematic_body`], the flag
///   [`super::gather::Scope`] poses by).
///
/// Both are read off desugar's cascade slice (instance style, element rules,
/// template bundles): a container made a schematic only by a descendant or
/// class rule is not seen here — the accepted stage-1 edge, and the reason the
/// resolve-side twin [`crate::layout::schematic::check_types`] — which walks
/// the resolved tree carrying the same flag — is the gate that decides whether
/// a part may exist at all.
#[derive(Clone, Copy)]
pub(crate) struct Nest {
    pub(super) drawing: bool,
    pub(super) schematic: bool,
}

impl Nest {
    /// No scope at all — what generated chrome lowers in.
    pub(super) const NONE: Nest = Nest {
        drawing: false,
        schematic: false,
    };
}

/// Whether this container is **itself** a drawing scope, detected as frames are
/// — by type chain (`|drawing|`, a `|floorplan|`, or a define over either:
/// the dialect's chain carries `drawing` [SPEC 8]) or an explicit drawing-family
/// `layout:` on the instance [SPEC 15/15.11].
pub(super) fn is_drawing_body(chain: &[String], style: &[Decl]) -> bool {
    chain.iter().any(|t| t == "drawing")
        || layout_of(style).is_some_and(crate::resolve::is_drawing_layout)
}

/// Whether this container opens a **datum scope** [SPEC 12] — a `|stack|`, or
/// any of the drawing family built on it. Desugar's twin of
/// `resolve::is_stack_layout`: the scale fold keys off this, since a stack
/// carries `unit:` / `density:` exactly as a drawing does.
pub(super) fn is_stack_body(chain: &[String], style: &[Decl]) -> bool {
    chain.iter().any(|t| t == "stack")
        || layout_of(style).is_some_and(crate::resolve::is_stack_layout)
        || is_drawing_body(chain, style)
}

/// Whether this container is **itself** a `|page|` [SPEC 15.8] — the sheet
/// whose furniture and density fold key off it. One reading for both stages:
/// an already-lowered node states its type as a `.lini-*` class, so the scale
/// fold passes its lowered chain where the main walk passes the authored one.
pub(super) fn is_page_body(chain: &[String]) -> bool {
    chain.iter().any(|t| t == "page")
}

/// Whether this container is **itself** a schematic [SPEC 16] — desugar's twin
/// of `layout::schematic::is_schematic`, and the only container-level reading
/// desugar makes: [`Nest::schematic`] is this answer plus the enclosing one, so
/// the reach is stated once, by the walk. One read of desugar's cascade slice
/// answers every form — `|schematic|` (whose template bundle sets the attr), an
/// explicit `layout: schematic` on any container, and a define that carries one
/// (`{ |sheet::group| { layout: schematic } }`).
///
/// The slice is instance style + element rules + template bundles, so a scope
/// declared **only** by a descendant or `.class` rule (`.sheet { layout:
/// schematic }`) is not seen here — resolve's cascade is wider than desugar's,
/// which is why `layout::schematic::check_types` and not this is the gate that
/// decides whether a part may exist. Such a file places correctly; only the
/// desugar-time readings (the label-wire mint, the invent refusal) miss it.
pub(super) fn is_schematic_body(cx: &Lower, chain: &[String], style: &[Decl]) -> bool {
    cx.chain_ident(chain, style, "layout").as_deref() == Some("schematic")
}

/// The engines that read their **own body's statements** [SPEC 12/13/14/15]: a
/// tree's links are branches, a sequence's are messages, a drawing's are
/// leaders / measures / mates, a chart's and a pie's body is its data.
///
/// The one list, two readers — one per stage that carries the schematic scope:
/// [`seals_schematic_scope`] here (desugar's cascade slice) and
/// `resolve::program::link_scope::statement_owner` (the resolved attrs).
pub(crate) const STATEMENT_ENGINES: &[&str] = &[
    "drawing",
    // The architectural dialect is the drawing engine [SPEC 15.11] — its links
    // are dimensions, leaders and mates just the same.
    "floorplan",
    "sequence",
    "tree",
    "chart",
    "pie",
];

/// Whether a node **seals** an enclosing drawing scope [SPEC 15.1]: it owns a
/// layout (a flow wrapper, a grid, an engine) and arranges its interior as
/// usual — its children are not the drawing's features. The layout-side twin
/// is `layout::owns_layout` (attr-based, post-cascade).
pub(super) fn seals_drawing_scope(chain: &[String], style: &[Decl]) -> bool {
    chain.iter().any(|t| {
        matches!(
            t.as_str(),
            "row"
                | "column"
                | "grid"
                | "table"
                | "entity"
                | "chart"
                | "pie"
                | "sequence"
                | "schematic"
                // A `|stack|` arranges its interior on its own datum
                // [SPEC 12], so it seals like any other layout-owning type —
                // its children are not the enclosing drawing's features. The
                // type has to be named here because a template bundle's
                // `layout:` is not an authored decl for the test below.
                | "stack"
        )
    }) || style
        .iter()
        .any(|d| d.name == "layout" || d.name == "direction")
}

/// Whether a node's **body** sits in a drawing scope [SPEC 15.1] — the law, in
/// one place, for every walk that carries the scope down: it holds if the node
/// opens a scope of its own, or inherits one it does not seal.
///
/// `opens` beats the seal, and must: the `layout: drawing` that opens the scope
/// is the very declaration [`seals_drawing_scope`] reads, so an `opens && seals`
/// node that cleared the flag would seal itself and leave its own features
/// outside the scope they belong to.
pub(super) fn in_drawing_scope(
    opens: bool,
    inherited: bool,
    chain: &[String],
    style: &[Decl],
) -> bool {
    opens || (inherited && !seals_drawing_scope(chain, style))
}

/// Whether a node **seals** an enclosing schematic scope [SPEC 16] — the twin
/// above, one grain narrower, and narrower for a reason worth stating.
///
/// The drawing scope is sealed by anything that owns a **layout**, because what
/// it carries is *placement*: a `|row|` inside a drawing arranges its interior
/// as usual, so its children are not the drawing's features. The schematic
/// scope carries no placement at all — it carries a **reading of statements**
/// (a one-ended wire is a net label, a bare unknown id is an error), and a
/// `|row|` reads no statement of its own, so the sheet's laws must reach right
/// through it. That reach is the carrier's whole point ([`Nest`]).
///
/// What does stop them is another engine that already owns that reading, and
/// only that: inside a nested `|drawing|` a leader (`r1 -> "a note"`,
/// [SPEC 15.7]) is a leader, not a minted tag, and inside a nested `|sequence|`
/// `x -> y "hi"` still declares its participants. Read through the same
/// cascade slice as [`is_schematic_body`], so a define carrying the engine
/// seals exactly as the built-in type does.
pub(super) fn seals_schematic_scope(cx: &Lower, chain: &[String], style: &[Decl]) -> bool {
    cx.chain_ident(chain, style, "layout")
        .is_some_and(|l| STATEMENT_ENGINES.contains(&l.as_str()))
        // A `|mindmap|` declares no layout of its own — [`tree::seat_mindmap`]
        // stamps `layout: tree` on its scope *after* this body lowers
        // [SPEC 8] — so the one engine that arrives late is sealed by its type.
        || chain.iter().any(|t| t == "mindmap")
}
