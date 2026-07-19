//! Role tints [SPEC 14.6] — the `--name` live vars a chart's chrome wears
//! (`muted` labels, `grid` lines, `tip-bg`, a `-soft` palette fill). One home so
//! `model/` and the sibling renderers share the pair rather than each re-typing it.

use crate::resolve::ResolvedValue;

/// A `--name` role reference the renderer resolves (so it themes / flips / bakes).
pub(super) fn live(name: &str) -> ResolvedValue {
    ResolvedValue::LiveVar {
        name: name.to_string(),
        raw: false,
    }
}

/// The muted role tint — a band tick / mark accent / axis label's default when unpainted.
pub(super) fn muted() -> ResolvedValue {
    live("muted")
}
