//! Grid layout (SPEC §5).
//!
//! `layout: grid` sizes from track lists: `columns` (required) and `rows`
//! (optional — implicit, auto-sized rows). A track is a fixed size, `auto`
//! (sized to its widest / tallest single-span child), or `repeat(N)` /
//! `repeat(N, size)`; the track count is the list length. Children flow
//! left-to-right, wrapping at the column count; `cell: c r` pins one (1-indexed)
//! and `span: c r` widens it. A cell fills its track only when it carries
//! `align`/`justify: stretch` (the table's shipped `|table box| { … }` rule) and
//! has no explicit size on that axis; otherwise it sits at natural size, centred.

use super::ir::{Bbox, GridRule, PlacedNode};
use super::primitives;
use super::values::as_pair;
use crate::error::Error;
use crate::resolve::{AttrMap, ResolvedValue, VarTable};
use crate::span::Span;

#[derive(Clone, Copy)]
enum Track {
    Fixed(f64),
    Auto,
}

/// Lay out a grid; returns the content bbox plus the rule segments a table
/// draws (frame + interior separators, span-aware). Non-table callers ignore
/// the rules.
pub fn lay_out_grid(
    children: &mut [PlacedNode],
    attrs: &AttrMap,
    vars: &VarTable,
    span: Span,
) -> Result<(Bbox, Vec<GridRule>), Error> {
    let (gap_y, gap_x) = primitives::gap(attrs, vars, span)?;

    let col_tracks = match attrs.get("columns") {
        Some(v) => parse_tracks(v, span)?,
        None => return Err(Error::at(span, "'layout: grid' requires 'columns'")),
    };
    let cols = col_tracks.len();
    if cols == 0 {
        return Err(Error::at(span, "'columns' needs at least one track"));
    }
    let row_tracks: Option<Vec<Track>> = match attrs.get("rows") {
        Some(v) => Some(parse_tracks(v, span)?),
        None => None,
    };

    // Place children: an explicit `cell` pins, the rest auto-flow.
    let mut grid = Occupancy::new(cols);
    let mut placements: Vec<Placement> = Vec::with_capacity(children.len());
    for (i, child) in children.iter().enumerate() {
        let (cs, rs) = read_span(&child.attrs, child.span)?;
        let (col, row) = match read_cell(&child.attrs, child.span)? {
            Some((c, r)) => (c - 1, r - 1),
            None => grid.next_open(cs, rs),
        };
        if col + cs > cols {
            return Err(Error::at(
                child.span,
                format!("cell: {} _ exceeds columns={}", col + 1, cols),
            ));
        }
        grid.occupy(row, col, cs, rs, placements.len());
        placements.push(Placement {
            child_index: i,
            col,
            row,
            colspan: cs,
            rowspan: rs,
        });
    }

    // A declared `rows` track list is a floor (SPEC §5/§20): it sizes the first
    // rows, and any overflow flows into implicit auto rows (CSS grid). Columns
    // are fixed — only a `cell:` past the column count errors (in the loop above).
    let declared = row_tracks.as_ref().map_or(0, Vec::len);
    let rows = grid.rows().max(declared).max(1);
    grid.ensure(rows);

    let mut row_tracks = row_tracks.unwrap_or_default();
    row_tracks.resize(rows, Track::Auto);
    let col_sizes = track_sizes(&col_tracks, &placements, children, Axis::Col);
    let row_sizes = track_sizes(&row_tracks, &placements, children, Axis::Row);

    let col_off = cumulative(&col_sizes, gap_x);
    let row_off = cumulative(&row_sizes, gap_y);
    let total_w = (col_off[cols] - gap_x).max(0.0);
    let total_h = (row_off[rows] - gap_y).max(0.0);

    for p in &placements {
        let (x0, x1) = (col_off[p.col], col_off[p.col + p.colspan] - gap_x);
        let (y0, y1) = (row_off[p.row], row_off[p.row + p.rowspan] - gap_y);
        let child = &mut children[p.child_index];

        // Cell-fill (SPEC §5/§8): the cell's own `align`/`justify: stretch` fills
        // its track, unless an explicit size pins that axis.
        let fill_w = stretch(&child.attrs, "justify") && child.attrs.get("width").is_none();
        let fill_h = stretch(&child.attrs, "align") && child.attrs.get("height").is_none();
        if fill_w || fill_h {
            let w = if fill_w { x1 - x0 } else { child.bbox.w() };
            let h = if fill_h { y1 - y0 } else { child.bbox.h() };
            child.bbox = Bbox::centered(w, h);
        }

        let cell_cx = (x0 + x1) / 2.0 - total_w / 2.0;
        let cell_cy = (y0 + y1) / 2.0 - total_h / 2.0;
        let off_x = (child.bbox.min_x + child.bbox.max_x) / 2.0;
        let off_y = (child.bbox.min_y + child.bbox.max_y) / 2.0;
        child.cx = cell_cx - off_x;
        child.cy = cell_cy - off_y;
    }

    let dividers = divider_segments(
        read_divider(attrs),
        &col_off,
        &row_off,
        (total_w, total_h),
        (gap_x, gap_y),
        &grid.owner,
    );
    Ok((Bbox::centered(total_w, total_h), dividers))
}

// ───────────────────────── Track lists ─────────────────────────

fn parse_tracks(value: &ResolvedValue, span: Span) -> Result<Vec<Track>, Error> {
    let mut out = Vec::new();
    match value {
        ResolvedValue::Tuple(items) => {
            for item in items {
                push_track(&mut out, item, span)?;
            }
        }
        single => push_track(&mut out, single, span)?,
    }
    Ok(out)
}

fn push_track(out: &mut Vec<Track>, v: &ResolvedValue, span: Span) -> Result<(), Error> {
    match v {
        ResolvedValue::Ident(s) if s == "auto" => out.push(Track::Auto),
        ResolvedValue::Call(c) if c.name == "repeat" => {
            let n = c
                .args
                .first()
                .and_then(ResolvedValue::as_number)
                .filter(|n| *n >= 1.0 && n.fract() == 0.0)
                .ok_or_else(|| Error::at(span, "repeat() needs a positive integer count"))?
                as usize;
            let size = c.args.get(1).and_then(ResolvedValue::as_number);
            for _ in 0..n {
                out.push(size.map_or(Track::Auto, Track::Fixed));
            }
        }
        other => match other.as_number() {
            Some(n) => out.push(Track::Fixed(n)),
            None => {
                return Err(Error::at(
                    span,
                    "a track is a size, 'auto', or repeat(N[, size])",
                ));
            }
        },
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Axis {
    Col,
    Row,
}

/// Fixed tracks take their size; auto tracks the max single-span child extent.
fn track_sizes(
    tracks: &[Track],
    placements: &[Placement],
    children: &[PlacedNode],
    axis: Axis,
) -> Vec<f64> {
    let mut sizes: Vec<f64> = tracks
        .iter()
        .map(|t| match t {
            Track::Fixed(n) => *n,
            Track::Auto => 0.0,
        })
        .collect();
    for p in placements {
        let (idx, span_n, extent) = match axis {
            Axis::Col => (p.col, p.colspan, children[p.child_index].bbox.w()),
            Axis::Row => (p.row, p.rowspan, children[p.child_index].bbox.h()),
        };
        if span_n == 1 && idx < sizes.len() && matches!(tracks[idx], Track::Auto) {
            sizes[idx] = sizes[idx].max(extent);
        }
    }
    sizes
}

fn cumulative(sizes: &[f64], gap: f64) -> Vec<f64> {
    let mut out = Vec::with_capacity(sizes.len() + 1);
    let mut acc = 0.0;
    out.push(acc);
    for s in sizes {
        acc += s + gap;
        out.push(acc);
    }
    out
}

fn stretch(attrs: &AttrMap, name: &str) -> bool {
    matches!(attrs.get(name), Some(ResolvedValue::Ident(s)) if s == "stretch")
}

// ───────────────────────── Placement / occupancy ─────────────────────────

struct Placement {
    child_index: usize,
    col: usize,
    row: usize,
    colspan: usize,
    rowspan: usize,
}

/// A growable column×row occupancy map; rows extend as children flow or pin
/// past the current bottom (implicit rows have no fixed count).
struct Occupancy {
    cols: usize,
    occ: Vec<Vec<bool>>,
    owner: Vec<Vec<Option<usize>>>,
}

impl Occupancy {
    fn new(cols: usize) -> Self {
        Self {
            cols,
            occ: Vec::new(),
            owner: Vec::new(),
        }
    }

    fn rows(&self) -> usize {
        self.occ.len()
    }

    fn ensure(&mut self, rows: usize) {
        while self.occ.len() < rows {
            self.occ.push(vec![false; self.cols]);
            self.owner.push(vec![None; self.cols]);
        }
    }

    fn is_free(&self, row: usize, col: usize, cs: usize, rs: usize) -> bool {
        if col + cs > self.cols {
            return false;
        }
        (0..rs).all(|dr| {
            (0..cs).all(|dc| {
                self.occ
                    .get(row + dr)
                    .and_then(|r| r.get(col + dc))
                    .copied()
                    .map(|filled| !filled)
                    .unwrap_or(true)
            })
        })
    }

    fn occupy(&mut self, row: usize, col: usize, cs: usize, rs: usize, who: usize) {
        self.ensure(row + rs);
        for dr in 0..rs {
            for dc in 0..cs {
                if col + dc < self.cols {
                    self.occ[row + dr][col + dc] = true;
                    self.owner[row + dr][col + dc] = Some(who);
                }
            }
        }
    }

    /// First free `(col, row)` for a `cs×rs` cell, scanning row by row and
    /// growing downward as needed.
    fn next_open(&mut self, cs: usize, rs: usize) -> (usize, usize) {
        let mut row = 0;
        loop {
            self.ensure(row + rs);
            for col in 0..=self.cols.saturating_sub(cs) {
                if self.is_free(row, col, cs, rs) {
                    return (col, row);
                }
            }
            row += 1;
        }
    }
}

fn read_cell(attrs: &AttrMap, span: Span) -> Result<Option<(usize, usize)>, Error> {
    match attrs.get("cell") {
        None => Ok(None),
        Some(v) => {
            let (c, r) = as_pair(v, span)?;
            Ok(Some((
                positive_int("cell column", c, span)?,
                positive_int("cell row", r, span)?,
            )))
        }
    }
}

fn read_span(attrs: &AttrMap, span: Span) -> Result<(usize, usize), Error> {
    match attrs.get("span") {
        None => Ok((1, 1)),
        // `span: N` is `N 1` (SPEC §5).
        Some(ResolvedValue::Number(n)) => Ok((positive_int("span", *n, span)?.max(1), 1)),
        Some(v) => {
            let (c, r) = as_pair(v, span)?;
            Ok((
                positive_int("span column", c, span)?.max(1),
                positive_int("span row", r, span)?.max(1),
            ))
        }
    }
}

fn positive_int(name: &str, n: f64, span: Span) -> Result<usize, Error> {
    if n < 1.0 || n.fract() != 0.0 {
        return Err(Error::at(
            span,
            format!("'{}' expects a positive integer, got {}", name, n),
        ));
    }
    Ok(n as usize)
}

// ───────────────────────── Dividers ─────────────────────────

/// Which interior separators a container draws (SPEC §5). The outer frame is the
/// container's own border, so dividers are **interior only** — never doubled.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Divider {
    None,
    All,
    Rows,
    Columns,
}

pub fn read_divider(attrs: &AttrMap) -> Divider {
    match attrs.get("divider") {
        Some(ResolvedValue::Ident(s)) => match s.as_str() {
            "all" => Divider::All,
            "rows" => Divider::Rows,
            "columns" => Divider::Columns,
            _ => Divider::None,
        },
        _ => Divider::None,
    }
}

/// A grid that insets its cells — a `|table|` (SPEC §8): its `padding` is the
/// per-cell text-to-divider inset, not outer box padding. A grid drawing
/// dividers is exactly that, so the divider is the signal.
pub(super) fn is_inset_grid(attrs: &AttrMap) -> bool {
    matches!(attrs.get("layout"), Some(ResolvedValue::Ident(s)) if s == "grid")
        && read_divider(attrs) != Divider::None
}

/// The interior separators of a grid, each run merged across the tracks where it
/// is a real boundary — so a spanning cell has no line crossing its interior.
/// Node-local coords (the grid is centred on the origin). No frame: the
/// container's own border supplies the outer edge, so the outer boundaries clamp
/// exactly to the content box and interior lines sit centred in the gap.
// The boundary scans run one index past the data to close a run at the final
// edge, so they can't iterate `owner` directly.
#[allow(clippy::needless_range_loop)]
fn divider_segments(
    divider: Divider,
    col_offsets: &[f64],
    row_offsets: &[f64],
    (total_w, total_h): (f64, f64),
    (gap_x, gap_y): (f64, f64),
    owner: &[Vec<Option<usize>>],
) -> Vec<GridRule> {
    // The cumulative offsets carry one entry past each axis's track count.
    let cols = col_offsets.len() - 1;
    let rows = row_offsets.len() - 1;
    // A boundary's coordinate: the outer edges clamp to the content box; an
    // interior boundary sits in the middle of the gap between its tracks.
    let x = |i: usize| match i {
        0 => -total_w / 2.0,
        i if i == cols => total_w / 2.0,
        i => col_offsets[i] - gap_x / 2.0 - total_w / 2.0,
    };
    let y = |j: usize| match j {
        0 => -total_h / 2.0,
        j if j == rows => total_h / 2.0,
        j => row_offsets[j] - gap_y / 2.0 - total_h / 2.0,
    };
    let mut segs: Vec<GridRule> = Vec::new();
    if matches!(divider, Divider::All | Divider::Columns) {
        for c in 1..cols {
            let mut start: Option<usize> = None;
            for r in 0..=rows {
                let real = r < rows && owner[r][c - 1] != owner[r][c];
                if real && start.is_none() {
                    start = Some(r);
                } else if !real && let Some(s) = start.take() {
                    segs.push((x(c), y(s), x(c), y(r)));
                }
            }
        }
    }
    if matches!(divider, Divider::All | Divider::Rows) {
        for r in 1..rows {
            let mut start: Option<usize> = None;
            for c in 0..=cols {
                let real = c < cols && owner[r - 1][c] != owner[r][c];
                if real && start.is_none() {
                    start = Some(c);
                } else if !real && let Some(s) = start.take() {
                    segs.push((x(s), y(r), x(c), y(r)));
                }
            }
        }
    }
    segs
}
