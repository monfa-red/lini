//! Relative + absolute positioning for children inside a parent's bbox.
//!
//! A child leaves the flow when it carries either `at:(x, y)` — explicit
//! parent-local coords — or `side:` — an edge anchor. For an edge anchor:
//! `side` picks the edge, `align` slides along it (start / center / end),
//! `place` puts the child inside or outside that edge — **size-aware**: the
//! child clears the edge by its own extent, so it lands flush at any size —
//! and `offset:(x, y)` nudges. Corners fall out of `side` + `align`
//! (`side:top align:end` = top-right); there are no compound anchor names.

use super::ir::Bbox;
use super::values::as_pair;
use crate::error::Error;
use crate::resolve::{AttrMap, ResolvedValue};
use crate::span::Span;

#[derive(Clone, Copy)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Clone, Copy)]
pub enum Align {
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Place {
    /// Flush inside the edge — reserves a band (content shifts to clear it).
    In,
    /// Flush outside the edge — reserves a band outside (the parent gap).
    Out,
    /// Centred on the edge/corner — no reserve, an absolute overlay (like
    /// `at:(x,y)`); it doesn't grow the parent.
    On,
}

/// How a non-flow child is positioned in its parent's local frame.
#[derive(Clone, Copy)]
pub enum Pos {
    /// `at:(x, y)` — bbox center at explicit parent-local coords.
    Coord(f64, f64),
    /// `side:` — anchored to an edge, slid by `align`, inside/outside by `place`.
    Edge {
        side: Side,
        align: Align,
        place: Place,
    },
}

impl Side {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "top" => Self::Top,
            "bottom" => Self::Bottom,
            "left" => Self::Left,
            "right" => Self::Right,
            _ => return None,
        })
    }
}

impl Align {
    /// `start`/`center`/`end`, with `left`/`right` accepted as the horizontal
    /// synonyms (`left` = start, `right` = end) since `align` is also the text
    /// attr. Defaults to `center`.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "start" | "left" => Self::Start,
            "center" => Self::Center,
            "end" | "right" => Self::End,
            _ => return None,
        })
    }
}

impl Place {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "in" => Self::In,
            "out" => Self::Out,
            "on" => Self::On,
            _ => return None,
        })
    }

    /// `in`/`out` reserve a band; `on` is an absolute overlay (no reserve).
    pub fn reserves(self) -> bool {
        matches!(self, Self::In | Self::Out)
    }
}

/// A child's positioning, or `None` if it is a flow child (no `at:`/`side:`).
/// `at:` takes coords only; `side:` carries the edge anchor with its optional
/// `align` (default center) and `place` (default in).
pub fn read_pos(attrs: &AttrMap, span: Span) -> Result<Option<Pos>, Error> {
    if let Some(v) = attrs.get("at") {
        let (x, y) = as_pair(v, span)?;
        return Ok(Some(Pos::Coord(x, y)));
    }
    let Some(side_v) = attrs.get("side") else {
        return Ok(None);
    };
    let side = ident(side_v)
        .and_then(Side::parse)
        .ok_or_else(|| Error::at(span, "'side' expects top, bottom, left, or right"))?;
    let align = match attrs.get("align") {
        Some(v) => ident(v)
            .and_then(Align::parse)
            .ok_or_else(|| Error::at(span, "'align' expects start, center, or end"))?,
        None => Align::Center,
    };
    let place = match attrs.get("place") {
        Some(v) => ident(v)
            .and_then(Place::parse)
            .ok_or_else(|| Error::at(span, "'place' expects in or out"))?,
        None => Place::In,
    };
    Ok(Some(Pos::Edge { side, align, place }))
}

pub fn parse_offset(value: &ResolvedValue, span: Span) -> Result<(f64, f64), Error> {
    as_pair(value, span)
}

/// A child's layout role, by how it is positioned.
#[derive(Clone, Copy, PartialEq)]
pub enum Role {
    /// No `at:`/`side:` — laid out by the container's `layout`.
    Flow,
    /// `side:` with `place:in`/`out` — reserves a band; the parent grows.
    Reserve,
    /// `at:(x,y)`, or `side:` with `place:on` — an absolute overlay; the parent
    /// does not grow for it.
    Absolute,
}

/// Classify a child from its positioning attrs.
pub fn child_role(attrs: &AttrMap, span: Span) -> Result<Role, Error> {
    Ok(match read_pos(attrs, span)? {
        None => Role::Flow,
        Some(Pos::Coord(..)) => Role::Absolute,
        Some(Pos::Edge { place, .. }) if place.reserves() => Role::Reserve,
        Some(Pos::Edge { .. }) => Role::Absolute,
    })
}

/// Resolve a `Pos` into the child's target bbox-center in the parent's local
/// frame. `place` is size-aware: the child's facing edge lands flush on the
/// parent edge, inside or outside, by shifting half the child's extent.
pub fn resolve(pos: Pos, parent: Bbox, child: Bbox) -> (f64, f64) {
    let (cw, ch) = (child.w(), child.h());
    match pos {
        Pos::Coord(x, y) => (x, y),
        Pos::Edge { side, align, place } => {
            // Position along the anchored edge, from the child's extent on that axis.
            let along = |min: f64, max: f64, size: f64, align: Align| match align {
                Align::Start => min + size / 2.0,
                Align::Center => (min + max) / 2.0,
                Align::End => max - size / 2.0,
            };
            // Distance across the edge: inside pulls the child in by its half
            // extent, outside pushes it out (flush either way), `on` centres it
            // on the edge — so a corner anchor straddles the corner.
            let across = |edge: f64, half: f64, place: Place, outward: f64| match place {
                Place::In => edge - outward * half,
                Place::Out => edge + outward * half,
                Place::On => edge,
            };
            match side {
                Side::Top => (
                    along(parent.min_x, parent.max_x, cw, align),
                    across(parent.min_y, ch / 2.0, place, -1.0),
                ),
                Side::Bottom => (
                    along(parent.min_x, parent.max_x, cw, align),
                    across(parent.max_y, ch / 2.0, place, 1.0),
                ),
                Side::Left => (
                    across(parent.min_x, cw / 2.0, place, -1.0),
                    along(parent.min_y, parent.max_y, ch, align),
                ),
                Side::Right => (
                    across(parent.max_x, cw / 2.0, place, 1.0),
                    along(parent.min_y, parent.max_y, ch, align),
                ),
            }
        }
    }
}

fn ident(v: &ResolvedValue) -> Option<&str> {
    match v {
        ResolvedValue::Ident(s) => Some(s.as_str()),
        _ => None,
    }
}
