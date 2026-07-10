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
            // Per-kind dimension selectors `(o) { }` / `(<) { }` are deferred [SPEC 23].
            Some(TokKind::DrawOp(_)) => Err(self.err(
                "'(-)' selects every dimension — per-kind '(o)' / '(<)' selectors are deferred (SPEC 23)",
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
            Some(TokKind::Pipe) | Some(TokKind::String(_)) => Kind::Node,
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
}
