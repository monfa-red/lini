//! Column alignment for sibling boxes (SPEC §14): within a run of children with
//! no blank line between them, the id and `|type|` columns of the *boxes* line
//! up so their blocks start at the same offset. Bare-text children don't carry
//! id/type, so they take no alignment. A blank line starts a fresh group.
//!
//! Aligning a table's bare-text *cells* into visual columns (the flat table
//! form) is the formatter's table pass (SPEC §8), separate from this.

use super::trivia::{Trivia, TriviaToken};
use crate::span::Span;
use crate::syntax::ast::Child;

#[derive(Default, Clone, Copy)]
pub struct NodeWidths {
    /// Widest id in the group, 0 if none carry one.
    pub id: usize,
    /// Widest `|type|` (bars included) in the group, 0 if none carry one.
    pub ty: usize,
}

/// Per-child alignment widths: boxes sharing a blank-line-free group share the
/// group's max id / type widths; text children get zeros.
pub fn child_widths(children: &[Child], trivia: &[TriviaToken]) -> Vec<NodeWidths> {
    let mut out = vec![NodeWidths::default(); children.len()];
    for group in split_groups(children, trivia) {
        let mut w = NodeWidths::default();
        for &i in &group {
            if let Child::Box(n) = &children[i] {
                if let Some(id) = &n.id {
                    w.id = w.id.max(id.len());
                }
                if let Some(ty) = &n.ty {
                    w.ty = w.ty.max(ty.len() + 2); // |type|
                }
            }
        }
        for i in group {
            out[i] = w;
        }
    }
    out
}

/// Index groups of consecutive children uninterrupted by a blank line.
fn split_groups(children: &[Child], trivia: &[TriviaToken]) -> Vec<Vec<usize>> {
    if children.is_empty() {
        return Vec::new();
    }
    let mut groups: Vec<Vec<usize>> = vec![vec![0]];
    for i in 1..children.len() {
        let prev_end = child_span(&children[i - 1]).end;
        let curr_start = child_span(&children[i]).start;
        let blank = trivia.iter().any(|t| {
            matches!(t.kind, Trivia::BlankLine) && t.pos >= prev_end && t.pos < curr_start
        });
        if blank {
            groups.push(vec![i]);
        } else {
            groups.last_mut().unwrap().push(i);
        }
    }
    groups
}

fn child_span(c: &Child) -> Span {
    match c {
        Child::Box(n) => n.span,
        Child::Text(t) => t.span,
    }
}
