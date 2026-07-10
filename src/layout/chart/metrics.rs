//! Chart look metrics [SPEC 14] — the type scale and paint constants every
//! chart family shares, in one home.

/// The chart title.
pub(super) const TITLE_SIZE: f64 = 13.0;
/// An axis title — a step under the chart title.
pub(super) const AXIS_TITLE_SIZE: f64 = 11.0;
/// Tick labels, legend entries, band / mark labels.
pub(super) const LABEL_SIZE: f64 = 11.0;
/// An area / radar body's fill opacity, so gridlines and overlaps still read.
pub(super) const AREA_OPACITY: f64 = 0.82;
/// The tick count a "nice" step aims for (`range / TICK_TARGET`).
pub(super) const TICK_TARGET: f64 = 5.0;
