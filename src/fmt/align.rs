//! Column alignment for sibling boxes (SPEC §14): within a run of children with
//! no blank line between them, the id column aligns so the `|type|` bars line
//! up. A group of *plain* boxes — no `.class`, no `{ }` block — additionally
//! aligns the type column, so their labels start at the same offset; a class or
//! block keeps that column ragged (a label never pads past a class or block,
//! and no class column is reserved). A blank line starts a fresh group.

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

/// A *plain* box — no `.class`, no `{ }` block — so the group may also align the
/// type column (the labels). Bare text counts as plain (it never aligns). A
/// class or block leaves the type column ragged; the id column still aligns.
fn is_plain(c: &Child) -> bool {
    match c {
        Child::Box(n) => n.classes.is_empty() && n.style.is_empty(),
        Child::Text(_) => true,
    }
}

/// Per-child alignment widths: the id column over a blank-line-free group's
/// id-bearing boxes (so the bars line up); the type column too, but only when
/// the whole group is plain. Every other child gets zeros.
pub fn child_widths(children: &[Child], trivia: &[TriviaToken]) -> Vec<NodeWidths> {
    let mut out = vec![NodeWidths::default(); children.len()];
    for group in split_groups(children, trivia) {
        let plain = group.iter().all(|&i| is_plain(&children[i]));
        let mut w = NodeWidths::default();
        for &i in &group {
            if let Child::Box(n) = &children[i] {
                if let Some(id) = &n.id {
                    w.id = w.id.max(id.len());
                }
                if plain {
                    w.ty = w.ty.max(type_len(n));
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
