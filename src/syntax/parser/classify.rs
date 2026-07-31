//! Statement classification — one token of lookahead decides what a line is.

use super::*;

impl<'a> Parser<'a> {
    // ───────────────────────── Classification ─────────────────────────

    /// A stylesheet item: a declaration, a `--var`, a rule (incl. `.class`), or a
    /// define (`|name::base|`). Assumes newlines skipped.
    pub(super) fn classify_setup(&self) -> Result<Kind, Error> {
        match self.kind() {
            Some(TokKind::RawCssVar(_)) => Ok(Kind::Var),
            Some(TokKind::Dot) => Ok(Kind::Rule),  // .class { … }
            Some(TokKind::Hash(_)) => Ok(Kind::Rule), // #hero { … } — an id rule
            Some(TokKind::LinkOp(_)) => Err(self.err(
                "'->' draws a link on the canvas — style every link with '|-| { stroke: … }' in a '{ }' block",
            )),
            // `(-) { … }` is the dimension selector [SPEC 4, 15.6] — the `|-|` subtype;
            // an operator only appears after endpoints, so a leading `(-)` is a rule.
            Some(TokKind::DrawOp(DrawOp::Linear)) => Ok(Kind::Rule),
            // Per-kind dimension selectors `(o) { }` / `(<) { }` are deferred [SPEC 24].
            Some(TokKind::DrawOp(_)) => Err(self.err(
                "'(-)' selects every dimension — per-kind '(o)' / '(<)' selectors are deferred (SPEC 24)",
            )),
            Some(TokKind::Pipe) => Ok(
                // `|name::base|` is a define; any other `|…|` is a rule selector.
                if matches!(self.kind_at(1), Some(TokKind::Ident(_)))
                    && matches!(self.kind_at(2), Some(TokKind::DColon))
                {
                    Kind::Define
                } else {
                    Kind::Rule
                },
            ),
            Some(TokKind::Ident(_)) => match self.kind_at(1) {
                Some(TokKind::Colon) => Ok(Kind::Decl),
                // `name = value` / `name(params) = value` is an `=` binding [SPEC 10.7].
                Some(TokKind::Assign) | Some(TokKind::LParen) => Ok(Kind::Func),
                _ => Err(self
                    .err("a type only appears in bars — write '|box| { }' to style every box")),
            },
            _ => Err(self.err("the stylesheet holds declarations, rules, and defines")),
        }
    }

    /// A canvas / body statement: a node (`|…|`), text (`"…"`), a link (a bare id
    /// followed by a link-op / `&` / a `.path` / a `:side`), or — flagged for a
    /// context error — a stray declaration or `--var`. A bare leading name with no
    /// link follow is invalid (a node leads with bars). Assumes newlines skipped.
    pub(super) fn classify_body(&self) -> Kind {
        match self.kind() {
            // A statement-head capsule resolves node-vs-link on what follows
            // the closed bars [SPEC 1/22]: an op / `&` / mate `||` (directly,
            // or after the capsule's glued endpoint anatomy) opens a
            // capsule-headed link; anything else is the node declaration it
            // always was. Bars that are not a capsule (`|-|`) stay a node
            // statement and error in the node parser.
            Some(TokKind::Pipe) => match self.capsule_width_at(0) {
                Some(w) if self.capsule_heads_link(w) => Kind::Link,
                _ => Kind::Node,
            },
            Some(TokKind::String(_)) => Kind::Node,
            Some(TokKind::RawCssVar(_)) => Kind::Var,
            Some(TokKind::Ident(_)) => match self.kind_at(1) {
                Some(TokKind::LinkOp(_)) | Some(TokKind::DrawOp(_)) | Some(TokKind::Amp) => {
                    Kind::Link
                }
                Some(TokKind::Dot) if self.glued_at(1) => Kind::Link, // a.b endpoint path
                // `a || b` — a mate [SPEC 15.5]: two adjacent pipes at operator
                // position (a node can never follow a bare ident, so this is
                // unambiguous and bars stay paired).
                Some(TokKind::Pipe) if self.pipes_glued_at(1) => Kind::Link,
                // `a:left -> b` is a sided first endpoint — `:ident` then a link-op
                // / `&`. A misplaced `gap: 4` decl has a value there, not `side ->`,
                // so it stays a (context-error) declaration; an invalid point then
                // surfaces as the proper anchor error at resolve.
                Some(TokKind::Colon)
                    if matches!(self.kind_at(2), Some(TokKind::Ident(_)))
                        && (matches!(
                            self.kind_at(3),
                            Some(TokKind::LinkOp(_))
                                | Some(TokKind::DrawOp(_))
                                | Some(TokKind::Amp)
                        ) || self.pipes_glued_at(3)) =>
                {
                    Kind::Link
                }
                Some(TokKind::Colon) => Kind::Decl,
                _ => Kind::Unknown,
            },
            _ => Kind::Unknown,
        }
    }

    /// The token width of a well-formed identity capsule at `pos + n` [SPEC 1/9]:
    /// `|type|` / `|#id|` → 3, `|type#id|` → 4. `None` when the bars there are
    /// not a capsule — `|-|` (the link selector), a define's `::`, unclosed bars.
    pub(super) fn capsule_width_at(&self, n: usize) -> Option<usize> {
        if !matches!(self.kind_at(n), Some(TokKind::Pipe)) {
            return None;
        }
        match self.kind_at(n + 1) {
            Some(TokKind::Ident(_)) => match self.kind_at(n + 2) {
                Some(TokKind::Pipe) => Some(3),
                Some(TokKind::Hash(_)) if matches!(self.kind_at(n + 3), Some(TokKind::Pipe)) => {
                    Some(4)
                }
                _ => None,
            },
            Some(TokKind::Hash(_)) if matches!(self.kind_at(n + 2), Some(TokKind::Pipe)) => Some(3),
            _ => None,
        }
    }

    /// Whether the tokens after a closed statement-head capsule (ending at
    /// `pos + w`) read as a **link** [SPEC 1/22]: a link op, `&`, or the mate
    /// `||` — directly after the bars, or after the capsule's glued endpoint
    /// anatomy (a `.path` / `.index` run, then an optional `:point`). A spaced
    /// `.class`, a label, a `{ }`, or a `[ ]` keeps the node reading.
    fn capsule_heads_link(&self, w: usize) -> bool {
        // The glued dotted run — `.seg` segments / a `.2` copy index [SPEC 9].
        let mut j = w;
        while matches!(self.kind_at(j), Some(TokKind::Dot))
            && self.glued_at(j)
            && matches!(
                self.kind_at(j + 1),
                Some(TokKind::Ident(_)) | Some(TokKind::Number(_))
            )
        {
            j += 2;
        }
        match self.kind_at(j) {
            Some(TokKind::LinkOp(_)) | Some(TokKind::DrawOp(_)) | Some(TokKind::Amp) => true,
            Some(TokKind::Pipe) if self.pipes_glued_at(j) => true,
            // `|cyl|:left -> x` — a sided capsule endpoint, the ident case's
            // mirror: `:point` then an op / `&` / mate.
            Some(TokKind::Colon)
                if matches!(self.kind_at(j + 1), Some(TokKind::Ident(_)))
                    && (matches!(
                        self.kind_at(j + 2),
                        Some(TokKind::LinkOp(_)) | Some(TokKind::DrawOp(_)) | Some(TokKind::Amp)
                    ) || self.pipes_glued_at(j + 2)) =>
            {
                true
            }
            _ => false,
        }
    }
}
