//! AST-level lint pass. Emits warnings for stylistic smells that aren't
//! parse/resolve errors — most notably inline visual attrs that belong in a
//! `.style` def (SPEC section 16 visual-attr lint category).

use crate::ast::{AttrItem, BodyItem, DefsEntry, File, ShapeInst, Stmt, TypeRef, WireDecl};
use crate::error::Diagnostic;

/// Attrs that are purely visual — appearance only, not what's drawn or where.
/// Inline use outside a style def emits a warning (SPEC §16's lint category).
const VISUAL_ATTRS: &[&str] = &[
    "fill",
    "stroke",
    "color",
    "thickness",
    "line",
    "opacity",
    "radius",
    "double",
    "rotation",
    "shadow",
    "weight",
    "align",
    "variant",
    "font",
    "text-size",
];

/// Attr names that existed under another name in earlier revisions; using one
/// warns with the current name (SPEC §16 renamed-attr diagnostic).
fn renamed_attr_hint(name: &str) -> Option<&'static str> {
    match name {
        "stroke-style" => Some("line"),
        _ => None,
    }
}

fn lint_attr_name(name: &str, span: crate::span::Span, diags: &mut Vec<Diagnostic>) {
    if let Some(new) = renamed_attr_hint(name) {
        diags.push(Diagnostic::warn(
            span,
            format!("unknown attr '{}'; use '{}'", name, new),
        ));
    }
}

pub fn lint(file: &File) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for stmt in &file.stmts {
        match stmt {
            Stmt::Node(inst) => lint_inst(inst, &mut diags),
            Stmt::Wire(w) => lint_wire(w, &mut diags),
        }
    }
    // Shape defs in the defs block can contain bodies with primitives that
    // should follow scene rules.
    if let Some(defs) = &file.defs {
        for entry in &defs.entries {
            if let DefsEntry::ShapeDef(sd) = entry
                && let Some(body) = &sd.body
            {
                for item in body {
                    lint_body_item(item, &mut diags);
                }
            }
        }
    }
    diags
}

fn lint_body_item(item: &BodyItem, diags: &mut Vec<Diagnostic>) {
    match item {
        BodyItem::Inst(i) => lint_inst(i, diags),
        BodyItem::Wire(w) => lint_wire(w, diags),
    }
}

fn lint_inst(inst: &ShapeInst, diags: &mut Vec<Diagnostic>) {
    for item in &inst.items {
        if let AttrItem::Attr(a) = item {
            lint_attr_name(&a.name, a.span, diags);
            if is_visual(&a.name, &inst.ty) {
                diags.push(Diagnostic::warn(
                    a.span,
                    format!(
                        "visual attr '{}' inline; consider moving to a .style",
                        a.name
                    ),
                ));
            }
        }
    }
    if let Some(body) = &inst.body {
        for child in body {
            lint_body_item(child, diags);
        }
    }
}

fn lint_wire(wire: &WireDecl, diags: &mut Vec<Diagnostic>) {
    for item in &wire.items {
        if let AttrItem::Attr(a) = item {
            lint_attr_name(&a.name, a.span, diags);
            // Marker attrs are structural on wires; everything else in
            // VISUAL_ATTRS is style.
            if VISUAL_ATTRS.contains(&a.name.as_str()) {
                diags.push(Diagnostic::warn(
                    a.span,
                    format!(
                        "visual attr '{}' inline; consider moving to a .style",
                        a.name
                    ),
                ));
            }
        }
    }
}

fn is_visual(name: &str, _ty: &TypeRef) -> bool {
    VISUAL_ATTRS.contains(&name)
}
