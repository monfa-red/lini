//! `lini desugar` (SPEC §14): expand the surface sugar — a box's id-as-label
//! into an explicit `[ "id" ]` text child, a wire's auto-distributed labels into
//! an explicit `along:` — into the form it stands for, then re-print. Types,
//! variables, and properties stay as written; comments are not preserved. A
//! teaching / debugging view, never a rewrite.

use crate::resolve::type_chain_contains;
use crate::syntax::ast::{Child, Decl, File, Node, TextNode, Value, Wire};

/// Expand the surface sugar across the whole file. The stylesheet is untouched.
pub fn desugar(file: &File) -> File {
    File {
        stylesheet: file.stylesheet.clone(),
        stylesheet_span: file.stylesheet_span,
        instances: file
            .instances
            .iter()
            .map(|c| desugar_child(c, file))
            .collect(),
        wires: file.wires.iter().map(desugar_wire).collect(),
    }
}

fn desugar_child(child: &Child, file: &File) -> Child {
    match child {
        Child::Box(n) => Child::Box(desugar_node(n, file)),
        Child::Text(t) => Child::Text(t.clone()),
    }
}

fn desugar_node(node: &Node, file: &File) -> Node {
    let ty = node.ty.as_deref().unwrap_or("box");

    let mut children: Vec<Child> = node
        .children
        .iter()
        .map(|c| desugar_child(c, file))
        .collect();

    // id-as-label (SPEC §3): a leaf box with no content of its own shows its id.
    // An `|icon|` consumes its text as the glyph name, and a container (group /
    // table / group-based define) holds its children — neither expands.
    let is_icon = type_chain_contains(ty, "icon", file);
    let is_container = type_chain_contains(ty, "group", file);
    if children.is_empty()
        && !is_icon
        && !is_container
        && let Some(id) = &node.id
    {
        children.push(Child::Text(TextNode {
            text: id.clone(),
            span: node.span,
        }));
    }

    Node {
        id: node.id.clone(),
        ty: node.ty.clone(),
        classes: node.classes.clone(),
        style: node.style.clone(),
        children,
        wires: node.wires.iter().map(desugar_wire).collect(),
        span: node.span,
    }
}

/// Make a wire's auto-distributed labels explicit: add an `along:` list of even
/// fractions when labels are present and no `along:` was written (SPEC §14).
fn desugar_wire(w: &Wire) -> Wire {
    let n = w.labels.len();
    let has_along = w.style.iter().any(|d| d.name == "along");
    if n == 0 || has_along {
        return w.clone();
    }
    let fractions: Vec<Value> = (0..n)
        .map(|i| {
            let f = (i as f64 + 1.0) / (n as f64 + 1.0);
            Value::Number((f * 100.0).round() / 100.0)
        })
        .collect();
    let mut style = w.style.clone();
    style.insert(
        0,
        Decl {
            name: "along".to_string(),
            groups: vec![fractions],
            span: w.span,
        },
    );
    Wire {
        chain: w.chain.clone(),
        op: w.op,
        classes: w.classes.clone(),
        style,
        labels: w.labels.clone(),
        span: w.span,
    }
}

#[cfg(test)]
mod tests {
    fn desugar(src: &str) -> String {
        crate::desugar_source(src).expect("desugar")
    }

    #[test]
    fn id_becomes_an_explicit_label() {
        assert_eq!(desugar("cat |box|\n"), "cat |box| [ \"cat\" ]\n");
    }

    #[test]
    fn an_explicit_label_is_left_alone() {
        assert_eq!(desugar("cat |box| \"Cat\"\n"), "cat |box| [ \"Cat\" ]\n");
    }

    #[test]
    fn icon_glyph_is_not_expanded() {
        assert_eq!(desugar("home |icon|\n"), "home |icon|\n");
    }

    #[test]
    fn a_container_keeps_its_children() {
        // A group holds its children; its id is not a label.
        let out = desugar("g |group| [\n  a |box|\n]\n");
        assert!(!out.contains("\"g\""), "{out}");
        assert!(out.contains("a |box|"), "{out}");
    }

    #[test]
    fn wire_labels_gain_an_explicit_along() {
        assert_eq!(
            desugar("a -> b \"near a\" \"near b\"\n"),
            "a -> b { along: 0.33 0.67; } \"near a\" \"near b\"\n"
        );
    }

    #[test]
    fn an_explicit_along_is_left_alone() {
        let out = desugar("a -> b { along: 0.2 } \"x\"\n");
        assert!(out.contains("along: 0.2;"), "{out}");
        assert_eq!(out.matches("along").count(), 1, "{out}");
    }
}
