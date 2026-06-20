//! Column alignment for sibling boxes (SPEC §14): within a run of children with
//! no blank line between them, the id, `|type|`, and `.class` columns of the
//! *boxes* line up so their blocks start at the same offset. Bare-text children
//! carry none of these, so they take no alignment. A blank line starts a fresh
//! group.
//!
//! Aligning a table's bare-text *cells* into visual columns (the flat table
//! form) is the formatter's table pass (SPEC §8), separate from this.

use super::trivia::{Trivia, TriviaToken};
use crate::span::Span;
use crate::syntax::ast::{Child, Node};

#[derive(Default, Clone, Copy)]
pub struct NodeWidths {
    /// Widest id in the group, 0 if none carry one.
    pub id: usize,
    /// Widest `|type|` bars in the group, 0 if none carry a type.
    pub ty: usize,
    /// Widest `.class` chain in the group, 0 if none carry one.
    pub cls: usize,
}

/// The rendered width of a node's `|type|` bars, or 0 when it has no type.
fn type_len(n: &Node) -> usize {
    n.ty.as_ref().map_or(0, |t| 2 + t.len())
}

/// The rendered width of a node's `.class` chain (`.a.b`), or 0 when it has none.
fn class_len(n: &Node) -> usize {
    n.classes.iter().map(|c| 1 + c.len()).sum()
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
                w.ty = w.ty.max(type_len(n));
                w.cls = w.cls.max(class_len(n));
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
