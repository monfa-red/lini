//! Grid layout [SPEC 12].
//!
//! `layout: grid` sizes from track lists: `columns` (required) and `rows`
//! (optional — implicit, auto-sized rows). A track is a fixed size, `auto`
//! (sized to its widest / tallest single-span child), or `repeat(N)` /
//! `repeat(N, size)`; the track count is the list length. Children flow
//! left-to-right, wrapping at the column count; `cell: c r` pins one (1-indexed)
//! and `span: c r` widens it. `align` (↔) / `justify` (↕) accept a per-column list
//! (parallel to `columns`) or a scalar and place each cell's box in its track
//! (`stretch` fills, else pack start/center/end, default centre); a **filled**
//! cell then honours its *own* `align`/`justify` to place its text [SPEC 12].
//! `gap-color` fills the interior gutters between cells.

use super::ir::{Bbox, Gutter, PlacedNode};
use super::primitives;
use super::values::as_pair;
use crate::error::Error;
use crate::resolve::{AttrMap, NodeKind, ResolvedValue};
use crate::span::Span;

#[derive(Clone, Copy)]
enum Track {
    Fixed(f64),
    Auto,
}

/// Lay out a grid; returns the content bbox plus the interior gutter rects the
/// container fills with its `gap-color` (span-aware, per-axis). Empty when
/// `gap-color` is unset.
pub fn lay_out_grid(
    children: &mut [PlacedNode],
    attrs: &AttrMap,
    span: Span,
) -> Result<(Bbox, Vec<Gutter>), Error> {
    let (gap_y, gap_x) = primitives::gap(attrs, span)?;

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

    // A declared `rows` track list is a floor [SPEC 12/18]: it sizes the first
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

        // The container's per-column box alignment [SPEC 12]: a scalar applies to
        // every column, a tuple is one value per column (parallel to `columns`). On
        // a grid `align` is the horizontal (↔) axis and `justify` the vertical (↕) —
        // matching column-flow, not CSS grid. `stretch` fills the track,
        // `start`/`center`/`end` pack the box, and the default centres.
        let col_h = track_align(attrs, "align", p.col);
        let col_v = track_align(attrs, "justify", p.col);

        let child = &mut children[p.child_index];
        let is_box = child.kind != NodeKind::Text;

        // A box cell fills its track when the column — or the cell itself — is
        // `stretch` on that axis and no explicit size pins it. Text can't stretch.
        let fill_w = is_box
            && (col_h == Some("stretch") || stretch(&child.attrs, "align"))
            && child.attrs.get("width").is_none();
        let fill_h = is_box
            && (col_v == Some("stretch") || stretch(&child.attrs, "justify"))
            && child.attrs.get("height").is_none();
        if fill_w || fill_h {
            let w = if fill_w { x1 - x0 } else { child.bbox.w() };
            let h = if fill_h { y1 - y0 } else { child.bbox.h() };
            child.bbox = Bbox::centered(w, h);
        }

        // Pack the box in its track per the column alignment (a filled box centres —
        // its size equals the track).
        let (cw, ch) = (child.bbox.w(), child.bbox.h());
        let cell_cx = pack(col_h, x0, x1, cw) - total_w / 2.0;
        let cell_cy = pack(col_v, y0, y1, ch) - total_h / 2.0;
        let off_x = (child.bbox.min_x + child.bbox.max_x) / 2.0;
        let off_y = (child.bbox.min_y + child.bbox.max_y) / 2.0;
        child.cx = cell_cx - off_x;
        child.cy = cell_cy - off_y;

        // A filled box was sized *after* it laid out its text, so the text sits
        // centred; complete the cell's own `align` (↔) / `justify` (↕) now that its
        // final size is known [SPEC 12]. Generic — a plain grid never triggers it
        // (only a stretched cell has the slack), and the core needs no "table" notion.
        if fill_w || fill_h {
            align_cell_content(child, span)?;
        }
    }

    let gutters = if has_gap_color(attrs) {
        interior_gutters(
            &col_off,
            &row_off,
            (total_w, total_h),
            (gap_x, gap_y),
            &grid.owner,
        )
    } else {
        Vec::new()
    };
    Ok((Bbox::centered(total_w, total_h), gutters))
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

/// A per-column alignment keyword for column `c` [SPEC 12]: a scalar `Ident`
/// applies to every column; a `Tuple` is one value per column, parallel to
/// `columns` (like [`parse_tracks`]). `None` when unset or out of range.
fn track_align<'a>(attrs: &'a AttrMap, name: &str, c: usize) -> Option<&'a str> {
    match attrs.get(name)? {
        ResolvedValue::Ident(s) => Some(s.as_str()),
        ResolvedValue::Tuple(items) => match items.get(c)? {
            ResolvedValue::Ident(s) => Some(s.as_str()),
            _ => None,
        },
        _ => None,
    }
}

/// The centre of a `size`-wide box packed in the track `[lo, hi]` per an alignment
/// keyword: `start`/`end` push it to an edge, everything else (incl. `stretch`,
/// already filled to the track) centres.
fn pack(align: Option<&str>, lo: f64, hi: f64, size: f64) -> f64 {
    match align {
        Some("start") => lo + size / 2.0,
        Some("end") => hi - size / 2.0,
        _ => (lo + hi) / 2.0,
    }
}

/// Complete a filled cell's own content alignment [SPEC 12]. The grid sizes a cell
/// *after* the cell has laid out its text, so the text sits centred at the cell's
/// natural size; now that the final size is known, slide its single text leaf to
/// the cell's `align` (↔) / `justify` (↕) edge within the padded content box. Only
/// a box wrapping one text node — a `|block|` body cell or a `|header|` — has a leaf
/// to move; anything else, or a centred cell, is left as-is. The leaf keeps its
/// centred bbox, so `text-anchor: middle` still renders it flush.
fn align_cell_content(cell: &mut PlacedNode, span: Span) -> Result<(), Error> {
    let (h, v) = (
        ident(cell.attrs.get("align")),
        ident(cell.attrs.get("justify")),
    );
    if edge(h).is_none() && edge(v).is_none() {
        return Ok(());
    }
    let pad = primitives::padding(&cell.attrs, span)?;
    let (bw, bh) = (cell.bbox.w(), cell.bbox.h());
    let [leaf] = cell.children.as_mut_slice() else {
        return Ok(());
    };
    if leaf.kind != NodeKind::Text {
        return Ok(());
    }
    // The leaf's centre when flush to the near / far content edge on each axis.
    let (lw, lh) = (leaf.bbox.w() / 2.0, leaf.bbox.h() / 2.0);
    if let Some(dir) = edge(h) {
        leaf.cx = flush(dir, -bw / 2.0 + pad.left + lw, bw / 2.0 - pad.right - lw);
    }
    if let Some(dir) = edge(v) {
        leaf.cy = flush(dir, -bh / 2.0 + pad.top + lh, bh / 2.0 - pad.bottom - lh);
    }
    Ok(())
}

/// `Some(false)` for `start`, `Some(true)` for `end`, `None` for anything else
/// (`center` / unset / `stretch` — the leaf keeps its centred position).
fn edge(align: Option<&str>) -> Option<bool> {
    match align {
        Some("start") => Some(false),
        Some("end") => Some(true),
        _ => None,
    }
}

/// The near edge (`false`) or far edge (`true`).
fn flush(far: bool, near_center: f64, far_center: f64) -> f64 {
    if far { far_center } else { near_center }
}

fn ident(v: Option<&ResolvedValue>) -> Option<&str> {
    match v {
        Some(ResolvedValue::Ident(s)) => Some(s.as_str()),
        _ => None,
    }
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
        // `span: N` is `N 1` [SPEC 12].
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

// ───────────────────────── Gutters ─────────────────────────

/// Whether a container paints its gutters — `gap-color` set to a real colour
/// (not `none`, the default). Layout emits the gutter rects when this holds;
/// render resolves the colour itself.
pub(super) fn has_gap_color(attrs: &AttrMap) -> bool {
    match attrs.get("gap-color") {
        None => false,
        Some(ResolvedValue::Ident(s)) => s != "none",
        Some(_) => true,
    }
}

/// The interior gutters of a grid — the gap regions between cells, each run
/// merged across the tracks where its boundary is real (owner-diff), so a
/// spanning cell has no gutter crossing its interior. A vertical gutter (a column
/// boundary) is `gap_x` wide, painted only when the column gap is positive; a
/// horizontal one (a row boundary) is `gap_y` tall, painted only when the row gap
/// is positive ([SPEC 12]: `gap: 1 0` → row rules, `gap: 0 1` → column rules).
/// Node-local, centred coords; each gutter is `(cx, cy, w, h)`. Interior only —
/// the container's own border supplies the outer edge.
// The boundary scans run one index past the data to close a run at the final
// edge, so they can't iterate `owner` directly.
#[allow(clippy::needless_range_loop)]
fn interior_gutters(
    col_offsets: &[f64],
    row_offsets: &[f64],
    (total_w, total_h): (f64, f64),
    (gap_x, gap_y): (f64, f64),
    owner: &[Vec<Option<usize>>],
) -> Vec<Gutter> {
    // The cumulative offsets carry one entry past each axis's track count.
    let cols = col_offsets.len() - 1;
    let rows = row_offsets.len() - 1;
    // A boundary's coordinate: the outer edges clamp to the content box; an
    // interior boundary sits in the middle of the gap between its tracks (so a
    // gutter of that gap's width fills exactly the gap region).
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
    let mut out: Vec<Gutter> = Vec::new();
    // Vertical gutters (column boundaries), only when the column gap paints: a
    // `gap_x`-wide rect at the boundary, spanning its run of rows.
    if gap_x > 0.0 {
        for c in 1..cols {
            let mut start: Option<usize> = None;
            for r in 0..=rows {
                let real = r < rows && owner[r][c - 1] != owner[r][c];
                if real && start.is_none() {
                    start = Some(r);
                } else if !real && let Some(s) = start.take() {
                    let (y0, y1) = (y(s), y(r));
                    out.push((x(c), (y0 + y1) / 2.0, gap_x, y1 - y0));
                }
            }
        }
    }
    // Horizontal gutters (row boundaries), only when the row gap paints: a
    // `gap_y`-tall rect at the boundary, spanning its run of columns.
    if gap_y > 0.0 {
        for r in 1..rows {
            let mut start: Option<usize> = None;
            for c in 0..=cols {
                let real = c < cols && owner[r - 1][c] != owner[r][c];
                if real && start.is_none() {
                    start = Some(c);
                } else if !real && let Some(s) = start.take() {
                    let (x0, x1) = (x(s), x(c));
                    out.push(((x0 + x1) / 2.0, y(r), x1 - x0, gap_y));
                }
            }
        }
    }
    out
}
