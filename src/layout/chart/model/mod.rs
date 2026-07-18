//! Parse a chart's resolved children into a typed model: the x (domain) axis, the
//! value axes, and the series bound to them [SPEC 14.2]. All chart-shape
//! validation [SPEC 20] lives here; the geometry is the renderers' job.

use super::palette;
use super::project::Dir;
use super::scale::{self, Scale};
use super::tooltip::{self, Tooltip};
use crate::error::Error;
use crate::expr::{self, Expr, FuncTable, Value as ExprValue};
use crate::ledger::format::{self, Format};
use crate::resolve::{AttrMap, MarkerKind, NodeKind, ResolvedInst, ResolvedValue};
use crate::span::Span;

mod annot;
mod axes;
mod build;
mod paint;
mod series;
mod types;

pub(crate) use build::build;
pub use types::*;

// Helpers reused by the sibling `pie.rs` (reached as `model::…`).
pub(crate) use build::{read_gap, tag};
pub(crate) use paint::{fill_color, fill_outline, label_of, live};

// Model-internal helpers shared across the submodules (each reaches them via
// `use super::*`).
use annot::{read_at, read_band, read_mark};
use axes::{axis_id, axis_spec, bind_axis, build_value_axes, build_x_axis, lookup_axis, read_side};
use paint::{clone_grid, muted, number, numbers, outline, paint_lists, real_color};
use series::{chart_marker, collect_strings, read_bubble, read_series, sample_formula};
