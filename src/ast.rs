//! Shared lexical primitives — the small enums the lexer produces and both the
//! parser and the back end consume: edge [`Side`] and the link-operator triple
//! ([`LinkOp`] / [`LineStyle`] / [`LinkMarker`]). The syntax tree itself lives
//! in [`crate::syntax::ast`]; this module is just the vocabulary they share,
//! kept here so the lexer doesn't depend on the parser.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
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

    /// Dense id (clockwise from top) — the routing stages' map key.
    pub fn index(self) -> u8 {
        match self {
            Side::Top => 0,
            Side::Right => 1,
            Side::Bottom => 2,
            Side::Left => 3,
        }
    }

    /// The side's spelling — `parse`'s inverse.
    pub fn name(self) -> &'static str {
        match self {
            Side::Top => "top",
            Side::Bottom => "bottom",
            Side::Left => "left",
            Side::Right => "right",
        }
    }

    /// The side's outward unit normal (y down).
    pub fn outward(self) -> (f64, f64) {
        match self {
            Side::Top => (0.0, -1.0),
            Side::Bottom => (0.0, 1.0),
            Side::Left => (-1.0, 0.0),
            Side::Right => (1.0, 0.0),
        }
    }
}

// ─────────────────────────── Link ops ───────────────────────────

/// A composed link operator: `[start_marker?][line][end_marker?]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkOp {
    pub line: LineStyle,
    pub start: LinkMarker,
    pub end: LinkMarker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStyle {
    Solid,  // -
    Dashed, // --
    Dotted, // ---
    Wavy,   // ~
}

impl LineStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Solid => "-",
            Self::Dashed => "--",
            Self::Dotted => "---",
            Self::Wavy => "~",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LinkMarker {
    #[default]
    None,
    Arrow,   // < at start, > at end
    Crow,    // > at start, < at end
    Dot,     // * on either side
    Diamond, // <> on either side
    // The ER cardinality end-markers [SPEC 9] — composed `[min][max]`, end-side
    // only (a start side mirrors only the simple crow). `Crow` above is "many".
    One,        // -+
    ExactlyOne, // -++
    ZeroOrOne,  // -o+
    OneOrMany,  // -+<
    ZeroOrMany, // -o<
}

/// A drawing measuring op [SPEC 15.6]: `(-)` the linear measure (a length,
/// binary), `(o)` the round measure (⌀ / R by the feature, unary), `(<)` the
/// angle. Each glyph pictures what it measures. Lexed as glued three-char tokens
/// only where a `(` is free-standing — a `(` glued to an ident opens a call [SPEC 2].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawOp {
    Linear, // (-)
    Round,  // (o)
    Angle,  // (<)
}

impl DrawOp {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Linear => "(-)",
            Self::Round => "(o)",
            Self::Angle => "(<)",
        }
    }
}

/// A link statement's operator [SPEC 9, 15]: a core wire op, a measuring op, or
/// the mate `||`. One statement carries one op; a chain never mixes them. The
/// mate has no token of its own — the parser reads two **adjacent** pipes at
/// operator position, so bars stay paired everywhere else [SPEC 21].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainOp {
    Wire(LinkOp),
    Measure(DrawOp),
    Mate, // ||
}

impl ChainOp {
    /// The op's source spelling, for diagnostics.
    pub fn spelling(self) -> String {
        match self {
            Self::Wire(op) => format!(
                "{}{}{}",
                op.start.start_str(),
                op.line.as_str(),
                op.end.end_str()
            ),
            Self::Measure(d) => d.as_str().to_string(),
            Self::Mate => "||".to_string(),
        }
    }

    /// The wire triple, for the ops that draw one (`None` for measure / mate).
    pub fn wire(self) -> Option<LinkOp> {
        match self {
            Self::Wire(op) => Some(op),
            _ => None,
        }
    }
}

impl LinkMarker {
    /// Glyph for this marker when rendered at the start side of a link op.
    pub fn start_str(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Arrow => "<",
            Self::Crow => ">",
            Self::Dot => "*",
            Self::Diamond => "<>",
            // The cardinality markers mirror at the start [SPEC 9]: the max glyph
            // (bar, or the `>` crow) sits outermost, the min ring / bar hugs the line.
            Self::One => "+",
            Self::ExactlyOne => "++",
            Self::ZeroOrOne => "+o",
            Self::OneOrMany => ">+",
            Self::ZeroOrMany => ">o",
        }
    }
    /// Glyph for this marker when rendered at the end side of a link op.
    pub fn end_str(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Arrow => ">",
            Self::Crow => "<",
            Self::Dot => "*",
            Self::Diamond => "<>",
            Self::One => "+",
            Self::ExactlyOne => "++",
            Self::ZeroOrOne => "o+",
            Self::OneOrMany => "+<",
            Self::ZeroOrMany => "o<",
        }
    }
}
