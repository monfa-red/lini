//! Stable diagnostic codes [ROADMAP 3.8, decision 7]: a **phase letter** —
//! `L`ex · `P`arse · `R`esolve · `V`alidate · la`Y`out · rou`T`e — then a
//! 3-digit number, e.g. `V001`. A code is **stable once assigned** (the
//! ROADMAP §2 promise); the message may still improve.
//!
//! The numbers live **only** in the [`catalog!`] table below — construction
//! sites name a `Code` const, never a literal, so a code cannot drift out from
//! under its family. A `x000` per phase is the generic fallback the phase
//! boundary stamps onto any diagnostic that names no specific family
//! ([`super::Error::in_phase`]); nothing is ever codeless. A new error family
//! opts into a stable number by adding one row here and naming it at the site.
//!
//! Two tests guard the set (`super::tests`): every code is unique, and a
//! snapshot pins each number to its family so a renumber fails CI.

/// The compile phase a diagnostic belongs to — the letter of its code.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Lex,
    Parse,
    Resolve,
    Validate,
    Layout,
    Route,
    /// Unclassified — the sentinel a fresh `Error`/`Diagnostic` carries until a
    /// phase boundary stamps it. Renders `E`; a diagnostic should never escape
    /// to a user wearing it.
    Internal,
}

impl Phase {
    pub fn letter(self) -> char {
        match self {
            Phase::Lex => 'L',
            Phase::Parse => 'P',
            Phase::Resolve => 'R',
            Phase::Validate => 'V',
            Phase::Layout => 'Y',
            Phase::Route => 'T',
            Phase::Internal => 'E',
        }
    }
}

/// A stable diagnostic code: its phase, its 3-digit number, and a snake-case
/// family id (the machine-readable label the JSON output and the pinning
/// snapshot carry). Equality is by `phase` + `num` — the uniqueness test keeps
/// those one-to-one with a family.
#[derive(Clone, Copy, Debug)]
pub struct Code {
    pub phase: Phase,
    pub num: u16,
    pub family: &'static str,
}

impl PartialEq for Code {
    fn eq(&self, other: &Self) -> bool {
        self.phase == other.phase && self.num == other.num
    }
}
impl Eq for Code {}

impl Code {
    /// The sentinel a fresh diagnostic carries until a phase boundary stamps a
    /// real phase onto it.
    pub const UNSPECIFIED: Code = Code {
        phase: Phase::Internal,
        num: 0,
        family: "unspecified",
    };

    /// Whether this is the pre-boundary sentinel — the phase stamp only fills
    /// these, never overwriting a named family code.
    pub fn is_unspecified(self) -> bool {
        self.phase == Phase::Internal
    }

    /// This code if it names a family, else the phase's generic `x000` — the
    /// one phase stamp, worn by [`crate::error::Error::in_phase`], its
    /// [`crate::error::Diagnostic`] twin, and the batch
    /// [`crate::error::stamp_phase`].
    pub fn or_generic(self, phase: Phase) -> Code {
        if self.is_unspecified() {
            Code::generic(phase)
        } else {
            self
        }
    }

    /// The phase's generic `x000` code — what the boundary stamps onto an
    /// untriaged diagnostic.
    pub fn generic(phase: Phase) -> Code {
        match phase {
            Phase::Lex => Code::LEX,
            Phase::Parse => Code::PARSE,
            Phase::Resolve => Code::RESOLVE,
            Phase::Validate => Code::VALIDATE,
            Phase::Layout => Code::LAYOUT,
            Phase::Route => Code::ROUTE,
            Phase::Internal => Code::UNSPECIFIED,
        }
    }

    /// The rendered code, e.g. `"V001"`.
    pub fn as_str(self) -> String {
        format!("{}{:03}", self.phase.letter(), self.num)
    }
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{:03}", self.phase.letter(), self.num)
    }
}

/// Declare the catalog: one `Phase Num CONST "family"` row each. Generates the
/// `Code` consts and the `CATALOG` slice the guard tests walk.
macro_rules! catalog {
    ( $( $phase:ident $num:literal $const_name:ident $family:literal ; )* ) => {
        impl Code {
            $( pub const $const_name: Code = Code {
                phase: Phase::$phase, num: $num, family: $family,
            }; )*
        }
        /// Every catalogued code, in declaration order — the guard tests and
        /// external tooling walk this; not referenced on the compile hot path.
        #[allow(dead_code)]
        pub const CATALOG: &[Code] = &[ $( Code::$const_name ),* ];
    };
}

catalog! {
    // ── Lex [SPEC 2] ──
    Lex 0 LEX "lex";
    Lex 1 UNTERMINATED_STRING "unterminated-string";
    Lex 2 BAD_ESCAPE "bad-escape";
    Lex 3 BAD_NUMBER "bad-number";
    Lex 4 UNEXPECTED_CHAR "unexpected-char";

    // ── Parse [SPEC 3] ──
    Parse 0 PARSE "parse";
    Parse 1 EXPECTED_TOKEN "expected-token";
    Parse 2 EMPTY_BARS "empty-bars";
    Parse 3 INVALID_ID "invalid-id";
    Parse 4 DECL_OUTSIDE_BLOCK "declaration-outside-block";
    Parse 5 STYLESHEET_ORDER "stylesheet-after-canvas";
    Parse 6 TEXT_CHILDREN "text-carries-children";
    Parse 7 CALL_GLUE "spaced-call-paren";

    // ── Resolve [SPEC 6/8] — desugar/lowering counts as resolve. ──
    Resolve 0 RESOLVE "resolve";
    Resolve 1 UNKNOWN_TYPE "unknown-type";
    Resolve 2 UNKNOWN_CLASS "unknown-class";
    Resolve 3 DUPLICATE_ID "duplicate-id";
    Resolve 4 INHERIT_CYCLE "inheritance-cycle";
    Resolve 5 INHERIT_DEPTH "inheritance-depth";
    Resolve 6 SHADOWS_BUILTIN "shadows-builtin";
    Resolve 7 RESERVED_ID "reserved-id";
    Resolve 8 UNKNOWN_ENDPOINT "unknown-endpoint";
    Resolve 9 CHAIN_TOO_SHORT "chain-too-short";
    Resolve 10 ASSET_NOT_FOUND "asset-not-found";
    Resolve 11 ASSET_ESCAPES_ROOT "asset-escapes-root";
    Resolve 12 PROJECTION "projection-link";
    Resolve 13 LEGACY_LIST "legacy-space-list";
    Resolve 14 UNKNOWN_SIDE "unknown-side";
    Resolve 15 UNKNOWN_STRATEGY "unknown-strategy";
    Resolve 16 CAPSULE_IN_DRAWING "capsule-in-drawing";
    Resolve 17 SCHEMATIC_POSE "schematic-pose";
    Resolve 18 PIN_SLIDE "pin-slide";
    Resolve 19 SCHEMATIC_MARKER "schematic-marker";
    Resolve 20 SCHEMATIC_INVENT "schematic-invent";
    Resolve 21 SCHEMATIC_ARITY "schematic-arity";
    Resolve 22 DUPLICATE_WIRE "duplicate-wire";
    Resolve 23 COLOR_RANGE "color-component-out-of-range";

    // ── Validate [SPEC 17/21] ──
    Validate 0 VALIDATE "validate";
    Validate 1 UNKNOWN_PROPERTY "unknown-property";
    Validate 2 MISUSED_PROPERTY "misused-property";
    Validate 3 INERT_EVERY_WEARER "inert-on-every-wearer";
    Validate 4 CLASS_NEVER_WORN "class-never-worn";
    Validate 5 MALFORMED_VALUE "malformed-value";
    Validate 6 OFF_GRID_PLACEMENT "off-grid-placement";
    Validate 7 PLACE_OUTSIDE_SEQUENCE "place-outside-sequence";
    Validate 8 ACTIVATION_OUTSIDE_SEQUENCE "activation-outside-sequence";
    Validate 9 WAVY_OUTLINE "wavy-outline";
    Validate 10 INVALID_COLOR "invalid-color";
    Validate 11 DEFERRED_PROPERTY "deferred-property";

    // ── Layout [SPEC 11–15] ──
    Layout 0 LAYOUT "layout";
    Layout 1 MISSING_REQUIRED "missing-required-property";
    Layout 2 CHART_DATA "chart-data";
    Layout 3 PROJECT_AXIS "project-axis-mismatch";
    Layout 4 DRAWING_MEASURE "drawing-measure";
    Layout 5 SCHEMATIC_TRACKS "schematic-tracks";
    Layout 6 SIDE_ON_TERMINAL "side-on-terminal";
    Layout 7 SCHEMATIC_SEAT "schematic-seat";
    Layout 8 SCHEMATIC_TYPE "schematic-type";
    Layout 9 CHART_TYPE "chart-type";
    Layout 10 FLOORPLAN_TYPE "floorplan-type";
    Layout 11 OPENING_HOST "opening-host";
    Layout 12 OPENING_PLACED "opening-placed";
    Layout 13 SLIDING_LEAF "sliding-door-leaf";
    Layout 14 WALL_CURVE "wall-curve";
    Layout 15 WALL_ARC "arc-under-thickness";

    // ── Route [ROUTING] — the routing engine's own law checker. ──
    Route 0 ROUTE "route";
    Route 1 IMPOSSIBLE_LINK "impossible-link";
    Route 2 LAW_BREACH "law-breach";
}
