//! Repo automation — the `cargo xtask …` pattern (std-only, never published).
//! Run via the alias in `.cargo/config.toml`.
//!
//! Commands:
//!   extract-icons <dir>   Regenerate `assets/phosphor-duotone.txt` from a folder
//!                         of Phosphor *duotone* SVGs (e.g. `<core>/raw/duotone`).
//!
//! Future home for `embed-font` and similar asset tooling.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("extract-icons") => match args.get(1) {
            Some(dir) => extract_icons(dir),
            None => usage("extract-icons <dir-of-duotone-svgs>"),
        },
        _ => usage("<command>   (commands: extract-icons)"),
    }
}

fn usage(msg: &str) -> ExitCode {
    eprintln!("usage: cargo xtask {msg}");
    ExitCode::FAILURE
}

// ───────────────────────────── extract-icons ─────────────────────────────

/// The rendering recipe for one stored geometry fragment.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    /// Faint background body — painted with the icon's `fill`.
    Fill,
    /// Outline — stroked with the icon's `stroke` (no fill).
    Line,
    /// Solid foreground detail (a dot, a nucleus) — filled with the ink `stroke`.
    Solid,
    /// One geometry that is both the body fill and the outline.
    Both,
}

impl Role {
    fn flag(self) -> char {
        match self {
            Role::Fill => 'F',
            Role::Line => 'L',
            Role::Solid => 'S',
            Role::Both => 'B',
        }
    }
}

fn extract_icons(dir: &str) -> ExitCode {
    let mut paths: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "svg"))
            .collect(),
        Err(e) => {
            eprintln!("cannot read {dir}: {e}");
            return ExitCode::FAILURE;
        }
    };
    paths.sort();

    // BTreeMap keeps the output sorted by name, so binary search works at runtime.
    let mut icons: BTreeMap<String, Vec<(Role, String)>> = BTreeMap::new();
    for path in &paths {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let Some(name) = stem.strip_suffix("-duotone") else {
            continue;
        };
        let svg = fs::read_to_string(path).expect("read svg");
        icons.insert(name.to_owned(), fragments(&svg));
    }

    let mut out = format!(
        "# phosphor-icons/core v2.1.1 (2b75f3ad) — duotone — {} icons\n\
         # Geometry only, paint stripped. Role flag: F=fill body, L=line stroke,\n\
         # S=solid ink, B=both. Regenerate: cargo xtask extract-icons <core>/raw/duotone\n",
        icons.len()
    );
    for (name, frags) in &icons {
        out.push_str(name);
        for (role, frag) in frags {
            out.push('\t');
            out.push(role.flag());
            out.push_str(frag);
        }
        out.push('\n');
    }

    let dest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("assets/phosphor-duotone.txt");
    fs::write(&dest, out).expect("write data file");
    eprintln!("wrote {} icons → {}", icons.len(), dest.display());
    ExitCode::SUCCESS
}

/// Parse one duotone SVG into deduped `(role, cleaned-fragment)` geometry in
/// document order. The document is flat self-closing elements, plus an optional
/// `<g opacity="0.2">…</g>` wrapping the faint fill layer.
fn fragments(svg: &str) -> Vec<(Role, String)> {
    let mut raw: Vec<(Role, String)> = Vec::new();
    let mut in_fill_group = false;
    let mut rest = svg;
    while let Some(lt) = rest.find('<') {
        let after = &rest[lt..];
        let gt = after.find('>').expect("closing >");
        let el = &after[..=gt];
        rest = &after[gt + 1..];

        if el.starts_with("<svg") || el.starts_with("</svg") || el.starts_with("<?") {
            continue;
        }
        if el.starts_with("<g") {
            in_fill_group = el.contains(r#"opacity="0.2""#);
            continue;
        }
        if el.starts_with("</g") {
            in_fill_group = false;
            continue;
        }
        if is_bounding_rect(el) {
            continue;
        }

        let role = if in_fill_group || el.contains(r#"opacity="0.2""#) {
            Role::Fill
        } else if el.contains("stroke=") {
            Role::Line
        } else {
            Role::Solid
        };
        raw.push((role, clean(el)));
    }
    dedup(raw)
}

/// The transparent `<rect width="256" height="256" fill="none"/>` every Phosphor
/// SVG carries to fix its bounds — pure framing, dropped.
fn is_bounding_rect(el: &str) -> bool {
    el.starts_with("<rect")
        && el.contains(r#"width="256""#)
        && el.contains(r#"height="256""#)
        && el.contains(r#"fill="none""#)
}

/// Strip paint attributes, leaving geometry (and any `transform`) verbatim, so a
/// fill copy and a line copy of the same shape collapse to one fragment.
fn clean(el: &str) -> String {
    let mut s = el.to_owned();
    for name in [
        "fill",
        "stroke",
        "stroke-width",
        "stroke-linecap",
        "stroke-linejoin",
        "opacity",
    ] {
        let pat = format!(" {name}=\"");
        while let Some(at) = s.find(&pat) {
            let val_start = at + pat.len();
            let end = s[val_start..]
                .find('"')
                .map_or(s.len(), |q| val_start + q + 1);
            s.replace_range(at..end, "");
        }
    }
    s
}

/// Geometry shared by a fill copy and a line copy becomes `Both` (drawn once);
/// otherwise each unique fragment keeps its role, in first-seen order.
fn dedup(raw: Vec<(Role, String)>) -> Vec<(Role, String)> {
    let fills: HashSet<&str> = role_set(&raw, Role::Fill);
    let lines: HashSet<&str> = role_set(&raw, Role::Line);
    let mut seen: HashSet<&str> = HashSet::new();
    let mut out = Vec::new();
    for (role, frag) in &raw {
        if !seen.insert(frag.as_str()) {
            continue;
        }
        let role = if fills.contains(frag.as_str()) && lines.contains(frag.as_str()) {
            Role::Both
        } else {
            *role
        };
        out.push((role, frag.clone()));
    }
    out
}

fn role_set(raw: &[(Role, String)], want: Role) -> HashSet<&str> {
    raw.iter()
        .filter(|(r, _)| *r == want)
        .map(|(_, f)| f.as_str())
        .collect()
}
