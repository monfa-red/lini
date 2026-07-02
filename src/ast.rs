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
    /// In `index` order — the canonical side enumeration.
    // Scaffold: consumed again by the search stage (ROUTING-V2.md stage 2).
    #[allow(dead_code)]
    pub const ALL: [Side; 4] = [Side::Top, Side::Right, Side::Bottom, Side::Left];

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
        }
    }
}
