//! Source-level desugaring: expand the sugar a reader can't see through —
//! positional labels and inline wire labels — into the explicit children they
//! stand for, while leaving types, variables, and attributes exactly as
//! written. The result round-trips through [`crate::fmt`], so `lini desugar`
//! shows the literal expansion of what you typed.
//!
//! This is purely syntactic; it does not resolve templates or bake variables.
//! Types are consulted only to classify each node — a `group` promotes its
//! first two labels to a caption and footer, a `|text|` keeps its label as its
//! own glyphs (SPEC §5/§9) — so the [`ShapesTable`] is the only input beyond
//! the AST.

use super::ShapeKind;
use super::expand_labels;
use super::shapes::ShapesTable;
use crate::ast::{BodyItem, File, ShapeInst, Stmt, TextDecl, WireDecl};

pub fn desugar_file(file: &File, shapes: &ShapesTable) -> File {
    File {
        defs: file.defs.clone(),
        stmts: file
            .stmts
            .iter()
            .map(|stmt| match stmt {
                Stmt::Node(inst) => Stmt::Node(desugar_inst(inst, shapes)),
                Stmt::Wire(wire) => Stmt::Wire(desugar_wire(wire)),
            })
            .collect(),
    }
}

fn desugar_inst(inst: &ShapeInst, shapes: &ShapesTable) -> ShapeInst {
    // Resolve the type only to classify it. An unknown type can't be classified
    // — leave its labels untouched and let full resolution report the error.
    let resolved = shapes.resolve(&inst.ty.name, inst.ty.span).ok();
    let is_text = resolved.as_ref().is_some_and(|r| r.kind == ShapeKind::Text);
    let is_group = resolved
        .as_ref()
        .is_some_and(|r| r.type_chain.iter().any(|t| t == "group"));

    let mut body: Vec<BodyItem> = Vec::new();
    // A closed shape's labels become text children; `|text|` keeps its own.
    if !is_text {
        body.extend(
            expand_labels(&inst.labels, is_group, inst.span)
                .into_iter()
                .map(BodyItem::Inst),
        );
    }
    if let Some(children) = &inst.body {
        body.extend(children.iter().map(|item| match item {
            BodyItem::Inst(child) => BodyItem::Inst(desugar_inst(child, shapes)),
            BodyItem::Wire(wire) => BodyItem::Wire(desugar_wire(wire)),
        }));
    }

    ShapeInst {
        id: inst.id.clone(),
        ty: inst.ty.clone(),
        labels: if is_text { inst.labels.clone() } else { Vec::new() },
        items: inst.items.clone(),
        body: (!body.is_empty()).then_some(body),
        span: inst.span,
    }
}

fn desugar_wire(wire: &WireDecl) -> WireDecl {
    // Inline labels become `|text|` children, prepended to any explicit body.
    let mut texts: Vec<TextDecl> = wire
        .labels
        .iter()
        .map(|label| TextDecl {
            text: label.clone(),
            items: Vec::new(),
            span: wire.span,
        })
        .collect();
    if let Some(body) = &wire.body {
        texts.extend(body.iter().cloned());
    }

    WireDecl {
        chain: wire.chain.clone(),
        op: wire.op,
        labels: Vec::new(),
        items: wire.items.clone(),
        body: (!texts.is_empty()).then_some(texts),
        span: wire.span,
    }
}
