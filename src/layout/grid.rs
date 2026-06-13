//! Grid layout.
//!
//! Container declares grid mode via `layout:(cols, rows)`. Children place
//! themselves with `cell:(c, r)` and span tracks with `span:(c, r)`. Both
//! tuples use (horizontal, vertical) = (x, y) = (col, row) order.

use super::ir::{Bbox, PlacedNode};
use super::primitives;
use super::values::{as_number_tuple, as_pair};
use crate::error::Error;
use crate::resolve::{AttrMap, ResolvedValue, VarTable};
use crate::span::Span;

pub fn lay_out_grid(
    children: &mut [PlacedNode],
    cols: usize,
    rows: usize,
    attrs: &AttrMap,
    vars: &VarTable,
    span: Span,
) -> Result<Bbox, Error> {
    let (gap_y, gap_x) = primitives::gap(attrs, vars, span)?;

    // Track sizes: explicit col-widths / row-heights, else auto from children.
    let explicit_col = read_track_sizes(attrs, "col-widths", cols, span)?;
    let explicit_row = read_track_sizes(attrs, "row-heights", rows, span)?;

    // Assign positions: build a 2D occupancy map.
    let mut placements: Vec<Placement> = Vec::with_capacity(children.len());
    let mut occupied = vec![vec![false; cols]; rows];

    for (i, child) in children.iter().enumerate() {
        let (cs, rs) = read_span(&child.attrs, child.span)?;
        let (explicit_col_idx, explicit_row_idx) = read_cell(&child.attrs, child.span)?;

        let (col, row) = match (explicit_col_idx, explicit_row_idx) {
            (Some(c), Some(r)) => (c.saturating_sub(1), r.saturating_sub(1)),
            (Some(c), None) => {
                let c = c.saturating_sub(1);
                let r = find_row_for(c, cs, &occupied, rows);
                (c, r)
            }
            (None, Some(r)) => {
                let r = r.saturating_sub(1);
                let c = find_col_for(r, cs, &occupied, cols);
                (c, r)
            }
            (None, None) => next_open(cs, rs, &occupied, cols, rows).unwrap_or((0, 0)),
        };

        if col + cs > cols || row + rs > rows {
            return Err(Error::at(
                child.span,
                format!(
                    "cell=({}, {}) with span=({}, {}) exceeds grid layout=({}, {})",
                    col + 1,
                    row + 1,
                    cs,
                    rs,
                    cols,
                    rows
                ),
            ));
        }

        for dr in 0..rs {
            for dc in 0..cs {
                occupied[row + dr][col + dc] = true;
            }
        }
        placements.push(Placement {
            child_index: i,
            col,
            row,
            colspan: cs,
            rowspan: rs,
        });
    }

    // Compute auto-sized tracks (max child size per track, considering spans
    // only when they distribute evenly).
    let mut col_widths = explicit_col.clone().unwrap_or_else(|| vec![0.0_f64; cols]);
    let mut row_heights = explicit_row.clone().unwrap_or_else(|| vec![0.0_f64; rows]);
    if explicit_col.is_none() {
        for p in &placements {
            if p.colspan == 1 {
                col_widths[p.col] = col_widths[p.col].max(children[p.child_index].bbox.w());
            }
        }
    }
    if explicit_row.is_none() {
        for p in &placements {
            if p.rowspan == 1 {
                row_heights[p.row] = row_heights[p.row].max(children[p.child_index].bbox.h());
            }
        }
    }

    // Cumulative offsets per track.
    let col_offsets = cumulative(&col_widths, gap_x);
    let row_offsets = cumulative(&row_heights, gap_y);

    let total_w = col_offsets[cols] - gap_x;
    let total_h = row_offsets[rows] - gap_y;

    // Place each child centered in its (possibly spanning) cell.
    for p in &placements {
        let cell_x_start = col_offsets[p.col];
        let cell_y_start = row_offsets[p.row];
        let cell_x_end = col_offsets[p.col + p.colspan] - gap_x;
        let cell_y_end = row_offsets[p.row + p.rowspan] - gap_y;
        let cell_cx = (cell_x_start + cell_x_end) / 2.0 - total_w / 2.0;
        let cell_cy = (cell_y_start + cell_y_end) / 2.0 - total_h / 2.0;

        let child = &mut children[p.child_index];
        let local_offset_x = (child.bbox.min_x + child.bbox.max_x) / 2.0;
        let local_offset_y = (child.bbox.min_y + child.bbox.max_y) / 2.0;
        child.cx = cell_cx - local_offset_x;
        child.cy = cell_cy - local_offset_y;
    }

    Ok(Bbox::centered(total_w, total_h))
}

struct Placement {
    child_index: usize,
    col: usize,
    row: usize,
    colspan: usize,
    rowspan: usize,
}

fn read_track_sizes(
    attrs: &AttrMap,
    name: &str,
    track_count: usize,
    span: Span,
) -> Result<Option<Vec<f64>>, Error> {
    match attrs.get(name) {
        Some(ResolvedValue::Number(n)) => Ok(Some(vec![*n; track_count])),
        Some(ResolvedValue::List(items)) => {
            if items.len() != track_count {
                return Err(Error::at(
                    span,
                    format!(
                        "'{}' has {} values but {}={}",
                        name,
                        items.len(),
                        if name == "col-widths" { "cols" } else { "rows" },
                        track_count
                    ),
                ));
            }
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(super::values::as_number(item, span)?);
            }
            Ok(Some(out))
        }
        Some(other) => {
            // Allow tuple form too.
            let nums = as_number_tuple(other, span)?;
            if nums.len() != track_count {
                return Err(Error::at(
                    span,
                    format!(
                        "'{}' has {} values but {}={}",
                        name,
                        nums.len(),
                        if name == "col-widths" { "cols" } else { "rows" },
                        track_count
                    ),
                ));
            }
            Ok(Some(nums))
        }
        None => Ok(None),
    }
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

fn find_row_for(col: usize, cs: usize, occupied: &[Vec<bool>], _rows: usize) -> usize {
    for (r, row) in occupied.iter().enumerate() {
        if (0..cs).all(|dc| col + dc < row.len() && !row[col + dc]) {
            return r;
        }
    }
    0
}

fn find_col_for(row: usize, cs: usize, occupied: &[Vec<bool>], cols: usize) -> usize {
    for c in 0..cols.saturating_sub(cs.saturating_sub(1)) {
        if (0..cs).all(|dc| !occupied[row][c + dc]) {
            return c;
        }
    }
    0
}

fn next_open(
    cs: usize,
    rs: usize,
    occupied: &[Vec<bool>],
    cols: usize,
    rows: usize,
) -> Option<(usize, usize)> {
    for r in 0..rows.saturating_sub(rs.saturating_sub(1)) {
        for c in 0..cols.saturating_sub(cs.saturating_sub(1)) {
            let free = (0..rs).all(|dr| (0..cs).all(|dc| !occupied[r + dr][c + dc]));
            if free {
                return Some((c, r));
            }
        }
    }
    None
}

/// Read `cell=(c, r)` on a child — returns the 1-indexed grid position (or
/// `(None, None)` if absent so the caller can auto-flow). One axis may be
/// omitted by leaving the other unset; the engine picks the missing axis.
fn read_cell(attrs: &AttrMap, span: Span) -> Result<(Option<usize>, Option<usize>), Error> {
    match attrs.get("cell") {
        None => Ok((None, None)),
        Some(v) => {
            let (c, r) = as_pair(v, span)?;
            check_positive_int("cell.col", c, span)?;
            check_positive_int("cell.row", r, span)?;
            Ok((Some(c as usize), Some(r as usize)))
        }
    }
}

/// Read `span=(c, r)` on a child — defaults to (1, 1) if absent.
fn read_span(attrs: &AttrMap, span: Span) -> Result<(usize, usize), Error> {
    match attrs.get("span") {
        None => Ok((1, 1)),
        Some(v) => {
            let (c, r) = as_pair(v, span)?;
            check_positive_int("span.col", c, span)?;
            check_positive_int("span.row", r, span)?;
            Ok(((c as usize).max(1), (r as usize).max(1)))
        }
    }
}

fn check_positive_int(name: &str, n: f64, span: Span) -> Result<(), Error> {
    if n < 1.0 || n.fract() != 0.0 {
        return Err(Error::at(
            span,
            format!("'{}' expects a positive integer, got {}", name, n),
        ));
    }
    Ok(())
}
