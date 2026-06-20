//! Column alignment for sibling boxes (SPEC §14): within a run of children with
//! no blank line between them, the id and `|type|` columns of the *boxes* line
//! up so their labels start at the same offset. A blank line starts a fresh
//! group.
//!
//! Alignment is for a clean table of plain `id |type| [label]` rows, so it kicks
//! in only when *every* box in the group is plain — a `.class` or `{ }` block
//! opts the group out (it stays ragged, single-spaced), exactly as a non-text
//! cell drops a table's grid alignment (the separate table pass, SPEC §8).

use super::trivia::{Trivia, TriviaToken};
use crate::span::Span;
use crate::syntax::ast::{Child, Node};

#[derive(Default, Clone, Copy)]
pub struct NodeWidths {
    /// Widest id in the group, 0 if none carry one or the group is ragged.
    pub id: usize,
    /// Widest `|type|` bars in the group, 0 if none carry a type or it is ragged.
    pub ty: usize,
}

/// The rendered width of a node's `|type|` bars, or 0 when it has no type.
fn type_len(n: &Node) -> usize {
    n.ty.as_ref().map_or(0, |t| 2 + t.len())
}

/// A child eligible for column alignment: a *plain* box (no `.class`, no `{ }`
/// block — a bare `id |type|` row), or bare text (which never aligns but does
/// not disqualify a group). A class or block opts its whole group out.
fn alignable(c: &Child) -> bool {
    match c {
        Child::Box(n) => n.classes.is_empty() && n.style.is_empty(),
        Child::Text(_) => true,
    }
}

/// Per-child alignment widths: boxes in a blank-line-free, all-plain group share
/// the group's max id / type widths; every other child (and ragged group) gets
/// zeros.
pub fn child_widths(children: &[Child], trivia: &[TriviaToken]) -> Vec<NodeWidths> {
    let mut out = vec![NodeWidths::default(); children.len()];
    for group in split_groups(children, trivia) {
        if !group.iter().all(|&i| alignable(&children[i])) {
            continue; // a class or block makes the whole group ragged
        }
        let mut w = NodeWidths::default();
        for &i in &group {
            if let Child::Box(n) = &children[i] {
                if let Some(id) = &n.id {
                    w.id = w.id.max(id.len());
                }
                w.ty = w.ty.max(type_len(n));
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
