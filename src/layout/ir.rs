use crate::resolve::{AttrMap, Markers, ResolvedValue, ShapeKind, SheetInputs, VarTable};
use crate::span::Span;

pub struct LaidOut {
    pub viewbox: ViewBox,
    pub nodes: Vec<PlacedNode>,
    pub wires: Vec<RoutedWire>,
    /// The router's report: kept crossings (counted output) and the wires it
    /// could not legally draw.
    pub wire_report: Vec<super::wires::Violation>,
    /// The impossible wires made visible (WIRING §Impossible layouts) —
    /// carried beside the wires, never as one, so the validator never sees
    /// them.
    pub airwires: Vec<Airwire>,
    /// Resolved CSS variables — carried through to render so the `<style>`
    /// block and `--bake-vars` mode can both read them.
    pub vars: VarTable,
    /// Defs-block stylesheet inputs (SPEC §14) — the renderer states these
    /// as class rules and diffs node attrs against them.
    pub sheet: SheetInputs,
    /// The root container's `fill:`, when set (SPEC §13): render paints a
    /// backing rect over the whole viewBox. `None` ⇒ a transparent canvas.
    pub canvas_fill: Option<ResolvedValue>,
    /// Distinct gradients (SPEC §11.3), collected post-layout: paint use-sites are
    /// rewritten to `url(#lini-gradient-N)` and the definitions emitted into
    /// `<defs>`. Empty unless the scene paints with a gradient.
    pub gradients: Vec<GradientDef>,
}

/// A distinct gradient paint (SPEC §11.3): a kind plus its colour stops, evenly
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

/// An impossible wire's report made visible: one straight segment between its
/// two bodies, centre to centre and trimmed to their boundaries, at whatever
/// angle the geometry gives. It obeys no law, takes no port slot, and blocks
/// nothing — rendered in the themable `--lini-airwire` style.
#[derive(Clone)]
pub struct Airwire {
    pub from: (f64, f64),
    pub to: (f64, f64),
    pub data_from: String,
    pub data_to: String,
}

/// One routed wire: its orthogonal path polyline plus what render needs.
#[derive(Clone)]
pub struct RoutedWire {
    pub path: Vec<(f64, f64)>,
    pub markers: Markers,
    pub attrs: AttrMap,
    /// `.style` names applied to the wire — rendered as `lini-style-*` classes,
    /// the same surface a node's styles get (SPEC §14). Routing never reads it.
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
    /// Span of the wire declaration this segment came from; segments sharing it
    /// are siblings of one statement (a chain or a fan).
    pub decl_span: Span,
    /// Fan-trunk group ids, one per end (source, target). Two wires sharing an
    /// id are fan siblings: their shared trunk is drawn as one line, so the
    /// validator exempts it from wire–wire separation.
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
}

#[derive(Debug, Clone, Copy)]
pub struct ViewBox {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// One straight divider segment `(x1, y1, x2, y2)` in node-local coords.
pub type GridRule = (f64, f64, f64, f64);

#[derive(Clone)]
pub struct PlacedNode {
    pub id: Option<String>,
    pub shape: ShapeKind,
    pub type_chain: Vec<String>,
    pub applied_styles: Vec<String>,
    pub label: Option<String>,
    pub attrs: AttrMap,
    pub markers: Markers,
    /// Local origin position in parent coords.
    pub cx: f64,
    pub cy: f64,
    /// Bbox in local coords (relative to this node's own origin) — the layout
    /// **footprint**: what siblings space against and the canvas includes.
    pub bbox: Bbox,
    pub rotation: f64,
    pub children: Vec<PlacedNode>,
    /// Interior divider segments the container draws (SPEC §5), painted by its
    /// own `stroke*`. The outer frame is the container's border, so dividers
    /// never double it. Empty unless `divider:` is set.
    pub dividers: Vec<GridRule>,
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
