//! `lini desugar` (SPEC §14): expand the surface sugar — a node's positional
//! labels into `|caption|` / `|text|` children, a wire's inline labels into
//! `|text|` children — into the explicit form they stand for, then re-print.
//! Types, variables, and properties stay as written; comments are not preserved.
//! A teaching / debugging view, never a rewrite.

use crate::resolve::type_chain_contains;
use crate::span::Span;
use crate::syntax::ast::{Block, Decl, File, Node, TextChild, Value, Wire, WireBlock};

/// Expand label/wire sugar across the whole file. The stylesheet is untouched —
/// only instances and wires carry the positional-label sugar.
pub fn desugar(file: &File) -> File {
    File {
        stylesheet: file.stylesheet.clone(),
        instances: file.instances.iter().map(|n| desugar_node(n, file)).collect(),
        wires: file.wires.iter().map(desugar_wire).collect(),
    }
}

fn desugar_node(node: &Node, file: &File) -> Node {
    let ty = node.ty.as_deref().unwrap_or("box");
    // A text-derived type (`|text|`, `|caption|`, …) keeps its label as content
    // and an `|icon|` keeps it as the glyph name (SPEC §7) — neither expands, so
    // a second pass is a no-op. Every other type's positional labels become
    // children.
    let consumes_label =
        type_chain_contains(ty, "text", file) || type_chain_contains(ty, "icon", file);
    let expand = !consumes_label && !node.labels.is_empty();

    let mut nodes = if expand {
        let is_group = type_chain_contains(ty, "group", file);
        label_sugar(&node.labels, is_group, node.span)
    } else {
        Vec::new()
    };
    if let Some(block) = &node.block {
        nodes.extend(block.nodes.iter().map(|c| desugar_node(c, file)));
    }

    let decls = node.block.as_ref().map(|b| b.decls.clone()).unwrap_or_default();
    let wires = node.block.as_ref().map(|b| b.wires.clone()).unwrap_or_default();
    let block = if decls.is_empty() && nodes.is_empty() && wires.is_empty() {
        node.block.clone() // preserve a `{}` vs. no block
    } else {
        Some(Block { decls, nodes, wires })
    };

    Node {
        id: node.id.clone(),
        ty: node.ty.clone(),
        labels: if expand { Vec::new() } else { node.labels.clone() },
        classes: node.classes.clone(),
        block,
        span: node.span,
    }
}

/// A host's positional labels as explicit child nodes (mirrors the resolver's
/// own expansion, SPEC §8): a group's 1st label is a top `|caption|`, its 2nd a
/// bottom `|caption|` (`side: bottom`), the rest plain `|text|`; every other
/// shape stacks all labels as `|text|`.
fn label_sugar(labels: &[String], is_group: bool, span: Span) -> Vec<Node> {
    labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let (ty, footer) = match (is_group, i) {
                (true, 0) => ("caption", false),
                (true, 1) => ("caption", true),
                _ => ("text", false),
            };
            let block = footer.then(|| Block {
                decls: vec![ident_decl("side", "bottom", span)],
                nodes: Vec::new(),
                wires: Vec::new(),
            });
            Node {
                id: None,
                ty: Some(ty.to_string()),
                labels: vec![label.clone()],
                classes: Vec::new(),
                block,
                span,
            }
        })
        .collect()
}

fn desugar_wire(w: &Wire) -> Wire {
    if w.labels.is_empty() {
        return w.clone();
    }
    let mut texts: Vec<TextChild> = w
        .labels
        .iter()
        .map(|label| TextChild {
            text: label.clone(),
            classes: Vec::new(),
            decls: Vec::new(),
            span: w.span,
        })
        .collect();
    let decls = if let Some(block) = &w.block {
        texts.extend(block.texts.iter().cloned());
        block.decls.clone()
    } else {
        Vec::new()
    };
    Wire {
        chain: w.chain.clone(),
        op: w.op,
        labels: Vec::new(),
        classes: w.classes.clone(),
        block: Some(WireBlock { decls, texts }),
        span: w.span,
    }
}

fn ident_decl(name: &str, value: &str, span: Span) -> Decl {
    Decl {
        name: name.to_string(),
        groups: vec![vec![Value::Ident(value.to_string())]],
        span,
    }
}

#[cfg(test)]
mod tests {
    fn desugar(src: &str) -> String {
        crate::desugar_source(src).expect("desugar")
    }

    #[test]
    fn rect_label_becomes_a_text_child() {
        assert_eq!(desugar("x |box| \"hi\"\n"), "x |box| {\n  |text| \"hi\"\n}\n");
    }

    #[test]
    fn group_labels_become_caption_and_footer() {
        assert_eq!(
            desugar("g |group| \"Head\" \"Foot\"\n"),
            "g |group| {\n  |caption| \"Head\"\n  |caption| \"Foot\" { side: bottom; }\n}\n"
        );
    }

    #[test]
    fn text_keeps_its_own_label() {
        assert_eq!(desugar("|text| \"raw\"\n"), "|text| \"raw\"\n");
    }

    #[test]
    fn icon_keeps_its_glyph_label() {
        assert_eq!(desugar("|icon| \"home\"\n"), "|icon| \"home\"\n");
    }

    #[test]
    fn user_define_over_group_gets_captions() {
        // panel::group derives from group, so its label is a caption.
        let out = desugar("panel::group { }\np |panel| \"Title\"\n");
        assert!(out.contains("|caption| \"Title\""), "{out}");
    }

    #[test]
    fn wire_labels_become_text_children() {
        assert_eq!(
            desugar("a -> b \"watches\"\n"),
            "a -> b {\n  |text| \"watches\"\n}\n"
        );
    }

    #[test]
    fn properties_and_existing_children_are_kept() {
        let out = desugar("g |group| \"Cap\" {\n  fill: red;\n  a |box| \"A\"\n}\n");
        assert!(out.contains("fill: red;"), "{out}");
        assert!(out.contains("|caption| \"Cap\""), "{out}");
        // the inner box's own label is expanded too
        assert!(out.contains("a |box| {"), "{out}");
    }
}
