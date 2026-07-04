//! Folding a cascade's ordered `(name, value)` declarations into an [`AttrMap`],
//! and extracting link/line markers from that ordered list (source order wins,
//! [SPEC 7]). Shared by node ([`super::scene`]) and link ([`super::links`])
//! resolution.

use super::ir::{AttrMap, MarkerKind, Markers, ResolvedValue};
use crate::error::Error;
use crate::span::Span;

/// Collapse an ordered declaration list into a map: later entries win, marker
/// attrs are dropped (they ride [`Markers`], not the map).
pub fn collapse(ordered: &[(String, ResolvedValue)]) -> AttrMap {
    let mut map = AttrMap::new();
    for (name, value) in ordered {
        if is_marker_attr(name) {
            continue;
        }
        map.insert(name.clone(), value.clone());
    }
    map
}

pub fn is_marker_attr(name: &str) -> bool {
    matches!(name, "marker" | "marker-start" | "marker-end")
}

/// Resolve the start/end marker from the ordered declarations, over the given
/// defaults. `marker:` sets both; `marker-start`/`-end` set one; source order
/// wins (`marker: arrow marker-end: dot` → start arrow, end dot).
pub fn resolve_markers(
    ordered: &[(String, ResolvedValue)],
    default_start: MarkerKind,
    default_end: MarkerKind,
    span: Span,
) -> Result<Markers, Error> {
    let mut start = default_start;
    let mut end = default_end;
    for (name, value) in ordered {
        match name.as_str() {
            "marker" => {
                let m = expect_marker(value, span)?;
                start = m;
                end = m;
            }
            "marker-start" => start = expect_marker(value, span)?,
            "marker-end" => end = expect_marker(value, span)?,
            _ => {}
        }
    }
    Ok(Markers { start, end })
}

fn expect_marker(value: &ResolvedValue, span: Span) -> Result<MarkerKind, Error> {
    match value {
        ResolvedValue::Ident(s) => MarkerKind::parse(s)
            .ok_or_else(|| Error::at(span, format!("invalid marker value '{}'", s))),
        _ => Err(Error::at(span, "marker requires an identifier value")),
    }
}
