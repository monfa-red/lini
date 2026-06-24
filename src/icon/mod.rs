//! Embedded Phosphor (duotone) icon set — geometry only, queried by name.
//!
//! Source: phosphor-icons/core v2.1.1, duotone weight, MIT (see `LICENSES/phosphor`).
//! The data lives in `assets/phosphor-duotone.txt` (sorted `name\t<role><fragment>…`);
//! regenerate it with `cargo xtask extract-icons <core>/raw/duotone`.
//!
//! Lookup builds the line index once (`OnceLock`) and binary-searches it; every
//! returned fragment borrows the embedded bytes — no allocation, no full parse.

/// How a stored geometry fragment is painted ([`super::render`] groups by this).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    /// Faint background body — painted with the icon's `fill`.
    Fill,
    /// Outline — stroked with the icon's `stroke`, no fill.
    Line,
    /// Solid foreground detail (a dot, a nucleus) — filled with the ink `stroke`.
    Solid,
    /// One shape that is both the body fill and the outline.
    Both,
}

/// Whether the icon set was compiled in (the `icons` feature).
pub const ENABLED: bool = cfg!(feature = "icons");

#[cfg(feature = "icons")]
const DATA: &str = include_str!("../../assets/phosphor-duotone.txt");
#[cfg(not(feature = "icons"))]
const DATA: &str = "";

/// The data lines (`name\t<role><fragment>…`), sorted by name; comment and blank
/// lines dropped. Built once, on first lookup.
fn lines() -> &'static [&'static str] {
    use std::sync::OnceLock;
    static LINES: OnceLock<Vec<&'static str>> = OnceLock::new();
    LINES.get_or_init(|| {
        DATA.lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect()
    })
}

fn name_of(line: &str) -> &str {
    line.split_once('\t').map_or(line, |(n, _)| n)
}

fn parse_field(field: &str) -> Option<(Role, &str)> {
    let (flag, fragment) = field.split_at_checked(1)?;
    let role = match flag {
        "F" => Role::Fill,
        "L" => Role::Line,
        "S" => Role::Solid,
        "B" => Role::Both,
        _ => return None,
    };
    Some((role, fragment))
}

/// The geometry fragments for `name`, or `None` if there is no such icon (or
/// icons are not compiled in). Each item is one shape's `(role, svg-fragment)`.
pub fn lookup(name: &str) -> Option<impl Iterator<Item = (Role, &'static str)>> {
    let lines = lines();
    let i = lines.binary_search_by(|l| name_of(l).cmp(name)).ok()?;
    let (_, rest) = lines[i].split_once('\t')?;
    Some(rest.split('\t').filter_map(parse_field))
}

/// Every known icon name, sorted — the basis for [`suggest`].
pub fn names() -> impl Iterator<Item = &'static str> {
    lines().iter().copied().map(name_of)
}

/// Up to three known names closest to `name` (edit distance ≤ 2), for a
/// "did you mean …?" hint on an unknown symbol.
pub fn suggest(name: &str) -> Vec<&'static str> {
    let mut near: Vec<(usize, &'static str)> = names()
        .map(|c| (edit_distance(c, name), c))
        .filter(|&(d, _)| d <= 2)
        .collect();
    near.sort_unstable();
    near.into_iter().take(3).map(|(_, c)| c).collect()
}

/// Levenshtein distance (two-row DP). Only ranks suggestions on the error path,
/// so it need not be fast.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        cur[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != cb);
            cur[j + 1] = sub.min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

#[cfg(all(test, feature = "icons"))]
mod tests {
    use super::*;

    #[test]
    fn known_icons_resolve() {
        // heart: a single Both path.
        let heart: Vec<_> = lookup("heart").unwrap().collect();
        assert_eq!(heart.len(), 1);
        assert_eq!(heart[0].0, Role::Both);
        assert!(heart[0].1.starts_with("<path"));
        // atom: carries a Solid nucleus.
        assert!(lookup("atom").unwrap().any(|(r, _)| r == Role::Solid));
        // user: a Both head + Line shoulders.
        let user: Vec<_> = lookup("user").unwrap().map(|(r, _)| r).collect();
        assert!(user.contains(&Role::Both) && user.contains(&Role::Line));
    }

    #[test]
    fn unknown_icon_is_none() {
        assert!(lookup("definitely-not-an-icon").is_none());
    }

    #[test]
    fn names_are_sorted_and_complete() {
        let all: Vec<_> = names().collect();
        assert_eq!(all.len(), 1512);
        assert!(all.windows(2).all(|w| w[0] < w[1]));
    }
}
