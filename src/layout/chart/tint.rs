//! Role tints [SPEC 14.6] — the `--name` live vars a chart's chrome wears
//! (`muted` labels, `grid` lines, `tip-bg`, a `-soft` palette fill), built through
//! the shared [`prim::live`].

use crate::resolve::ResolvedValue;

/// The muted role tint — a band tick / mark accent / axis label's default when unpainted.
pub(super) fn muted() -> ResolvedValue {
    ResolvedValue::live("muted")
}
