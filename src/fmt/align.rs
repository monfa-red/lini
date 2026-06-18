//! Column alignment for sibling instances (SPEC §14): within a run of nodes
//! with no blank line between them, the id and `|type|` columns line up so the
//! labels start at the same offset. A blank line starts a fresh group.
//!
//! Aligning the anonymous string *cells* of a table into visual columns (the
//! flat table form) waits on the parser accepting multiple node statements per
//! line — until then each cell is its own line and needs no horizontal
//! alignment.

use super::trivia::{Trivia, TriviaToken};
use crate::syntax::ast::Node;

#[derive(Default, Clone, Copy)]
pub struct NodeWidths {
    /// Widest id in the group, 0 if none carry one.
    pub id: usize,
    /// Widest `|type|` (bars included) in the group, 0 if none carry one.
    pub ty: usize,
}

/// Per-node alignment widths: nodes sharing a blank-line-free group share the
/// group's max id / type widths.
pub fn node_widths(nodes: &[Node], trivia: &[TriviaToken]) -> Vec<NodeWidths> {
    let mut out = vec![NodeWidths::default(); nodes.len()];
    for group in split_groups(nodes, trivia) {
        let mut w = NodeWidths::default();
        for &i in &group {
            if let Some(id) = &nodes[i].id {
                w.id = w.id.max(id.len());
            }
            if let Some(ty) = &nodes[i].ty {
                w.ty = w.ty.max(ty.len() + 2); // |type|
            }
        }
        for i in group {
            out[i] = w;
        }
    }
    out
}

/// Index groups of consecutive nodes uninterrupted by a blank line.
fn split_groups(nodes: &[Node], trivia: &[TriviaToken]) -> Vec<Vec<usize>> {
    if nodes.is_empty() {
        return Vec::new();
    }
    let mut groups: Vec<Vec<usize>> = vec![vec![0]];
    for i in 1..nodes.len() {
        let prev_end = nodes[i - 1].span.end;
        let curr_start = nodes[i].span.start;
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
