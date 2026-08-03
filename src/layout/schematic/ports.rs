//! What a placed part hands the router [SPEC 16.2/16.5]: the one obstacle its
//! whole anatomy folds into, and the **fixed ports** its terminals offer.
//!
//! Both answers come from [`terminal`] — the single connection-geometry
//! reader the seat pass already uses — computed **once per part**, so a pin's
//! landing ordinate is one `f64` every wire touching it shares (bit-exact, as
//! ROUTING.md's implicit fan demands).
//!
//! The **connection frame** is the obstacle-identity decision made geometry:
//! a pin is never a router obstacle of its own — its stub, name and number
//! fold into the component's rect [SPEC 16.2] — and the frame's side lines
//! pass through the terminals that land on them, so a fixed port sits exactly
//! on its rect's side (ROUTING.md Fixed ports) and the wire meets the ink.
//! Chrome outside the frame (the ref / value readouts) stays an obstacle
//! through the scene index's overflow, never through the frame.

use super::super::ir::{Bbox, PlacedNode};
use super::terminal::{Terminal, terminal};
use crate::desugar::pose::Side;
use crate::desugar::schematic::{PartNode, SchKind, sch_kind, terminal_ids};

/// A placed schematic part, as the router sees it — in the part's **own**
/// coordinates (the scene index shifts it). Addresses are the id an endpoint
/// names, `None` meaning the part's own path.
pub(crate) struct PartPorts {
    /// The connection frame: the part's one obstacle rect.
    pub frame: Bbox,
    /// The part's **terminals** [SPEC 16.4] — its pins, and a `|label|`'s own
    /// connection point. A terminal owns its connection geometry whether or
    /// not the part's drawing gives it a facing, so this list is what the
    /// `:side` gate asks, never [`PartPorts::ports`]: an `|L|` pin whose glyph
    /// ports tie, or a symbol-less `|label|`, is a terminal with no landing.
    /// A part's own path is **not** a terminal (a pinless landing resolves to
    /// a pin — SPEC 16.5's arity rule).
    pub terminals: Vec<Option<String>>,
    /// The fixed landings: the address, its forced side and its exact point.
    /// A superset of the terminals with a facing — a symbol part also lands a
    /// **bare** wire (`- r1`) on its first pin, which the seat pass reads the
    /// same way, so the two agree until Phase 5 resolves arity.
    pub ports: Vec<(Option<String>, crate::ast::Side, (f64, f64))>,
}

/// The router's view of `node`, or `None` when it is not a schematic part.
/// An **anonymous** part still folds (its chrome is never a separate
/// obstacle); it offers no terminals and no ports, because it has no dot-path
/// to name [SPEC 9].
pub(crate) fn part_ports(node: &PlacedNode) -> Option<PartPorts> {
    let kind = sch_kind(&node.type_chain)?;
    let addressable = node.id.is_some();
    let mut terms: Vec<Terminal> = Vec::new();
    let mut terminals = Vec::new();
    let mut ports = Vec::new();
    for id in terminal_ids(node) {
        let t = terminal(node, id.as_deref());
        // An **anonymous** pin shapes the frame like any other but is neither
        // terminal nor port: it has no address to name it by [SPEC 9].
        if let (true, Some(id)) = (addressable, id) {
            if let Some(side) = t.facing {
                ports.push((Some(id.clone()), side.into(), t.at));
            }
            terminals.push(Some(id));
        }
        terms.push(t);
    }
    // …and the bare form (`- gnd1`, `- r1`): a `|label|`'s one connection
    // point — its own terminal — or a symbol part's first pin, which is a
    // landing on a *part*, not a terminal of it.
    let bare = terminal(node, None);
    if addressable {
        if let Some(side) = bare.facing {
            ports.push((None, side.into(), bare.at));
        }
        if kind == SchKind::Label {
            terminals.push(None);
        }
    }
    terms.push(bare);
    Some(PartPorts {
        frame: frame(node.bbox, &terms),
        terminals,
        ports,
    })
}

/// The placed tree's adapter onto the one pin walk
/// ([`crate::desugar::schematic::terminal_ids`]) — the resolved tree wears the
/// twin of it where arity resolves a pinless landing [SPEC 16.5].
impl PartNode for PlacedNode {
    fn type_chain(&self) -> &[String] {
        &self.type_chain
    }
    fn attrs(&self) -> &crate::resolve::AttrMap {
        &self.attrs
    }
    fn node_id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn kids(&self) -> &[Self] {
        &self.children
    }
}

/// The connection frame: the part's box with every side that carries
/// terminals moved onto their landing line — outward over a component's pin
/// stubs, inward onto a symbol's glyph ports. Terminals on one side share
/// that line by construction (a rail's stubs are one length, a glyph's ports
/// on a side one coordinate); the outermost wins if a future glyph disagrees.
fn frame(mut box_: Bbox, terms: &[Terminal]) -> Bbox {
    for side in Side::ALL {
        let mut hits = terms
            .iter()
            .filter(|t| t.facing == Some(side))
            .map(|t| match side {
                Side::Left | Side::Right => t.at.0,
                Side::Top | Side::Bottom => t.at.1,
            });
        let Some(first) = hits.next() else { continue };
        let line = hits.fold(first, |a, b| match side {
            Side::Left | Side::Top => a.min(b),
            Side::Right | Side::Bottom => a.max(b),
        });
        let moved = match side {
            Side::Left => Bbox {
                min_x: line,
                ..box_
            },
            Side::Right => Bbox {
                max_x: line,
                ..box_
            },
            Side::Top => Bbox {
                min_y: line,
                ..box_
            },
            Side::Bottom => Bbox {
                max_y: line,
                ..box_
            },
        };
        if moved.min_x < moved.max_x && moved.min_y < moved.max_y {
            box_ = moved;
        }
    }
    box_
}
