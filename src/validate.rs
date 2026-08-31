//! The owner-aware property validation pass [SPEC 17/21], reading the ledger.
//! Strict where the wearer is statically known, lenient where a class is
//! polymorphic:
//!
//! - an **unknown property name** is an error, everywhere — the message
//!   suggests the nearest name;
//! - a known property **misused where its wearer is statically known** (an
//!   instance's own block, an element / id / descendant rule's tail, the root
//!   block) is an error with a contextual correction;
//! - in a **class rule** a property is inert on wearers that can't use it — it
//!   warns only when it is dead for *every* wearer, and a defined class no one
//!   wears warns too;
//! - a **malformed value** the ledger shape can judge statically (arity,
//!   range) is an error, wearer-independent;
//! - a row the ledger marks **deferred** — named in the language, reader not
//!   built ([SPEC 24]) — is an error wherever it is written, so accepting it
//!   silently can never freeze the non-behaviour.
//!
//! The pass runs on the parsed file, before desugar, so it sees exactly what
//! the user wrote; the handful of attr names desugar/layout generate
//! internally are whitelisted for the lowered-form round-trip.

use crate::desugar::types::{self, Types};
use crate::error::{Code, Diagnostic};
use crate::ledger::properties::{self, Gate, Inherit, Kind, Owner, Property, Shape};
use crate::span::Span;
use crate::suggest;
use crate::syntax::ast::{
    Child, Decl, Define, File, LabelItem, Link, Node, Rule, SelUnit, StyleItem, TextNode, Value,
    layout_of, root_ident,
};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

/// Attr names desugar/layout write internally (view chrome, detail clips, the
/// sourced-view title, mate seating, title-block field markers) — never user
/// properties, but present when a lowered file (`lini desugar` output) is
/// compiled back.
const INTERNAL: &[&str] = &[
    "chrome",
    "clip",
    "of-title",
    "mount",
    crate::desugar::scale::PX_PER_UNIT,
    crate::desugar::scale::WALL_THICKNESS,
    crate::desugar::scale::UNIT_MM,
    "field",
    "font-scale",
];

/// `density:` is **pixels per millimetre** [SPEC 15.1], so under `unit: px`
/// there are no millimetres for it to convert. `1` is the identity and agrees
/// with what pixel space already means; any other value is silently doing
/// nothing, which is worth saying — a `density: 4` copied over from a drawing
/// looks like it scales the artwork and does not.
fn check_density_unit(file: &File, out: &mut Vec<Diagnostic>) {
    let root_decl = |name: &str| {
        file.stylesheet.iter().find_map(|i| match i {
            StyleItem::RootDecl(d) if d.name == name => Some(d),
            _ => None,
        })
    };
    // Pixel space is reached two ways: stated, or taken as a plain `stack`'s
    // default [SPEC 12]. The warning has to cover both — the copied-over
    // `density:` this exists to catch is likeliest where nothing was stated.
    let pixels = match root_decl("unit") {
        Some(u) => matches!(u.single(), Some(Value::Ident(u)) if u == "px"),
        None => file.stylesheet.iter().any(|i| match i {
            StyleItem::RootDecl(d) if d.name == "layout" => {
                matches!(d.single(), Some(Value::Ident(l))
                    if crate::resolve::is_stack_layout(l)
                        && !crate::resolve::is_drawing_layout(l))
            }
            _ => false,
        }),
    };
    if !pixels {
        return;
    }
    let Some(d) = root_decl("density") else {
        return;
    };
    if matches!(d.single(), Some(Value::Number(n)) if *n == 1.0) {
        return;
    }
    out.push(
        Diagnostic::warn(
            d.span,
            "'density' is pixels per millimetre — 'unit: px' has none; drop it, or state 'unit: mm' to scale",
        )
        .code(Code::DENSITY_WITHOUT_MM),
    );
}

pub fn validate(file: &File) -> Vec<Diagnostic> {
    // A broken type table (cycle, shadowing) is desugar's error to report.
    let Ok(types) = Types::build(file) else {
        return Vec::new();
    };
    let ctx = Ctx::new(file, &types);
    let mut out = Vec::new();

    // The stylesheet: root config, rules, define bodies.
    for item in &file.stylesheet {
        match item {
            StyleItem::RootDecl(d) => ctx.check_decl(d, &Wearer::Root, &mut out),
            StyleItem::Rule(r) => ctx.check_rule(r, &mut out),
            StyleItem::Define(d) => ctx.check_define(d, &mut out),
            StyleItem::Var(_) | StyleItem::Binding(_) => {}
        }
    }
    ctx.check_unworn_classes(file, &mut out);
    check_density_unit(file, &mut out);

    // The canvas: every instance block, text style, and link block, with the
    // parent's statically-known layout as context.
    let root_layout = ctx.root_layout.clone();
    for c in &file.instances {
        ctx.check_child(c, Some(root_layout.as_str()), &mut out);
    }
    for w in &file.links {
        ctx.check_link(w, Some(root_layout.as_str()), &mut out);
    }
    out
}

/// What a declaration is written on — decides which owners satisfy it.
enum Wearer<'a> {
    /// The scene root (its `layout:` is always statically known).
    Root,
    /// A node with a resolved type: primitive kind name, template/define chain
    /// (base→derived), its own static layout, and the parent's static layout.
    Node {
        /// The written type name, for messages (`'|box|'`).
        shown: &'a str,
        kind: &'a str,
        chain: &'a [String],
        own_layout: Option<&'a str>,
        parent_layout: Option<&'a str>,
    },
    /// A link (`|-|` / `(-)` rules, a link's own block) — polymorphic between
    /// wires, dimensions, and mates, so the owner check is the link's own set.
    Link,
    /// A bare text leaf — resolve enforces text validity with its own message;
    /// beyond name/value checks only the text-specific gates apply here.
    Text,
    /// A wearer that isn't statically known (a class or id rule's tail) — the
    /// wearer-independent name and value checks only [SPEC 17].
    Unknown,
}

struct Ctx<'a> {
    types: &'a Types,
    /// Define name → its own style decls, for chain-walking static layouts.
    define_styles: HashMap<&'a str, &'a [Decl]>,
    /// Whether any stylesheet rule sets `layout:` — if so, a node's layout can
    /// come from a class/id/element rule and is never statically known.
    rules_set_layout: bool,
    root_layout: String,
}

impl<'a> Ctx<'a> {
    fn new(file: &'a File, types: &'a Types) -> Self {
        let define_styles = file
            .stylesheet
            .iter()
            .filter_map(|it| match it {
                StyleItem::Define(d) => Some((d.name.as_str(), d.style.as_slice())),
                _ => None,
            })
            .collect();
        let rules_set_layout = file.stylesheet.iter().any(
            |it| matches!(it, StyleItem::Rule(r) if r.decls.iter().any(|d| d.name == "layout")),
        );
        let root_layout = root_ident(&file.stylesheet, "layout")
            .unwrap_or("flow")
            .to_string();
        Self {
            types,
            define_styles,
            rules_set_layout,
            root_layout,
        }
    }

    // ── The canvas walk ──

    fn check_child(&self, child: &Child, parent_layout: Option<&str>, out: &mut Vec<Diagnostic>) {
        match child {
            Child::Text(t) => self.check_text(t, out),
            Child::Box(n) => self.check_node(n, parent_layout, out),
        }
    }

    fn check_node(&self, n: &Node, parent_layout: Option<&str>, out: &mut Vec<Diagnostic>) {
        let ty = n.ty.as_deref().unwrap_or("box");
        let info = self.types.resolve(ty, n.span).ok();
        let own_layout = self.static_layout(n, info.as_ref());
        if let Some(info) = &info {
            // A lowered file (`lini desugar` output) carries its type chain as
            // worn `.lini-*` classes — fold them in, so the round-trip
            // validates like the sugar it came from.
            let chain = with_worn_types(&info.chain, &n.classes);
            let wearer = Wearer::Node {
                shown: ty,
                kind: info.kind.as_str(),
                chain: &chain,
                own_layout: own_layout.as_deref(),
                parent_layout,
            };
            for d in &n.style {
                self.check_decl(d, &wearer, out);
            }
        }
        if let Some(label) = &n.label {
            self.check_text(label, out);
        }
        // A sequence frame's `[ ]` opens no scope [SPEC 13]: its notes and messages are
        // the sequence's own, so they keep its layout as their placement context.
        let frame_body = parent_layout == Some("sequence")
            && info
                .as_ref()
                .is_some_and(|i| crate::layout::sequence::is_frame(&i.chain));
        let body_layout = if frame_body {
            parent_layout
        } else {
            own_layout.as_deref()
        };
        for c in &n.children {
            self.check_child(c, body_layout, out);
        }
        for w in &n.links {
            self.check_link(w, body_layout, out);
        }
    }

    fn check_text(&self, t: &TextNode, out: &mut Vec<Diagnostic>) {
        for d in &t.style {
            self.check_decl(d, &Wearer::Text, out);
        }
    }

    fn check_link(&self, w: &Link, parent_layout: Option<&str>, out: &mut Vec<Diagnostic>) {
        for d in &w.style {
            self.check_decl(d, &Wearer::Link, out);
        }
        for item in &w.labels {
            match item {
                LabelItem::Text(t) => self.check_text(t, out),
                // A carried annotation node [SPEC 15.9] validates like a child
                // of the link's scope.
                LabelItem::Node(n) => self.check_node(n, parent_layout, out),
            }
        }
    }

    // ── The stylesheet walk ──

    fn check_rule(&self, r: &Rule, out: &mut Vec<Diagnostic>) {
        let wearer = match r.selector.units.last() {
            Some(SelUnit::Type { name, .. }) => match self.types.resolve(name, r.span).ok() {
                Some(info) => Some((info.kind.as_str().to_string(), info.chain)),
                None => None, // unknown type — desugar's error
            },
            Some(SelUnit::Link | SelUnit::Dimension) => {
                for d in &r.decls {
                    self.check_decl(d, &Wearer::Link, out);
                }
                return;
            }
            // A class rule is judged wearer-set-wide in `check_unworn_classes`;
            // an id rule's node is checked where it is declared (the instance
            // block) — here both get the wearer-independent checks.
            Some(SelUnit::Class(_) | SelUnit::Id(_)) | None => None,
        };
        let shown = match r.selector.units.last() {
            Some(SelUnit::Type { name, .. }) => name.as_str(),
            _ => "",
        };
        match wearer {
            Some((kind, chain)) => {
                let wearer = Wearer::Node {
                    shown,
                    kind: &kind,
                    chain: &chain,
                    own_layout: None,
                    parent_layout: None,
                };
                for d in &r.decls {
                    self.check_decl(d, &wearer, out);
                }
            }
            None => {
                for d in &r.decls {
                    self.check_decl(d, &Wearer::Unknown, out); // name + value checks only
                }
            }
        }
    }

    fn check_define(&self, def: &Define, out: &mut Vec<Diagnostic>) {
        if let Ok(info) = self.types.resolve(&def.name, def.span) {
            let wearer = Wearer::Node {
                shown: &def.name,
                kind: info.kind.as_str(),
                chain: &info.chain,
                own_layout: None,
                parent_layout: None,
            };
            for d in &def.style {
                self.check_decl(d, &wearer, out);
            }
        }
        for c in &def.children {
            self.check_child(c, None, out);
        }
        for w in &def.links {
            self.check_link(w, None, out);
        }
    }

    // ── One declaration ──

    fn check_decl(&self, d: &Decl, wearer: &Wearer, out: &mut Vec<Diagnostic>) {
        if INTERNAL.contains(&d.name.as_str()) {
            return;
        }
        let Some(prop) = properties::get(&d.name) else {
            let near = suggest::nearest(&d.name, properties::PROPERTIES.iter().map(|p| p.name), 1);
            let mut diag = Diagnostic::error(
                d.span,
                format!(
                    "unknown property '{}'{}",
                    d.name,
                    suggest::did_you_mean(&near)
                ),
            )
            .code(Code::UNKNOWN_PROPERTY);
            // The name is a machine-applicable replacement — it heads the decl,
            // so its span is derivable [ROADMAP 3.8].
            if let [best] = near.as_slice() {
                let name_span = Span::new(d.span.start, d.span.start + d.name.len());
                diag = diag.suggest(name_span, *best);
            }
            out.push(diag);
            return;
        };
        // A row the language names but has not built [SPEC 24]: accepting it
        // silently would freeze the non-behaviour, so it is an error until its
        // reader lands — the deferred feature stays a free option.
        if prop.deferred {
            out.push(
                Diagnostic::error(
                    d.span,
                    format!("'{}' is named but not built yet — see SPEC 24", d.name),
                )
                .code(Code::DEFERRED_PROPERTY),
            );
            return;
        }
        self.check_value(d, prop, wearer, out);
        let Wearer::Node {
            shown,
            kind,
            chain,
            own_layout,
            parent_layout,
        } = wearer
        else {
            match wearer {
                Wearer::Root => self.check_root_decl(d, prop, out),
                // A link's own block / a `|-|` / `(-)` rule wears the link's
                // owner set — the same reading `check_unworn_classes` gives a
                // class's link side [SPEC 17].
                Wearer::Link if !link_accepts(prop) => out.push(
                    Diagnostic::error(d.span, misuse_message(&d.name, "a link", prop))
                        .code(Code::MISUSED_PROPERTY),
                ),
                _ => {}
            }
            return;
        };
        if !node_accepts(prop, kind, chain, *own_layout) {
            out.push(
                Diagnostic::error(d.span, misuse_message(&d.name, shown, prop))
                    .code(Code::MISUSED_PROPERTY),
            );
            return;
        }
        // `wavy` is link-only by design [SPEC 17] — a wire waves, an outline
        // never does. A value check, wearer-independent; not a context gate.
        // (The async sequence message's wavy |line| is engine-lowered at
        // layout, never authored, so it never passes here.)
        if d.name == "stroke-style" && matches!(d.single(), Some(Value::Ident(s)) if s == "wavy") {
            out.push(
                Diagnostic::error(
                    d.span,
                    "'wavy' waves a link's wire — a shape's outline takes solid, dashed, dotted, center, or phantom",
                )
                .code(Code::WAVY_OUTLINE),
            );
        }
        // `radius` rounds a rect's corners and a polyline's joins [SPEC 17];
        // rounding the other primitives is deferred [SPEC 24], so a value that
        // would be silently dropped errors instead.
        if d.name == "radius" && matches!(*kind, "hex" | "slant" | "diamond" | "poly") {
            out.push(
                Diagnostic::error(
                    d.span,
                    format!(
                        "'radius' rounds a rect or a polyline join — rounding a '|{shown}|' is deferred"
                    ),
                )
                .code(Code::MISUSED_PROPERTY),
            );
        }
        // Layout-owned placement props hard-error out of context only where the
        // ledger marks a hard gate [SPEC 17, decision 10] — otherwise inert. The
        // statically-judgeable gates read the known container context here;
        // `tol`/`project` gate later, at drawing layout.
        if !matches!(prop.gate, Gate::Hard) {
            return;
        }
        match d.name.as_str() {
            "cell" | "span" => {
                // The one-time migration pointer [SPEC 14.5]: a band's extent
                // moved to `range:` (the axis's interval shape).
                if d.name == "span" && chain.iter().any(|c| c == "band") {
                    out.push(
                        Diagnostic::error(
                            d.span,
                            "a band's extent is 'range: a b' — 'span' places a grid child",
                        )
                        .code(Code::MISUSED_PROPERTY),
                    );
                    return;
                }
                // A placement prop's legal hosts are its ledger `Layout` owners
                // — `cell:` places on a grid *and* on a schematic's own ordinal
                // track grid [SPEC 16.1], where it also promotes a satellite to
                // an anchor; `span:` stays grid-only, schematic tracks have no
                // spans.
                if let Some(parent) = parent_layout
                    && !prop.layout_owners().any(|l| l == *parent)
                {
                    let verb = if d.name == "cell" {
                        "places a grid or schematic child"
                    } else {
                        "spans grid tracks"
                    };
                    out.push(
                        Diagnostic::error(
                            d.span,
                            format!(
                                "'{}' {verb} — this box sits in a 'layout: {parent}'",
                                d.name
                            ),
                        )
                        .code(Code::OFF_GRID_PLACEMENT),
                    );
                }
            }
            "place" => {
                if let Some(parent) = parent_layout
                    && *parent != "sequence"
                {
                    out.push(
                        Diagnostic::error(d.span, "'place' is valid only in a 'layout: sequence'")
                            .code(Code::PLACE_OUTSIDE_SEQUENCE),
                    );
                }
            }
            "activation" => {
                if let Some(own) = own_layout
                    && *own != "sequence"
                {
                    out.push(
                        Diagnostic::error(
                            d.span,
                            "'activation' is valid only in a 'layout: sequence'",
                        )
                        .code(Code::ACTIVATION_OUTSIDE_SEQUENCE),
                    );
                }
            }
            _ => {}
        }
    }

    /// Root-block misuse: the root accepts scene config (universal, root,
    /// layout-owned for its own layout) — never a type-/role-owned property.
    fn check_root_decl(&self, d: &Decl, prop: &Property, out: &mut Vec<Diagnostic>) {
        if !root_reads(&self.root_layout, prop) {
            out.push(
                Diagnostic::error(d.span, misuse_message(&d.name, "the root block", prop))
                    .code(Code::MISUSED_PROPERTY),
            );
        }
    }

    // ── Value shapes the ledger can judge statically [SPEC 21] ──

    fn check_value(&self, d: &Decl, prop: &Property, wearer: &Wearer, out: &mut Vec<Diagnostic>) {
        if matches!(prop.shape, Shape::One(_)) && d.groups.len() > 1 {
            // Per-datum paint [SPEC 14.6]: `fill`/`stroke`/`opacity` read comma
            // lists on a repeated-mark series (the series reader validates the
            // count); a one-shape series (`|line|` / `|area|` in a chart) gets
            // the pointed message, everything else the general shape error.
            let paint = matches!(d.name.as_str(), "fill" | "stroke" | "opacity");
            if let (
                true,
                Wearer::Node {
                    chain,
                    kind,
                    parent_layout,
                    ..
                },
            ) = (paint, wearer)
            {
                if chain.iter().any(|c| c == "bars" || c == "dots") {
                    return;
                }
                if *parent_layout == Some("chart")
                    && (*kind == "line" || chain.iter().any(|c| c == "line" || c == "area"))
                {
                    let shape = if chain.iter().any(|c| c == "area") {
                        "area"
                    } else {
                        "line"
                    };
                    out.push(
                        Diagnostic::error(d.span, crate::ledger::format::one_shape_paint(shape))
                            .code(Code::MALFORMED_VALUE),
                    );
                    return;
                }
            }
            out.push(
                Diagnostic::error(
                    d.span,
                    format!("'{}' takes one value, not a comma list", d.name),
                )
                .code(Code::MALFORMED_VALUE),
            );
        }
        match d.name.as_str() {
            "opacity" => {
                if let Some(Value::Number(n)) = d.single()
                    && !(0.0..=1.0).contains(n)
                {
                    out.push(
                        Diagnostic::error(d.span, "'opacity' is a fraction 0..1")
                            .code(Code::MALFORMED_VALUE),
                    );
                }
            }
            "translate" => {
                // `translate: x y` — flag a bare scalar or a longer run; a
                // single `(…)` group may fold to a point, so it passes.
                let bad = match d.groups.first().map(Vec::as_slice) {
                    Some([Value::Number(_)]) => true,
                    Some(g) if g.len() > 2 => true,
                    _ => false,
                };
                if bad {
                    out.push(
                        Diagnostic::error(d.span, "'translate' takes 'x y'")
                            .code(Code::MALFORMED_VALUE),
                    );
                }
            }
            // A connector's generated pin count and a pin's number [SPEC 16.2]
            // — counts, judged here like any other value shape.
            "pins" | "number" => {
                let count = match d.single() {
                    Some(Value::Number(n)) => Some(*n),
                    _ => None,
                };
                let pins = d.name == "pins";
                let ok = count.is_some_and(|n| n.fract() == 0.0 && (!pins || n >= 1.0));
                if !ok {
                    let msg = if pins {
                        "'pins' takes a count ≥ 1"
                    } else {
                        "'number' takes an integer"
                    };
                    out.push(Diagnostic::error(d.span, msg).code(Code::MALFORMED_VALUE));
                }
            }
            // A wall's poché depth [SPEC 15.11/17] — drawing units, > 0.
            "thickness" => {
                let ok = matches!(d.single(), Some(Value::Number(n)) if *n > 0.0);
                if !ok {
                    out.push(
                        Diagnostic::error(d.span, "'thickness' takes a number > 0")
                            .code(Code::MALFORMED_VALUE),
                    );
                }
            }
            // A flight's tread count [SPEC 15.11] — an integer ≥ 2; one step
            // is a threshold, not a stair.
            "steps" => {
                let ok =
                    matches!(d.single(), Some(Value::Number(n)) if n.fract() == 0.0 && *n >= 2.0);
                if !ok {
                    out.push(
                        Diagnostic::error(d.span, "'steps' takes a tread count ≥ 2")
                            .code(Code::MALFORMED_VALUE),
                    );
                }
            }
            // A door's two pose knobs [SPEC 15.11]: the hinge jamb along the
            // segment's draw direction, and the side the leaf opens toward.
            "hinge" | "swing" => {
                let (words, message): (&[&str], &str) = if d.name == "hinge" {
                    (
                        &["start", "end"],
                        "'hinge' hangs the leaf at the segment's start or end",
                    )
                } else {
                    (
                        &["left", "right"],
                        "'swing' opens the leaf left or right of the pen's travel",
                    )
                };
                let ok = matches!(d.single(), Some(Value::Ident(s)) if words.contains(&s.as_str()));
                if !ok {
                    out.push(Diagnostic::error(d.span, message).code(Code::MALFORMED_VALUE));
                }
            }
            // The built face set [SPEC 6]: the four keywords and their numbers.
            // Arbitrary 100–900 is deferred [SPEC 24] — measurement would read
            // the nearest built static while the emitted CSS asked for another,
            // so a number outside the set errors instead of drifting.
            "font-weight" => {
                if let Some(Value::Number(n)) = d.single()
                    && !matches!(*n as u16, 400 | 500 | 600 | 700)
                {
                    out.push(
                        Diagnostic::error(
                            d.span,
                            "'font-weight' takes normal, medium, semibold, bold, or 400, 500, 600, 700",
                        )
                        .code(Code::MALFORMED_VALUE),
                    );
                }
            }
            _ => {}
        }
        // A `%` is a colour component and nothing else [SPEC 2] — inside
        // `rgb()` / `hsl()` / `oklch()` it rides the call (whose own reader
        // range-checks it); written bare in any slot it would flow through to
        // the output unread, so it errors here.
        if d.groups
            .iter()
            .flatten()
            .any(|v| matches!(v, Value::Percent(_)))
        {
            out.push(
                Diagnostic::error(
                    d.span,
                    format!("'{}' takes a number — a '%' is a colour component", d.name),
                )
                .code(Code::MALFORMED_VALUE),
            );
        }
        // A colour slot takes a colour [SPEC 2/10]: the ledger's `Colour` /
        // `Paint` kinds are what says a value *is* one, so the name check hangs
        // off them — the component-range check rides the builder call itself,
        // one stage on ([`crate::resolve::value`]).
        if matches!(
            prop.shape,
            Shape::One(Kind::Colour) | Shape::One(Kind::Paint)
        ) {
            // Gradients fill a shape, never text [SPEC 10.3] — so a flat-colour
            // slot (`color:`, the text colour of a whole subtree) and a text
            // leaf's own paint both refuse one; gradient-on-text is deferred
            // [SPEC 24].
            let text_slot =
                matches!(prop.shape, Shape::One(Kind::Colour)) || matches!(wearer, Wearer::Text);
            for v in d.groups.iter().flatten() {
                if text_slot && is_gradient(v) {
                    out.push(
                        Diagnostic::error(
                            d.span,
                            format!(
                                "'{}' takes a flat colour — a gradient fills a shape, and gradient-on-text is deferred",
                                d.name
                            ),
                        )
                        .code(Code::MALFORMED_VALUE),
                    );
                    continue;
                }
                check_colour(v, d.span, out);
            }
        }
    }

    // ── Class rules: wearer-set-wide judgment [SPEC 17] ──

    fn check_unworn_classes(&self, file: &File, out: &mut Vec<Diagnostic>) {
        let mut node_wearers: HashMap<&str, Vec<(String, Vec<String>)>> = HashMap::new();
        let mut link_wearers: HashSet<&str> = HashSet::new();
        let mut text_wearers: HashSet<&str> = HashSet::new();
        collect_wearers(
            file,
            self.types,
            &mut node_wearers,
            &mut link_wearers,
            &mut text_wearers,
        );

        for item in &file.stylesheet {
            let StyleItem::Rule(r) = item else { continue };
            let [SelUnit::Class(name)] = r.selector.units.as_slice() else {
                continue;
            };
            // Generated `.lini-*` classes (a lowered file's type bundles) are
            // worn implicitly at resolve — the compiler's, not the user's.
            if name.starts_with("lini-") {
                continue;
            }
            let nodes = node_wearers.get(name.as_str());
            let on_links = link_wearers.contains(name.as_str());
            let on_text = text_wearers.contains(name.as_str());
            if nodes.is_none() && !on_links && !on_text {
                out.push(
                    Diagnostic::warn(r.span, format!("class '.{name}' is never worn"))
                        .code(Code::CLASS_NEVER_WORN),
                );
                continue;
            }
            // CSS semantics: a property inert on one wearer is fine; dead on
            // every wearer it warns. A text leaf accepts any text-valid property
            // [SPEC 3] — the class-polymorphism law makes it live there.
            for d in &r.decls {
                let Some(prop) = properties::get(&d.name) else {
                    continue; // unknown-name already reported
                };
                let node_ok = nodes.is_some_and(|ws| {
                    ws.iter()
                        .any(|(kind, chain)| node_accepts(prop, kind, chain, None))
                });
                let link_ok = on_links && link_accepts(prop);
                let text_ok = on_text && properties::is_text_valid(&d.name);
                if !node_ok && !link_ok && !text_ok {
                    out.push(
                        Diagnostic::warn(
                            d.span,
                            format!("'.{name} {{ {}: … }}' is inert on every wearer", d.name),
                        )
                        .code(Code::INERT_EVERY_WEARER),
                    );
                }
            }
        }
    }

    /// A node's statically-known layout: its own `layout:` decl; else — when no
    /// stylesheet rule can inject one — the nearest layout default in its
    /// define/template chain; else `flow`. `None` when it can't be known.
    fn static_layout(&self, n: &Node, info: Option<&types::TypeInfo>) -> Option<String> {
        if n.style.iter().any(|d| d.name == "layout") {
            return layout_of(&n.style).map(str::to_string);
        }
        if self.rules_set_layout {
            return None;
        }
        let info = info?;
        for name in info.chain.iter().rev() {
            if let Some(style) = self.define_styles.get(name.as_str())
                && style.iter().any(|d| d.name == "layout")
            {
                return layout_of(style).map(str::to_string);
            }
            if let Some(l) = container_layout(name) {
                return Some(l.to_string());
            }
        }
        Some("flow".to_string())
    }
}

/// The layout a built-in container type owns [SPEC 8] — how a `Type` owner is
/// satisfied by a scope whose `layout:` matches it. Read off the ledger, never
/// restated: a type owns the `layout:` its own template bundle declares, or the
/// one a template it builds on declares (`|entity|` is a `|table|`, which is the
/// grid). Built once, since the bundles are constant.
fn container_layout(t: &str) -> Option<&'static str> {
    static MAP: OnceLock<HashMap<&'static str, String>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut m = HashMap::new();
        for (name, _) in types::TEMPLATES {
            let mut walk = Some(*name);
            while let Some(n) = walk {
                if let Some(l) = layout_of(&crate::ledger::defaults::template_bundle(n)) {
                    m.insert(*name, l.to_string());
                    break;
                }
                walk = types::template_base(n);
            }
        }
        m
    })
    .get(t)
    .map(String::as_str)
}

/// Whether a scope running `layout` reads a `Type(t)`-owned property [SPEC 17]
/// — `t` owns that very layout, or the scope is a **dialect** of `t`'s engine
/// ([`crate::resolve::layout_reads`]): a `layout: floorplan` root reads
/// `unit:`, the `|drawing|`'s own, because a floorplan *is* a drawing
/// [SPEC 15.11].
/// Whether a root running `layout` reads `prop` [SPEC 17] — the one answer to
/// "is this scene config meaningful here", shared by the root-block validator
/// above and by desugar, which must not stamp a default the root's engine
/// cannot read (its output is itself a legal file [SPEC 20]).
///
/// Inheriting scene config (text props, `clearance` / `routing`) reaches every
/// root; a scope-link property that *also* has node owners (`format`) is judged
/// against the layout, like any owned property.
pub(crate) fn root_reads(layout: &str, prop: &Property) -> bool {
    if prop.inherit != Inherit::No && !prop.has_node_owner() {
        return true;
    }
    prop.owners.iter().any(|o| match o {
        Owner::Universal | Owner::Root => true,
        // A layout-owned property reads on the root only when the root *runs*
        // that layout — `{ layout: flow; activation: none }` is the same misuse
        // as `activation:` on a flow node [SPEC 21]. A dialect runs its parent
        // engine, so it reads through the same predicate the `Owner::Type` arm
        // does.
        Owner::Layout(l) => crate::resolve::layout_reads(layout, l),
        Owner::Link => false,
        Owner::Type(t) => scope_reads_type(layout, t),
        Owner::Role(_) => false,
    })
}

fn scope_reads_type(layout: &str, t: &str) -> bool {
    container_layout(t).is_some_and(|owner| crate::resolve::layout_reads(layout, owner))
}

/// Whether a node wearer can use the property at all [SPEC 17].
fn node_accepts(prop: &Property, kind: &str, chain: &[String], own_layout: Option<&str>) -> bool {
    // The inheriting channels reach every node — text props cascade to every
    // node, pure scene config (`clearance`/`routing`) is valid on any
    // container. But a scope-link property that *also* has node owners
    // (`format`: chart / axis / series / drawing) is validated by those owners,
    // not blanket-accepted — so it errors on a wearer it can't mean anything on.
    if prop.inherit != Inherit::No && !prop.has_node_owner() {
        return true;
    }
    prop.owners.iter().any(|o| match o {
        Owner::Universal => true,
        Owner::Root | Owner::Link => false,
        // Layout-owned properties read on any container (its layout may be
        // set later in the cascade); `cell`/`span` gate on the parent instead.
        Owner::Layout(_) => true,
        // A container type's own properties also read on a scope whose
        // `layout:` is that type's layout (`{ layout: drawing; unit: "mm" }`).
        Owner::Type(t) => {
            *t == kind
                || chain.iter().any(|c| c == t)
                || own_layout.is_some_and(|l| scope_reads_type(l, t))
        }
        Owner::Role(r) => role_accepts(r, kind, chain),
    })
}

/// Whether a link wearer can use the property — a class's link side, and a
/// link's own block / `|-|` / `(-)` rule. The text channel dresses a link's
/// labels [SPEC 9]; everything else the link must own. A scope-config property
/// reaches a link through the scope-link channel, not off the link's own block,
/// so it is *not* accepted here (`routing:` — one scope, one strategy).
fn link_accepts(prop: &Property) -> bool {
    if prop.inherit == Inherit::Text {
        return true;
    }
    prop.owners.iter().any(|o| match o {
        Owner::Link => true,
        Owner::Role("dimension" | "mate") => true,
        // Links are styled with the node paint vocabulary [SPEC 9].
        Owner::Universal => true,
        _ => false,
    })
}

fn role_accepts(role: &str, kind: &str, chain: &[String]) -> bool {
    let in_chain = |names: &[&str]| {
        names
            .iter()
            .any(|n| *n == kind || chain.iter().any(|c| c == n))
    };
    match role {
        "series" => in_chain(&["line", "bars", "area", "dots", "bubble"]),
        "title-block" => in_chain(&["title-block"]),
        // Closed shapes [SPEC 7]: everything that has a body to duplicate.
        "closed" => !in_chain(&["line", "image"]),
        // The discrete part family [SPEC 16.3] — one list, in the type table.
        "discrete" => in_chain(types::DISCRETES),
        // A wall opening [SPEC 15.11] — the same, one list over.
        "opening" => in_chain(types::OPENINGS),
        // Dimensions and mates are links, never nodes.
        "dimension" | "mate" => false,
        _ => false,
    }
}

/// The contextual correction for a misused property [SPEC 21]: where it *does*
/// read, phrased per owner kind.
fn misuse_message(name: &str, wearer: &str, prop: &Property) -> String {
    // Scene config: the correction is where to *put* it, not a list of owners.
    // `routing:` is the scope's strategy — one scope, one strategy [SPEC 11,
    // ROUTING]. A link cannot select a second strategy inside that scope.
    match name {
        "density" => return "'density' is scene config — set it in the root block".to_string(),
        "routing" => {
            return "'routing' is a scope's strategy — one scope, one strategy; set it on the container".to_string();
        }
        _ => {}
    }
    let mut homes: Vec<String> = Vec::new();
    for o in prop.owners {
        let home = match o {
            Owner::Type(t) => format!("'|{t}|'"),
            Owner::Role("series") => "a chart series".to_string(),
            Owner::Role("dimension") => "a '(-)' dimension".to_string(),
            Owner::Role("mate") => "a '||' mate".to_string(),
            Owner::Role("title-block") => "the '|title-block|' fields".to_string(),
            Owner::Role("closed") => "closed shapes".to_string(),
            Owner::Role("discrete") => "the discrete parts ('|R|', '|C|', …)".to_string(),
            Owner::Role("opening") => "a wall opening ('|door|' / '|window|')".to_string(),
            Owner::Role(r) => format!("'{r}'"),
            Owner::Link => "links".to_string(),
            Owner::Layout(l) => format!("a 'layout: {l}'"),
            Owner::Root => "the root block".to_string(),
            Owner::Universal => continue,
        };
        if !homes.contains(&home) {
            homes.push(home);
        }
    }
    // A prose wearer ("the root block", "a link") reads as written; a bare type
    // name is spelled as the author wrote it.
    let wearer = if wearer.starts_with("the ") || wearer.starts_with("a ") {
        wearer.to_string()
    } else {
        format!("'|{wearer}|'")
    };
    format!(
        "'{name}' has no meaning on {wearer} — it reads on {}",
        homes.join(" / ")
    )
}

fn collect_wearers<'a>(
    file: &'a File,
    types: &Types,
    nodes: &mut HashMap<&'a str, Vec<(String, Vec<String>)>>,
    links: &mut HashSet<&'a str>,
    texts: &mut HashSet<&'a str>,
) {
    fn walk_children<'a>(
        children: &'a [Child],
        child_links: &'a [Link],
        types: &Types,
        nodes: &mut HashMap<&'a str, Vec<(String, Vec<String>)>>,
        links: &mut HashSet<&'a str>,
        texts: &mut HashSet<&'a str>,
    ) {
        for c in children {
            let n = match c {
                Child::Box(n) => n,
                // A bare text leaf wears its classes as a text wearer [SPEC 3].
                Child::Text(t) => {
                    for class in &t.classes {
                        texts.insert(class.as_str());
                    }
                    continue;
                }
            };
            if !n.classes.is_empty()
                && let Ok(info) = self_resolve(types, n)
            {
                let chain = with_worn_types(&info.1, &n.classes);
                for class in &n.classes {
                    nodes
                        .entry(class.as_str())
                        .or_default()
                        .push((info.0.clone(), chain.clone()));
                }
            }
            walk_children(&n.children, &n.links, types, nodes, links, texts);
        }
        for w in child_links {
            for class in &w.classes {
                links.insert(class.as_str());
            }
            // A link `[ ]` label is a text leaf and wears its classes there;
            // a carried annotation node wears them like any node [SPEC 15.9].
            for item in &w.labels {
                match item {
                    LabelItem::Text(label) => {
                        for class in &label.classes {
                            texts.insert(class.as_str());
                        }
                    }
                    LabelItem::Node(n) => {
                        if !n.classes.is_empty()
                            && let Ok(info) = self_resolve(types, n)
                        {
                            let chain = with_worn_types(&info.1, &n.classes);
                            for class in &n.classes {
                                nodes
                                    .entry(class.as_str())
                                    .or_default()
                                    .push((info.0.clone(), chain.clone()));
                            }
                        }
                        walk_children(&n.children, &n.links, types, nodes, links, texts);
                    }
                }
            }
        }
    }
    fn self_resolve(types: &Types, n: &Node) -> Result<(String, Vec<String>), ()> {
        let ty = n.ty.as_deref().unwrap_or("box");
        types
            .resolve(ty, n.span)
            .map(|i| (i.kind.as_str().to_string(), i.chain))
            .map_err(|_| ())
    }
    walk_children(&file.instances, &file.links, types, nodes, links, texts);
    for item in &file.stylesheet {
        if let StyleItem::Define(d) = item {
            walk_children(&d.children, &d.links, types, nodes, links, texts);
        }
    }
}

/// The chain plus any worn `.lini-<type>` classes' names — how a lowered
/// (`lini desugar`) instance still reads as its sugared type.
fn with_worn_types(chain: &[String], classes: &[String]) -> Vec<String> {
    let mut out = chain.to_vec();
    for c in classes {
        if let Some(name) = c.strip_prefix("lini-")
            && !out.iter().any(|n| n == name)
        {
            out.push(name.to_string());
        }
    }
    out
}

/// One value in a colour slot [SPEC 2/21]: a bare word must name a colour, and
/// a paint that *contains* colours (a gradient's stops, a hatch's line colour)
/// is judged stop by stop. Everything else — a hex (the lexer validated it), a
/// `--var`, a number, a folded expression, a builder call whose own reader owns
/// it — passes here.
fn check_colour(v: &Value, span: Span, out: &mut Vec<Diagnostic>) {
    match v {
        Value::Ident(name) if !crate::palette::css::is_color_name(name) => out.push(
            Diagnostic::error(span, format!("invalid color '{name}'")).code(Code::INVALID_COLOR),
        ),
        Value::Call(c) => {
            let stops = match c.name.as_str() {
                // A `linear-gradient`'s leading angle is a number, so it needs
                // no slicing — a non-ident is never judged.
                "gradient" | "linear-gradient" | "radial-gradient" | "light-dark" => &c.args[..],
                "hatch" => c.args.get(2..).unwrap_or(&[]),
                _ => &[],
            };
            for stop in stops {
                check_colour(stop, span, out);
            }
        }
        _ => {}
    }
}

/// Whether a value is one of the gradient paints [SPEC 10.3].
fn is_gradient(v: &Value) -> bool {
    matches!(v, Value::Call(c) if matches!(
        c.name.as_str(),
        "gradient" | "linear-gradient" | "radial-gradient"
    ))
}
