use crate::resolve::{AttrMap, Markers, NodeKind, ResolvedValue, SheetInputs, Strategy, VarTable};
use crate::span::Span;

pub struct LaidOut {
    pub viewbox: ViewBox,
    pub nodes: Vec<PlacedNode>,
    pub links: Vec<RoutedLink>,
    /// The routing report: drawn crossings (counted output) and the links it
    /// could not legally draw.
    pub link_report: Vec<crate::routing::Violation>,
    /// The impossible links made visible (ROUTING Impossible layouts) —
    /// carried beside the links, never as one, so the validator never sees
    /// them.
    pub strays: Vec<Stray>,
    /// Resolved CSS variables — carried through to render so the `<style>`
    /// block and `--bake-vars` mode can both read them.
    pub vars: VarTable,
    /// Defs-block stylesheet inputs [SPEC 17] — the renderer states these
    /// as class rules and diffs node attrs against them.
    pub sheet: SheetInputs,
    /// The root container's `fill:`, when set [SPEC 17]: render paints a
    /// backing rect over the whole viewBox. `None` ⇒ a transparent canvas.
    pub canvas_fill: Option<ResolvedValue>,
    /// Distinct gradients [SPEC 10.3], collected post-layout: paint use-sites are
    /// rewritten to `url(#lini-gradient-N)` and the definitions emitted into
    /// `<defs>`. Empty unless the scene paints with a gradient.
    pub gradients: Vec<GradientDef>,
}

/// A distinct gradient paint [SPEC 10.3]: a kind plus its colour stops, evenly
/// spaced. Twin of the drop-shadow filter — collected, deduplicated, and emitted
/// once into `<defs>`; the stops stay `ResolvedValue`s so they flip and bake.
#[derive(Clone)]
pub struct GradientDef {
    pub kind: GradientKind,
    pub stops: Vec<ResolvedValue>,
}

#[derive(Clone, Copy)]
pub enum GradientKind {
    /// Linear, at the given CSS-style angle in degrees (`gradient()` defaults 135).
    Linear(f64),
    Radial,
}

/// An impossible link's report made visible: one straight segment between its
/// two bodies, centre to centre and trimmed to their boundaries, at whatever
/// angle the geometry gives. It obeys no law, takes no port slot, and blocks
/// nothing — rendered in the themable `--lini-stray` style.
#[derive(Clone)]
pub struct Stray {
    pub from: (f64, f64),
    pub to: (f64, f64),
    pub data_from: String,
    pub data_to: String,
}

/// One routed link: its path polyline plus what render needs.
#[derive(Clone)]
pub struct RoutedLink {
    pub path: Vec<(f64, f64)>,
    /// The strategy that drew this wire. The independent law checker judges
    /// orthogonal wires only — a `straight` wire is lawfully oblique and
    /// avoids nothing (ROUTING Strategies).
    pub strategy: Strategy,
    pub markers: Markers,
    pub attrs: AttrMap,
    /// `.style` names applied to the link — rendered as `lini-style-*` classes,
    /// the same surface a node's styles get [SPEC 17]. Routing never reads it.
    pub applied_styles: Vec<String>,
    pub texts: Vec<RoutedText>,
    /// First and last endpoints of the chain this segment belongs to — surfaced
    /// as `data-from` / `data-to`.
    pub data_from: String,
    pub data_to: String,
    /// This segment's own endpoints (the nodes it may touch — used by the
    /// validator's attachment check).
    pub seg_from: String,
    pub seg_to: String,
    /// Span of the link declaration this segment came from; segments sharing it
    /// are siblings of one statement (a chain or a fan).
    pub decl_span: Span,
    /// Fan-trunk group ids, one per end (source, target). Two links sharing an
    /// id are fan siblings: their shared trunk is drawn as one line, so the
    /// validator exempts it from link–link separation.
    pub fan_from: Option<u32>,
    pub fan_to: Option<u32>,
}

#[derive(Clone)]
pub struct RoutedText {
    pub content: String,
    pub position: (f64, f64),
    /// Unit tangent at the text position (for future rotation / offset frames).
    pub tangent: (f64, f64),
    pub attrs: AttrMap,
    /// The CSS class the label wears — `lini-link-label` for a diagram link label
    /// (on the wire), `lini-sequence-message` for a sequence message label (a
    /// heading above the arrow). One shared rule per role, so the size states once
    /// and the two coexist in one file [SPEC 9/13].
    pub class: &'static str,
}

/// The default label class: a diagram link label riding on the wire.
pub const LINK_LABEL_CLASS: &str = "lini-link-label";
/// A sequence message label — a heading above the arrow.
pub const SEQUENCE_MESSAGE_CLASS: &str = "lini-sequence-message";

#[derive(Debug, Clone, Copy)]
pub struct ViewBox {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// One interior gutter rect `(cx, cy, w, h)` in node-local coords, centred on its
/// own centre — the gap region between cells, filled with the container's
/// `gap-color` [SPEC 11]. Rendered as a `<rect fill="…" stroke="none">`: a filled
/// rect (not a stroked line) both carries a gradient `gap-color` — a `<line>`'s
/// degenerate bbox can't — and states `stroke="none"` so the container's own
/// `stroke` never bleeds onto it.
pub type Gutter = (f64, f64, f64, f64);

#[derive(Clone)]
pub struct PlacedNode {
    pub id: Option<String>,
    pub kind: NodeKind,
    pub type_chain: Vec<String>,
    pub applied_styles: Vec<String>,
    pub label: Option<String>,
    pub attrs: AttrMap,
    /// A `Text` node's own `{ }` style [SPEC 3] — rendered as `style=` /
    /// `transform` on the `<text>`. Empty for boxes and unstyled text.
    pub own_style: AttrMap,
    pub markers: Markers,
    /// Local origin position in parent coords.
    pub cx: f64,
    pub cy: f64,
    /// Bbox in local coords (relative to this node's own origin) — the layout
    /// **footprint**: what siblings space against and the canvas includes.
    pub bbox: Bbox,
    pub rotation: f64,
    pub children: Vec<PlacedNode>,
    /// Interior gutter rects the container fills with its `gap-color` [SPEC 11] —
    /// the gap regions between children. Interior only: the outer frame is the
    /// container's own border. Empty unless `gap-color:` is set.
    pub gutters: Vec<Gutter>,
    /// Links this container drew itself, in its local frame — a sequence's
    /// messages, lowered through the `straight` strategy [SPEC 13]. Routing
    /// lifts them into scene coordinates; the renderer's one link path draws
    /// them. Empty everywhere else.
    pub links: Vec<RoutedLink>,
    /// A sketch's authored `:name` products [SPEC 15.2], in the node's local
    /// frame (scaled) — the drawing engine's mate / dimension anchors read
    /// them. Empty for everything but a `|sketch|`.
    pub names: Vec<(String, super::drawing::pen::Product)>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy)]
pub struct Bbox {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Bbox {
    pub fn empty() -> Self {
        Self {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 0.0,
            max_y: 0.0,
        }
    }

    pub fn centered(w: f64, h: f64) -> Self {
        Self {
            min_x: -w / 2.0,
            min_y: -h / 2.0,
            max_x: w / 2.0,
            max_y: h / 2.0,
        }
    }

    pub fn w(&self) -> f64 {
        self.max_x - self.min_x
    }

    pub fn h(&self) -> f64 {
        self.max_y - self.min_y
    }

    /// Inflate by `pad` on every side.
    pub fn inflate(self, pad: f64) -> Self {
        Self {
            min_x: self.min_x - pad,
            min_y: self.min_y - pad,
            max_x: self.max_x + pad,
            max_y: self.max_y + pad,
        }
    }

    /// Expand each side independently (signed): top/left grow the min edges
    /// outward, right/bottom grow the max edges. Negative values shrink. Used
    /// for a table's per-cell padding inset, which inflates each cell so the
    /// auto tracks size to content + inset.
    pub fn expand(self, top: f64, right: f64, bottom: f64, left: f64) -> Self {
        Self {
            min_x: self.min_x - left,
            min_y: self.min_y - top,
            max_x: self.max_x + right,
            max_y: self.max_y + bottom,
        }
    }

    /// Union with another bbox already expressed in this frame.
    pub fn union(self, other: Bbox) -> Self {
        Self {
            min_x: self.min_x.min(other.min_x),
            min_y: self.min_y.min(other.min_y),
            max_x: self.max_x.max(other.max_x),
            max_y: self.max_y.max(other.max_y),
        }
    }

    /// Shift this bbox by (dx, dy). Useful when composing child bboxes into a
    /// parent's frame.
    pub fn shifted(self, dx: f64, dy: f64) -> Self {
        Self {
            min_x: self.min_x + dx,
            min_y: self.min_y + dy,
            max_x: self.max_x + dx,
            max_y: self.max_y + dy,
        }
    }
}
