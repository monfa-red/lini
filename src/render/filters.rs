//! Drop-shadow `<filter>` defs (SPEC §8). A closed shape's `shadow:` compiles
//! to one `<filter>`; identical specs share a filter, emitted once into
//! `<defs>` and referenced from the shape's geometry by id.
//!
//! The tint stays a `ResolvedValue` so `--bake-vars` inlines it like any other
//! paint — a `var(--lini-shadow)` in live mode, a literal in bake mode, the
//! same honesty the rest of the renderer keeps.

use super::values::{format_value, num};
use crate::Options;
use crate::layout::PlacedNode;
use crate::resolve::{ResolvedValue, VarTable};
use std::fmt::Write;

struct Shadow {
    dx: f64,
    dy: f64,
    blur: f64,
    color: ResolvedValue,
}

/// The default tint: `var(--lini-shadow)`.
fn default_tint() -> ResolvedValue {
    ResolvedValue::LiveVar {
        name: "shadow".into(),
        raw: false,
        baked: None,
    }
}

/// Parse a `shadow:` value (SPEC §8). Forms: `N` → offset (N, N) blur N ·
/// `(dx, dy)` · `(dx, dy, blur)` · `(dx, dy, blur, color)`. The tint defaults
/// to `--lini-shadow`. A malformed value yields `None` (drawn unshadowed).
fn parse(value: &ResolvedValue) -> Option<Shadow> {
    let n = |v: &ResolvedValue| v.as_number();
    match value {
        ResolvedValue::Number(s) => Some(Shadow {
            dx: *s,
            dy: *s,
            blur: *s,
            color: default_tint(),
        }),
        ResolvedValue::Tuple(items) => match items.as_slice() {
            [dx, dy] => Some(Shadow {
                dx: n(dx)?,
                dy: n(dy)?,
                blur: 0.0,
                color: default_tint(),
            }),
            [dx, dy, b] => Some(Shadow {
                dx: n(dx)?,
                dy: n(dy)?,
                blur: n(b)?,
                color: default_tint(),
            }),
            [dx, dy, b, c] => Some(Shadow {
                dx: n(dx)?,
                dy: n(dy)?,
                blur: n(b)?,
                color: c.clone(),
            }),
            _ => None,
        },
        _ => None,
    }
}

/// A stable key for deduping — two shapes with the same offset/blur/tint share
/// one filter. Built from the formatted parts, so it tracks `--bake-vars`.
fn key(s: &Shadow, vars: &VarTable, opts: &Options) -> String {
    format!(
        "{},{},{},{}",
        num(s.dx),
        num(s.dy),
        num(s.blur),
        format_value(&s.color, vars, opts)
    )
}

/// Every distinct drop shadow in a scene, in first-appearance order — the
/// order their `<filter>` ids are assigned, so output stays deterministic.
pub struct FilterTable {
    entries: Vec<(String, Shadow)>,
}

impl FilterTable {
    pub fn collect(nodes: &[PlacedNode], vars: &VarTable, opts: &Options) -> Self {
        let mut entries: Vec<(String, Shadow)> = Vec::new();
        collect_into(nodes, vars, opts, &mut entries);
        Self { entries }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The `url(#…)` filter id for a node's shadow, if it has one.
    pub fn id_for(&self, n: &PlacedNode, vars: &VarTable, opts: &Options) -> Option<String> {
        let shadow = parse(n.attrs.get("shadow")?)?;
        let k = key(&shadow, vars, opts);
        let i = self.entries.iter().position(|(ek, _)| *ek == k)?;
        Some(format!("lini-shadow-{}", i + 1))
    }

    /// Emit each filter once. A generous region keeps offset + blur from
    /// clipping. `feDropShadow` carries the offset, blur (`stdDeviation`), and
    /// tint in one primitive — resvg and browsers both render it.
    pub fn emit_defs(&self, out: &mut String, vars: &VarTable, opts: &Options) {
        for (i, (_, s)) in self.entries.iter().enumerate() {
            writeln!(
                out,
                r#"    <filter id="lini-shadow-{}" x="-50%" y="-50%" width="200%" height="200%"><feDropShadow dx="{}" dy="{}" stdDeviation="{}" flood-color="{}"/></filter>"#,
                i + 1,
                num(s.dx),
                num(s.dy),
                num(s.blur),
                format_value(&s.color, vars, opts),
            )
            .unwrap();
        }
    }
}

fn collect_into(
    nodes: &[PlacedNode],
    vars: &VarTable,
    opts: &Options,
    entries: &mut Vec<(String, Shadow)>,
) {
    for n in nodes {
        if let Some(v) = n.attrs.get("shadow")
            && let Some(shadow) = parse(v)
        {
            let k = key(&shadow, vars, opts);
            if !entries.iter().any(|(ek, _)| *ek == k) {
                entries.push((k, shadow));
            }
        }
        collect_into(&n.children, vars, opts, entries);
    }
}
