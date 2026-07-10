//! Generated drawing-chrome nodes [SPEC 15.7]: the shared scaffolding every
//! chrome producer (a `revolve:`'s centerline, a `|page|`'s frame furniture)
//! lowers to — an anonymous, typed node carrying its `chrome:` marker, sitting
//! at the parent's tail so the body printer keeps it after the authored `[ ]`.

use crate::span::Span;
use crate::syntax::ast::{Decl, Node};

/// A generated chrome node: anonymous, of type `ty`, its `style` (the `chrome:`
/// marker plus any pin / extra), spanned at `tail` — the empty span at the
/// parent's end.
pub(super) fn node(ty: &str, style: Vec<Decl>, tail: Span) -> Node {
    Node {
        id: None,
        ty: Some(ty.into()),
        label: None,
        classes: Vec::new(),
        style,
        style_span: None,
        children: Vec::new(),
        links: Vec::new(),
        span: tail,
    }
}
