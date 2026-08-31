//! Template table + define/template chain resolution at the AST level. Returns
//! base→derived name chains (primitive excluded — it is the `kind`); desugar turns
//! each chain name into a `.lini-<name>` class. Cycles, depth > 16, and shadowing a
//! built-in are errors.

use crate::error::{Code, Error};
use crate::resolve::NodeKind;
use crate::span::Span;
use crate::syntax::ast::{Define, File, StyleItem};
use std::collections::HashMap;

const MAX_INHERITANCE_DEPTH: usize = 16;

/// Built-in templates and their base type [SPEC 8]. Each is a bundle over a
/// primitive (or, for `table`, over `group`).
pub const TEMPLATES: &[(&str, &str)] = &[
    ("box", "block"),
    ("rect", "box"),
    ("group", "block"),
    ("caption", "block"),
    ("footnote", "caption"),
    ("badge", "block"),
    ("row", "block"),
    ("column", "block"),
    ("grid", "block"),
    ("table", "group"),
    // Table cells and the ER entity [SPEC 8]. `cell` is a `|block|` carrying the
    // cell inset (`padding`); `header` / `footer` build on it (so `|table| |cell|`
    // reaches them, not the caption); `entity` is a 2-column table.
    ("cell", "block"),
    ("header", "cell"),
    ("footer", "cell"),
    ("entity", "table"),
    ("sign", "icon"),
    // Charts [SPEC 14]: the two container layouts and the series / structural
    // types, each a bundle over |block|. `line` is absent — a chart line reuses the
    // |line| primitive (the chart layout branches on its `data:`/`fn:` vs `points:`).
    ("chart", "block"),
    ("pie", "block"),
    ("area", "block"),
    ("bars", "block"),
    ("dots", "block"),
    ("bubble", "block"),
    ("slice", "block"),
    ("axis", "block"),
    ("band", "block"),
    ("mark", "block"),
    // Sequences [SPEC 13]: the container layout and the frame / separator types,
    // each a bundle over |block|. Participants are ordinary boxes, so they need
    // no type here.
    ("sequence", "block"),
    ("loop", "block"),
    ("opt", "block"),
    ("alt", "block"),
    ("else", "block"),
    // Core cross-layout templates [SPEC 8]: the note card (sequence, drawing,
    // and plain diagrams alike) and the assembly balloon.
    ("note", "block"),
    ("balloon", "oval"),
    // Trees [SPEC 12]: the structural topic node — a compact framed card over
    // |block|; custom structural types derive from it (`|person::topic|`) — and
    // the |mindmap| preset over it: the node is the visible root topic; desugar
    // seats it in a generated `layout: tree; direction: bilateral` scope and
    // lowers the palette walk / depth ramp as rules ([`super::tree`]).
    ("topic", "block"),
    ("mindmap", "topic"),
    // Drawings [SPEC 15]: the container layout, the round feature, the
    // centerline chrome types, hidden interior geometry, and the revolve's
    // shoulder-line chrome (|sketch| is a primitive, not a template).
    ("drawing", "block"),
    ("hole", "oval"),
    ("centerline", "line"),
    ("pitch-circle", "oval"),
    ("breakline", "line"),
    ("hidden", "sketch"),
    ("shoulder", "line"),
    ("threadline", "line"),
    // The crossing-halo knockouts [SPEC 15.7] — generated mask cuts, no
    // instances; registered so the `|halo|` chrome rule runs the cascade.
    ("halo", "line"),
    // Drafting symbols [SPEC 15.9], drawing-scope only: the ISO 1302
    // surface-texture symbol (label = the textual indication, `symbol:` the
    // vee variant), the GD&T frame with its `|control|` rows, and the framed
    // datum letter as a node.
    ("surface-finish", "block"),
    ("feature-control", "block"),
    ("control", "block"),
    ("datum", "block"),
    // Sections & details [SPEC 15.8]: the authored plane line (chrome — its ISO
    // anatomy fills from the view's extent) and the magnifier region marker. A
    // section or detail **view** is a plain `|drawing| { of: <marker> }` — no
    // dedicated type; the marker's kind decides whether it re-renders.
    ("plane", "line"),
    ("magnifier", "oval"),
    // A sheet's projection construction line [SPEC 15.8] — generated at layout
    // from a cross-view link, or authored freely; chrome like the centerlines.
    ("projection", "line"),
    // The sheet [SPEC 15.8]: the ISO page container, its seated title block,
    // and the generated furniture types (frame / zone references / ticks).
    ("page", "block"),
    ("title-block", "table"),
    ("field", "block"),
    ("frame", "rect"),
    ("zone", "block"),
    ("tick", "line"),
    // The datum layout [SPEC 12]: a frameless container whose children put
    // their origin on its datum instead of flowing — the placement core the
    // drawing family is built from.
    ("stack", "block"),
    // Floorplans [SPEC 15.11]: the architectural dialect of the drawing engine
    // — the scope (a `|drawing|`, so `|drawing|`-scoped rules dress it too),
    // the wall run and its thinner interior define, the two openings that ride
    // a wall's `[ ]`, and the six symbol-bodied fixtures. Legal only in a
    // `layout: floorplan` ([`crate::layout::floorplan`] is the gate).
    ("floorplan", "drawing"),
    ("wall", "sketch"),
    ("partition", "wall"),
    ("door", "block"),
    ("window", "block"),
    // …and the chrome an opening generates [SPEC 15.7/15.11] — the leaf (a
    // sliding door's panels too), its quarter swing arc, and a window's sill
    // pair. `|line|`-based like every other chrome type; the arc flips to a
    // `|path|` where it fills, the round-thread play.
    ("door-leaf", "line"),
    ("door-swing", "line"),
    ("window-sill", "line"),
    // …and the chrome a `|stairs|` generates from its `steps:` — the risers
    // across the flight and the up arrow (a `|path|` where it turns a head).
    ("stair-tread", "line"),
    ("stair-arrow", "line"),
    ("bed", "block"),
    ("sofa", "block"),
    ("dining", "block"),
    ("bath", "block"),
    ("appliance", "block"),
    ("stairs", "block"),
    // Schematics [SPEC 16]: the scope, the pin-bearing part and its terminal,
    // the net tag and its built-in defines, the junction dot, the connector /
    // amplifier presets, and the discrete family (the type is the ref family,
    // [SPEC 16.3]). All protected from define shadowing via this table
    // [SPEC 23]. `|schematic|` is `|block|` + `layout: schematic` — the scope
    // the engine dispatches on ([`crate::layout::schematic`]).
    ("schematic", "block"),
    ("component", "block"),
    ("pin", "block"),
    ("label", "block"),
    ("junction", "oval"),
    ("J", "component"),
    ("opamp", "component"),
    ("gnd", "label"),
    ("nc", "label"),
    ("R", "block"),
    ("C", "block"),
    ("L", "block"),
    ("D", "block"),
    ("LED", "block"),
    ("Q", "block"),
    ("Y", "block"),
    ("F", "block"),
    ("FB", "block"),
    ("SW", "block"),
    ("BT", "block"),
    ("V", "block"),
    ("I", "block"),
];

/// The discrete two/three-terminal part types [SPEC 16.3] — the type name is
/// the ref family. One list; the symbol/pin tables in
/// [`crate::desugar::schematic`] and the validation role key off it.
pub const DISCRETES: &[&str] = &[
    "R", "C", "L", "D", "LED", "Q", "Y", "F", "FB", "SW", "BT", "V", "I",
];

/// The wall **openings** [SPEC 15.11] — the two types that ride a `|wall|`'s
/// `[ ]`, stationed on one of its segments. One list; the vocabulary gate, the
/// `at:` / `on:` owner columns and the validation role key off it.
pub const OPENINGS: &[&str] = &["door", "window"];

/// The floorplan **fixtures** [SPEC 15.11] — the six symbol-bodied furniture
/// families, true-size and stretched to their resolved box. One list, read the
/// way [`DISCRETES`] is; `|stairs|` takes no `symbol:` (it generates from
/// `steps:`), which is the one thing its readers split on.
pub const FIXTURES: &[&str] = &["bed", "sofa", "dining", "bath", "appliance", "stairs"];

pub fn is_template(name: &str) -> bool {
    TEMPLATES.iter().any(|(n, _)| *n == name)
}

/// The base a built-in template builds on; `None` for a primitive or non-template.
pub fn template_base(name: &str) -> Option<&'static str> {
    TEMPLATES.iter().find(|(n, _)| *n == name).map(|(_, b)| *b)
}

/// The built-in **type chain** a template name wears — the name itself and each
/// base above it, **derived → base**, stopping before the primitive (that is
/// the node's `kind`, never a type; [`template_base`] of the last entry names
/// it). Empty for a primitive or a name the table does not know.
///
/// The one walk of [`TEMPLATES`]: the type-chain reading available before
/// [`Types`] exists (the raw AST, a root engine's worn classes) and wherever
/// only a name is in hand. A user define over a template is caught downstream,
/// where the resolved chain is known.
pub fn template_chain(name: &str) -> Vec<&'static str> {
    let mut out = Vec::new();
    let mut cur = name;
    while let Some((ty, base)) = TEMPLATES.iter().find(|(n, _)| *n == cur) {
        out.push(*ty);
        if crate::resolve::NodeKind::parse(base).is_some() {
            break;
        }
        cur = base;
    }
    out
}

/// Whether a written type name **is, or builds on,** the template `base`.
pub fn derives_from(name: &str, base: &str) -> bool {
    name == base || template_chain(name).contains(&base)
}

/// A define may not take the name of a primitive, a template, the `link` rule
/// target, or a structural SVG class [SPEC 23] — once the `shape` infix is gone,
/// a `|node::box|` define's `.lini-node` would collide with the universal marker.
fn is_builtin_type(name: &str) -> bool {
    NodeKind::parse(name).is_some()
        || is_template(name)
        || matches!(
            name,
            "link" | "node" | "text" | "marker" | "canvas" | "scene" | "cut"
        )
}

/// A resolved type: its primitive kind and the template/define names walked
/// base→derived (the primitive is excluded — it is `kind`).
pub struct TypeInfo {
    pub kind: NodeKind,
    pub chain: Vec<String>,
}

/// The type table: user defines (name → base), validated for cycles, depth, and
/// shadowing on construction.
pub struct Types {
    defines: HashMap<String, String>,
}

impl Types {
    pub fn build(file: &File) -> Result<Self, Error> {
        let mut defines = HashMap::new();
        for d in file.stylesheet.iter().filter_map(as_define) {
            if is_builtin_type(&d.name) {
                return Err(
                    Error::at(d.span, format!("'{}' shadows a built-in type", d.name))
                        .code(Code::SHADOWS_BUILTIN),
                );
            }
            if defines.insert(d.name.clone(), d.base.clone()).is_some() {
                return Err(Error::at(d.span, format!("duplicate type '{}'", d.name)));
            }
        }
        let types = Self { defines };
        // Validate every define's chain up front, so a cycle or over-deep
        // inheritance is reported even for a type no instance uses.
        for d in file.stylesheet.iter().filter_map(as_define) {
            types.walk(&d.name, d.span, &mut Vec::new(), 0)?;
        }
        Ok(types)
    }

    pub fn is_known(&self, name: &str) -> bool {
        NodeKind::parse(name).is_some() || is_template(name) || self.defines.contains_key(name)
    }

    pub fn resolve(&self, name: &str, span: Span) -> Result<TypeInfo, Error> {
        self.walk(name, span, &mut Vec::new(), 0)
    }

    /// Walk a type to its primitive base, accumulating the chain base→derived.
    /// `visiting` carries the chain for cycle detection; `depth` bounds it.
    fn walk(
        &self,
        name: &str,
        span: Span,
        visiting: &mut Vec<String>,
        depth: usize,
    ) -> Result<TypeInfo, Error> {
        if depth > MAX_INHERITANCE_DEPTH {
            return Err(Error::at(
                span,
                format!(
                    "'{}' exceeds max inheritance depth ({})",
                    name, MAX_INHERITANCE_DEPTH
                ),
            )
            .code(Code::INHERIT_DEPTH));
        }
        if visiting.iter().any(|n| n == name) {
            return Err(Error::at(
                span,
                format!("cycle in '{} → {}'", visiting.join(" → "), name),
            )
            .code(Code::INHERIT_CYCLE));
        }
        if let Some(kind) = NodeKind::parse(name) {
            return Ok(TypeInfo {
                kind,
                chain: Vec::new(),
            });
        }
        let base = template_base(name)
            .map(str::to_string)
            .or_else(|| self.defines.get(name).cloned())
            .ok_or_else(|| {
                Error::at(span, format!("unknown type '{}'", name)).code(Code::UNKNOWN_TYPE)
            })?;
        visiting.push(name.to_string());
        let mut info = self.walk(&base, span, visiting, depth + 1)?;
        visiting.pop();
        info.chain.push(name.to_string());
        Ok(info)
    }
}

fn as_define(it: &StyleItem) -> Option<&Define> {
    match it {
        StyleItem::Define(d) => Some(d),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> File {
        crate::syntax::parser::parse(src, &crate::lexer::lex(src).expect("lex")).expect("parse")
    }
    fn chain(src: &str, name: &str) -> Vec<String> {
        let file = parse(src);
        let t = Types::build(&file).expect("build");
        t.resolve(name, Span::empty()).expect("resolve").chain
    }
    fn build_err(src: &str) -> String {
        let file = parse(src);
        Types::build(&file)
            .err()
            .map(|e| e.message)
            .unwrap_or_default()
    }

    #[test]
    fn primitive_has_empty_chain() {
        assert!(chain("", "block").is_empty());
        // box is now a template over the bare block primitive.
        assert_eq!(chain("", "box"), vec!["box"]);
    }

    #[test]
    fn table_chain_is_group_then_table() {
        assert_eq!(chain("", "table"), vec!["group", "table"]);
    }

    #[test]
    fn user_define_appends_after_its_base_chain() {
        assert_eq!(
            chain("{ |panel::group| { } }\n", "panel"),
            vec!["group", "panel"]
        );
    }

    /// The depth bound [SPEC 3]: the deepest chain the builder accepts still
    /// builds, and one link further is refused by name with the limit spelled
    /// out. Without the bound a pathological chain is only slow, so nothing
    /// else in the suite would ever notice it.
    #[test]
    fn a_define_chain_bottoms_out_at_the_max_inheritance_depth() {
        // `|t0::box|` already sits two hops over the `block` primitive
        // (`box` is itself a template), so `n` user defines walk n + 2 deep.
        let chain_of = |n: usize| {
            let mut src = String::from("{\n  |t0::box| { }\n");
            for i in 1..n {
                src.push_str(&format!("  |t{i}::t{}| {{ }}\n", i - 1));
            }
            src.push_str("}\n");
            src
        };
        let deepest = MAX_INHERITANCE_DEPTH - 1;
        assert_eq!(
            chain(&chain_of(deepest), &format!("t{}", deepest - 1)).len(),
            deepest + 1,
            "box plus every define in the chain"
        );
        let e = build_err(&chain_of(deepest + 1));
        assert!(
            e.contains(&format!(
                "exceeds max inheritance depth ({MAX_INHERITANCE_DEPTH})"
            )),
            "the limit is named: {e}"
        );
    }

    #[test]
    fn cycle_and_shadow_error() {
        // The cycle spells its chain with `→`, the one arrow every cycle
        // message uses [SPEC 21] (the theme's define cycle prints it too).
        assert_eq!(
            build_err("{ |a::b| { }\n|b::a| { } }\n"),
            "cycle in 'a → b → a'"
        );
        assert!(build_err("{ |rect::oval| { } }\n").contains("shadows a built-in"));
    }
}
