//! A schematic part's **pose** [SPEC 16.1]: `rotate:` in 90° steps, read at
//! lowering. A part carries connection geometry — pins on a component, glyph
//! ports on a symbol, one connection point on a label — so a turn is
//! **structural**: pins re-side, the symbol's `d` and its ports re-lay, and
//! every text (pin names and numbers, ref, value, net text) stays upright,
//! because nothing here is a paint transform. Any other angle is an error.
//!
//! The pose is consumed at lowering and left behind as a generated class
//! (`lini-pose-90`), exactly as the family is [SPEC 16.7] — so the engine
//! reads a part's pose back off its chain (a label's connection point is the
//! registry's, turned by this) while `rotate:` never reaches the renderer.

use super::Lower;
use crate::error::{Code, Error};
use crate::span::Span;
use crate::syntax::ast::{Decl, Node, Value};

/// One side of a part — where a pin lives and which way its stub points.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    pub(crate) const ALL: [Side; 4] = [Side::Left, Side::Right, Side::Top, Side::Bottom];

    pub(crate) fn parse(s: &str) -> Option<Side> {
        match s {
            "left" => Some(Side::Left),
            "right" => Some(Side::Right),
            "top" => Some(Side::Top),
            "bottom" => Some(Side::Bottom),
            _ => None,
        }
    }

    /// The side's slot in a rail table — the four rails a component builds.
    pub(crate) fn index(self) -> usize {
        match self {
            Side::Left => 0,
            Side::Right => 1,
            Side::Top => 2,
            Side::Bottom => 3,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Right => "right",
            Side::Top => "top",
            Side::Bottom => "bottom",
        }
    }

    /// Whether the side runs vertically — its pins stack, and a pin slides
    /// along `y`; a horizontal side's slides along `x` [SPEC 16.2].
    pub(crate) fn is_vertical(self) -> bool {
        matches!(self, Side::Left | Side::Right)
    }

    /// The side across the box — a half turn, so there is no second table.
    pub(crate) fn opposite(self) -> Side {
        Pose(2).side(self)
    }

    /// The side's **outward unit normal** in scene coordinates (`y` grows
    /// down) — where a terminal on this side points, and the direction a
    /// satellite chain grows [SPEC 16.1].
    pub(crate) fn normal(self) -> (f64, f64) {
        crate::ast::Side::from(self).outward()
    }

    /// The direction a rail reads in: a column top-to-bottom, a row
    /// left-to-right. A turn maps this vector, and the target side either
    /// reads the same way or backwards — which is what [`Pose::flips`] asks.
    fn reading(self) -> (i8, i8) {
        if self.is_vertical() { (0, 1) } else { (1, 0) }
    }
}

/// The same four names in the router's vocabulary (ROUTING.md) — a part's
/// forced side crosses into the routing contract here and nowhere else.
impl From<Side> for crate::ast::Side {
    fn from(s: Side) -> crate::ast::Side {
        match s {
            Side::Left => crate::ast::Side::Left,
            Side::Right => crate::ast::Side::Right,
            Side::Top => crate::ast::Side::Top,
            Side::Bottom => crate::ast::Side::Bottom,
        }
    }
}

/// A part's pose: clockwise quarter turns, `0..4`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Pose(u8);

impl Pose {
    pub(crate) const NONE: Pose = Pose(0);

    /// The four poses **in tie-break order** — the unrotated one, then
    /// clockwise [SPEC 16.1]. [`super::autopose`] walks this and takes the
    /// first pose that faces its anchor; the order *is* the tie-break, so
    /// there is no second rule to remember.
    pub(crate) const ALL: [Pose; 4] = [Pose(0), Pose(1), Pose(2), Pose(3)];

    pub(crate) fn is_turned(self) -> bool {
        self.0 != 0
    }

    /// Whether this pose swaps the box's axes — a quarter or three-quarter
    /// turn (a half turn keeps them).
    pub(crate) fn swaps_axes(self) -> bool {
        self.0 % 2 == 1
    }

    /// The pose `deg` names — any multiple of 90, normalized clockwise.
    pub(super) fn from_degrees(deg: f64, span: Span) -> Result<Pose, Error> {
        let quarters = deg / 90.0;
        if quarters.fract() != 0.0 {
            return Err(Error::at(
                span,
                "a schematic part rotates in 90° steps — 0, 90, 180, or 270",
            )
            .code(Code::SCHEMATIC_POSE));
        }
        Ok(Pose(quarters.rem_euclid(4.0) as u8))
    }

    /// The class a turned part wears, so the engine reads the pose back.
    fn class(self) -> Option<String> {
        (self.0 != 0).then(|| format!("lini-pose-{}", self.0 as u32 * 90))
    }

    /// The degrees this pose is written as — what [`set_rotate`] authors.
    fn degrees(self) -> f64 {
        self.0 as f64 * 90.0
    }

    /// The pose a lowered part wears — its `lini-*` classes with the prefix
    /// stripped, the same chain [`super::schematic::sch_kind`] reads. The
    /// engine's read-back: a label's connection point is the registry port
    /// turned by this (a component's pins and a symbol's ports lower turned,
    /// so they need no reader).
    pub(crate) fn of_chain<S: AsRef<str>>(chain: &[S]) -> Pose {
        for q in 1..4u8 {
            let name = format!("pose-{}", q as u32 * 90);
            if chain.iter().any(|t| t.as_ref() == name) {
                return Pose(q);
            }
        }
        Pose::NONE
    }

    /// The side a pin authored on `side` wears after the turn.
    pub(crate) fn side(self, side: Side) -> Side {
        let mut s = side;
        for _ in 0..self.0 {
            // One clockwise quarter: the left edge swings up to the top.
            s = match s {
                Side::Left => Side::Top,
                Side::Top => Side::Right,
                Side::Right => Side::Bottom,
                Side::Bottom => Side::Left,
            };
        }
        s
    }

    /// Whether the pins of `side` read backwards on the side they land on —
    /// a rigid turn keeps their physical order, but a column reads
    /// top-to-bottom and a row left-to-right, so half the landings reverse.
    pub(crate) fn flips(self, side: Side) -> bool {
        let mut d = side.reading();
        for _ in 0..self.0 {
            d = (-d.1, d.0);
        }
        d != self.side(side).reading()
    }

    /// A point of the `w` × `h` glyph box after the turn — the one map the
    /// symbol's `d` and its ports both take.
    pub(crate) fn point(self, p: (f64, f64), w: f64, h: f64) -> (f64, f64) {
        crate::path_data::point(p, self.0, w, h)
    }

    /// A free vector (a `translate:` nudge) after the turn — the same map with
    /// no box to re-anchor it to.
    pub(crate) fn vector(self, v: (f64, f64)) -> (f64, f64) {
        crate::path_data::point(v, self.0, 0.0, 0.0)
    }

    /// The glyph's `d`, re-laid.
    pub(crate) fn path(self, d: &str, w: f64, h: f64) -> String {
        crate::path_data::rotated(d, self.0, w, h)
    }
}

/// Which side of its own `w` × `h` box a connection point sits on — the
/// direction that terminal **faces** [SPEC 16.1]. `None` when no single edge
/// is nearest (a point at the centre, or on a corner): such a terminal has no
/// facing to turn toward an anchor, and its chain falls back to the pin's own
/// outward normal.
///
/// A pose commutes with this — `facing(pose.point(p, w, h), …) ==
/// pose.side(facing(p, w, h))` — so the base facing is read once off the
/// registry and turned with [`Pose::side`].
pub(crate) fn facing(p: (f64, f64), w: f64, h: f64) -> Option<Side> {
    let reach = [
        (Side::Left, p.0),
        (Side::Right, w - p.0),
        (Side::Top, p.1),
        (Side::Bottom, h - p.1),
    ];
    let nearest = reach.iter().fold(f64::INFINITY, |m, &(_, d)| m.min(d));
    let mut hits = reach.iter().filter(|&&(_, d)| d <= nearest + 1e-9);
    match (hits.next(), hits.next()) {
        (Some(&(side, _)), None) => Some(side),
        _ => None,
    }
}

/// Take the part's pose out of its style [SPEC 16.1]: read `rotate:` (the
/// node's own, else its defines' / element rules'), then make sure nothing
/// downstream paints it — the turn is structural, and a part's texts must
/// stand upright. A pose read from the chain is neutralized with an explicit
/// `rotate: 0`, which beats the class rule it came from.
pub(super) fn take(
    cx: &Lower,
    chain: &[String],
    style: &mut Vec<Decl>,
    span: Span,
) -> Result<Pose, Error> {
    let Some(deg) = cx.chain_number(chain, style, "rotate") else {
        return Ok(Pose::NONE);
    };
    let pose = Pose::from_degrees(deg, span)?;
    if pose.is_turned() {
        // A rule the instance can't delete needs cancelling, not dropping —
        // ask the chain alone, since the node's own decl is about to go.
        let from_a_rule = cx.chain_decl(chain, &[], "rotate").is_some();
        style.retain(|d| d.name != "rotate");
        if from_a_rule {
            style.push(Decl {
                name: "rotate".into(),
                groups: vec![vec![Value::Number(0.0)]],
                span: Span::empty(),
            });
        }
    }
    Ok(pose)
}

/// Leave the pose behind as a class, so the engine reads it back.
pub(super) fn mark(pose: Pose, classes: &mut Vec<String>) {
    classes.extend(pose.class());
}

/// Write a decided pose onto an **authored** part, before it lowers — as the
/// `rotate:` a user would have written, so it takes [`take`]'s path and there
/// is exactly one applier. The decision is [`super::autopose`]'s.
pub(crate) fn set_rotate(part: &mut Node, pose: Pose) {
    part.style.retain(|d| d.name != "rotate");
    part.style.push(Decl {
        name: "rotate".into(),
        groups: vec![vec![Value::Number(pose.degrees())]],
        span: Span::empty(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::ast::Child;

    fn pose(q: u8) -> Pose {
        Pose(q)
    }

    #[test]
    fn a_quarter_turn_walks_the_sides_clockwise() {
        let p = pose(1);
        assert!(p.side(Side::Left) == Side::Top);
        assert!(p.side(Side::Top) == Side::Right);
        assert!(p.side(Side::Right) == Side::Bottom);
        assert!(p.side(Side::Bottom) == Side::Left);
        // A half turn is the opposite side, whole turn the identity.
        assert!(pose(2).side(Side::Left) == Side::Right);
        assert!(pose(0).side(Side::Left) == Side::Left);
    }

    #[test]
    fn a_rigid_turn_flips_the_rails_that_change_reading_direction() {
        // Rotating clockwise, the left column's top pin lands rightmost on the
        // top row — so that rail reverses; the top row's reads straight down
        // the right column.
        assert!(pose(1).flips(Side::Left));
        assert!(!pose(1).flips(Side::Top));
        assert!(pose(1).flips(Side::Right));
        assert!(!pose(1).flips(Side::Bottom));
        // A half turn reverses every rail; a whole turn none.
        assert!(Side::ALL.iter().all(|s| pose(2).flips(*s)));
        assert!(Side::ALL.iter().all(|s| !pose(0).flips(*s)));
        // Three quarters is one quarter widdershins.
        assert!(!pose(3).flips(Side::Left));
        assert!(pose(3).flips(Side::Top));
    }

    #[test]
    fn a_turn_maps_points_and_vectors_alike() {
        // The 64×12 resistor glyph: its right port swings to the bottom.
        assert_eq!(pose(1).point((64.0, 6.0), 64.0, 12.0), (6.0, 64.0));
        // A pin's downward slide becomes a leftward one.
        assert_eq!(pose(1).vector((0.0, 10.0)), (-10.0, 0.0));
    }

    #[test]
    fn only_right_angles_pose_a_part() {
        for deg in [0.0, 90.0, 180.0, 270.0, 360.0, -90.0] {
            assert!(Pose::from_degrees(deg, Span::empty()).is_ok(), "{deg}");
        }
        assert_eq!(Pose::from_degrees(-90.0, Span::empty()).unwrap(), pose(3));
        assert_eq!(Pose::from_degrees(360.0, Span::empty()).unwrap(), pose(0));
        let err = Pose::from_degrees(45.0, Span::empty()).unwrap_err();
        assert!(err.to_string().contains("90° steps"), "{err}");
    }

    #[test]
    fn the_pose_rides_a_class_the_engine_reads_back() {
        for (i, p) in Pose::ALL.into_iter().enumerate() {
            let mut classes = vec!["lini-R".to_string()];
            mark(p, &mut classes);
            let chain: Vec<&str> = classes
                .iter()
                .filter_map(|c| c.strip_prefix("lini-"))
                .collect();
            assert!(Pose::of_chain(&chain) == p, "pose {i}");
        }
        // The candidate order **is** the tie-break: unrotated, then clockwise.
        assert_eq!(
            Pose::ALL.map(|p| p.degrees()),
            [0.0, 90.0, 180.0, 270.0],
            "the chooser's walk order"
        );
    }

    /// Lower `src`, optionally posing its first instance through the seam
    /// first — the chooser's path, as `autopose::choose` runs it.
    fn lowered(src: &str, decide: Option<Pose>) -> String {
        let toks = crate::lexer::lex(src).expect("lex");
        let mut file = crate::syntax::parser::parse(src, &toks).expect("parse");
        if let Some(p) = decide
            && let Child::Box(part) = &mut file.instances[0]
        {
            set_rotate(part, p);
        }
        crate::fmt::print_file(&crate::desugar::desugar(&file).expect("desugar"))
    }

    #[test]
    fn a_pose_decided_before_lowering_is_the_authored_pose() {
        // The seam's whole contract [SPEC 16.1]: a decision written onto the
        // authored part takes the one applier's path, so an auto-pose and an
        // authored `rotate:` cannot drift — there is nothing to keep in step.
        assert_eq!(
            lowered("|R#r1| \"1k\"\n", Some(pose(1))),
            lowered("|R#r1| \"1k\" { rotate: 90 }\n", None),
        );
        // …including on a part whose pins re-side.
        assert_eq!(
            lowered("|component#U7| [ |pin#a|; |pin#b| ]\n", Some(pose(2))),
            lowered(
                "|component#U7| { rotate: 180 } [ |pin#a|; |pin#b| ]\n",
                None
            ),
        );
    }

    #[test]
    fn a_decided_pose_is_written_as_the_authored_rotate() {
        // The auto-pose seam's writer: the chooser decides, this authors, `take`
        // applies — one applier. An earlier decision is replaced, never
        // stacked.
        let mut part = Node {
            id: None,
            ty: Some("R".into()),
            label: None,
            classes: Vec::new(),
            style: vec![Decl {
                name: "rotate".into(),
                groups: vec![vec![Value::Number(90.0)]],
                span: Span::empty(),
            }],
            style_span: None,
            children: Vec::new(),
            links: Vec::new(),
            span: Span::empty(),
        };
        set_rotate(&mut part, pose(3));
        let turns: Vec<&Decl> = part.style.iter().filter(|d| d.name == "rotate").collect();
        assert_eq!(turns.len(), 1, "one decl, not a stack");
        assert!(
            matches!(turns[0].groups.first().and_then(|g| g.first()), Some(Value::Number(n)) if *n == 270.0),
            "{:?}",
            turns[0].groups
        );
    }
}
