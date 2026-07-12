use super::*;

fn parse_ok(src: &str) -> File {
    let tokens = crate::lexer::lex(src).expect("lex");
    parse(src, &tokens).expect("parse")
}

fn parse_err(src: &str) -> String {
    let tokens = crate::lexer::lex(src).expect("lex");
    match parse(src, &tokens) {
        Ok(_) => panic!("expected a parse error for: {src}"),
        Err(e) => e.message,
    }
}

/// The i-th top-level instance as a box, panicking if it is bare text.
fn instance(f: &File, i: usize) -> &Node {
    match &f.instances[i] {
        Child::Box(n) => n,
        Child::Text(_) => panic!("instance {i} is text, not a box"),
    }
}

fn label(n: &Node) -> Option<&str> {
    n.label.as_ref().map(|t| t.text.as_str())
}

// ── Identity in the bars ──

#[test]
fn identity_type_and_id_in_bars() {
    let f = parse_ok("|box#server|\n");
    let n = instance(&f, 0);
    assert_eq!(n.ty.as_deref(), Some("box"));
    assert_eq!(n.id.as_deref(), Some("server"));
    assert_eq!(label(n), None);
}

#[test]
fn id_only_bars_default_box() {
    let f = parse_ok("|#cat|\n");
    let n = instance(&f, 0);
    assert_eq!(n.ty, None);
    assert_eq!(n.id.as_deref(), Some("cat"));
}

#[test]
fn anonymous_labelled_box() {
    let f = parse_ok("|box| \"Load balancer\"\n");
    let n = instance(&f, 0);
    assert_eq!(n.ty.as_deref(), Some("box"));
    assert_eq!(n.id, None);
    assert_eq!(label(n), Some("Load balancer"));
}

#[test]
fn full_node_head_label_classes_style_child() {
    let f = parse_ok("|box#cat| \"Cat\" .hot.loud { fill: red } [ |badge| \"x\" ]\n");
    let n = instance(&f, 0);
    assert_eq!(n.id.as_deref(), Some("cat"));
    assert_eq!(label(n), Some("Cat"));
    assert_eq!(n.classes, vec!["hot", "loud"]);
    assert_eq!(n.style.len(), 1);
    assert_eq!(n.children.len(), 1);
    assert!(matches!(&n.children[0], Child::Box(b) if b.ty.as_deref() == Some("badge")));
}

#[test]
fn empty_string_label_is_kept() {
    let f = parse_ok("|box#cat| \"\"\n");
    assert_eq!(label(instance(&f, 0)), Some(""));
}

#[test]
fn head_label_may_carry_the_nodes_own_style() {
    // `{ }` after the head label is the node's block, not the label's [SPEC 3].
    let f = parse_ok("|box#api| \"API\" { fill: red }\n");
    let n = instance(&f, 0);
    assert_eq!(label(n), Some("API"));
    assert_eq!(n.style.len(), 1);
}

#[test]
fn label_and_bracket_content_coexist() {
    let f = parse_ok("|group#k| \"Kitchen\" [ |box#bowl| \"Bowl\" ]\n");
    let n = instance(&f, 0);
    assert_eq!(label(n), Some("Kitchen"));
    assert_eq!(n.children.len(), 1);
}

// ── Tail-order errors ──

#[test]
fn two_head_labels_error() {
    assert!(parse_err("|box#cat| \"a\" \"b\"\n").contains("one inline label"));
}

#[test]
fn label_after_a_class_errors() {
    assert!(parse_err("|box#cat| .hot \"Cat\"\n").contains("comes before classes"));
}

#[test]
fn class_in_the_bars_errors() {
    for src in ["|box.hot|\n", "|.hot|\n"] {
        assert!(parse_err(src).contains("follows the bars"), "{src}");
    }
    parse_ok("|box| .hot\n"); // the class follows the bars
}

#[test]
fn empty_bars_error() {
    for src in ["| |\n", "||\n"] {
        assert!(parse_err(src).contains("needs a type or an '#id'"), "{src}");
    }
}

#[test]
fn invalid_id_errors() {
    assert!(parse_err("|box#123|\n").contains("not a valid id"));
}

// ── Selectors ──

#[test]
fn selector_units() {
    let f = parse_ok(
        "{\n  |box| { radius: 4; }\n  .hot { stroke-width: 2; }\n  #hero { fill: gold; }\n  |table| |box| { stroke-width: 0; }\n  .sidebar |box| { fill: gray; }\n  |table#main| |box| { fill: white; }\n}\n",
    );
    let rule = |i: usize| match &f.stylesheet[i] {
        StyleItem::Rule(r) => &r.selector.units,
        _ => panic!("rule {i}"),
    };
    assert!(matches!(rule(0).as_slice(), [SelUnit::Type { name, id: None }] if name == "box"));
    assert!(matches!(rule(1).as_slice(), [SelUnit::Class(c)] if c == "hot"));
    assert!(matches!(rule(2).as_slice(), [SelUnit::Id(i)] if i == "hero"));
    assert_eq!(rule(3).len(), 2);
    assert!(matches!(rule(4)[0], SelUnit::Class(_)));
    assert!(
        matches!(&rule(5)[0], SelUnit::Type { name, id: Some(id) } if name == "main" || id == "main")
    );
}

#[test]
fn dimension_and_link_selectors() {
    let f = parse_ok("{\n  |-| { stroke: gray; }\n  (-) { stroke: blue; }\n}\n");
    let rule = |i: usize| match &f.stylesheet[i] {
        StyleItem::Rule(r) => &r.selector.units,
        _ => panic!("rule {i}"),
    };
    assert!(matches!(rule(0).as_slice(), [SelUnit::Link]));
    assert!(matches!(rule(1).as_slice(), [SelUnit::Dimension]));
    // Per-kind dimension selectors `(o)` / `(<)` are deferred [SPEC 23].
    assert!(parse_err("{\n  (o) { stroke: red; }\n}\n").contains("deferred"));
}

#[test]
fn compound_selector_unit_errors() {
    assert!(parse_err("{\n  |box.hot| { fill: red; }\n}\n").contains("can't glue"));
}

#[test]
fn bare_type_rule_errors() {
    assert!(parse_err("{\n  box { radius: 4; }\n}\n").contains("only appears in bars"));
}

#[test]
fn define_in_stylesheet() {
    let f = parse_ok("{\n  |treat::box| { radius: 5; }\n}\n|treat#x|\n");
    match &f.stylesheet[0] {
        StyleItem::Define(d) => {
            assert_eq!(d.name, "treat");
            assert_eq!(d.base, "box");
        }
        _ => panic!("expected a define"),
    }
    assert_eq!(instance(&f, 0).ty.as_deref(), Some("treat"));
}

#[test]
fn define_with_intrinsic_children() {
    let f = parse_ok(
        "{\n  |room::group| {\n    gap: 4;\n  } [\n    |box#inlet|\n    |box#outlet|\n    inlet -> outlet\n  ]\n}\n",
    );
    match &f.stylesheet[0] {
        StyleItem::Define(d) => {
            assert_eq!(d.children.len(), 2);
            assert_eq!(d.links.len(), 1);
            assert_eq!(d.style.len(), 1);
        }
        _ => panic!("expected a define"),
    }
}

// ── Links ──

#[test]
fn quickstart_three_box_chain() {
    let f = parse_ok("cat -> dog -> bird\n");
    assert!(f.stylesheet.is_empty() && f.instances.is_empty());
    assert_eq!(f.links.len(), 1);
    assert_eq!(f.links[0].chain.len(), 3);
}

fn point_of(ep: &Endpoint) -> Option<&str> {
    ep.point.as_ref().map(|p| p.name.as_str())
}

fn wire_line(f: &File) -> crate::ast::LineStyle {
    match f.links[0].op() {
        ChainOp::Wire(op) => op.line,
        other => panic!("expected a wire op, got {other:?}"),
    }
}

#[test]
fn link_with_sides_label_class_style() {
    let f = parse_ok("a:left -> b:top \"watches\" .loud { along: 0.5 }\n");
    let w = &f.links[0];
    assert_eq!(point_of(&w.chain[0].endpoints[0]), Some("left"));
    assert_eq!(point_of(&w.chain[1].endpoints[0]), Some("top"));
    assert_eq!(w.label.as_ref().map(|t| t.text.as_str()), Some("watches"));
    assert_eq!(w.classes, vec!["loud"]);
    assert_eq!(w.style.len(), 1);
}

#[test]
fn link_line_styles() {
    assert_eq!(
        wire_line(&parse_ok("a -> b\n")),
        crate::ast::LineStyle::Solid
    );
    assert_eq!(
        wire_line(&parse_ok("a --> b\n")),
        crate::ast::LineStyle::Dashed
    );
    assert_eq!(
        wire_line(&parse_ok("a ---> b\n")),
        crate::ast::LineStyle::Dotted
    );
    assert_eq!(
        wire_line(&parse_ok("a ~> b\n")),
        crate::ast::LineStyle::Wavy
    );
}

#[test]
fn fan_and_class_on_link() {
    let f = parse_ok("a & b -> c & d .loud\n");
    let w = &f.links[0];
    assert_eq!(w.chain[0].endpoints.len(), 2);
    assert_eq!(w.chain[1].endpoints.len(), 2);
    assert_eq!(w.classes, vec!["loud"]);
}

#[test]
fn link_head_label_and_bracket_labels_coexist() {
    let f = parse_ok("a -> b \"x\" [ \"y\" ]\n");
    let w = &f.links[0];
    assert_eq!(w.label.as_ref().map(|t| t.text.as_str()), Some("x"));
    assert_eq!(w.labels.len(), 1);
}

#[test]
fn link_two_bracket_labels() {
    let f = parse_ok("a -> b [ \"x\" \"y\" ]\n");
    assert_eq!(f.links[0].labels.len(), 2);
    assert_eq!(f.links[0].labels[0].text, "x");
}

#[test]
fn two_head_labels_on_a_link_error() {
    assert!(parse_err("a -> b \"x\" \"y\"\n").contains("one inline label"));
}

#[test]
fn dotpath_endpoint_and_forced_side() {
    let f = parse_ok("cat:right -> kitchen.counter.bowl:left\n");
    let w = &f.links[0];
    assert_eq!(w.chain[0].endpoints[0].path, vec!["cat"]);
    assert_eq!(point_of(&w.chain[0].endpoints[0]), Some("right"));
    assert_eq!(
        w.chain[1].endpoints[0].path,
        vec!["kitchen", "counter", "bowl"]
    );
    assert_eq!(point_of(&w.chain[1].endpoints[0]), Some("left"));
}

#[test]
fn endpoint_point_is_raw_at_parse() {
    // The wider point set [SPEC 15.2] is resolve's call, per scope — the
    // parser stores the raw name (`:middle` errors there, not here).
    let f = parse_ok("a:middle -> b:top-left\n");
    assert_eq!(point_of(&f.links[0].chain[0].endpoints[0]), Some("middle"));
    assert_eq!(
        point_of(&f.links[0].chain[1].endpoints[0]),
        Some("top-left")
    );
}

#[test]
fn measuring_ops_parse_one_ended_and_binary() {
    // `pin (o)` — a unary round measure toward its tail [SPEC 15.6/21]
    // (the parser accepts unary; resolve gates arity per op).
    let f = parse_ok("pin (o)\n");
    assert_eq!(f.links[0].chain.len(), 1);
    assert_eq!(f.links[0].op(), ChainOp::Measure(crate::ast::DrawOp::Round));
    // `(-)` binary — the linear span between two anchors.
    let f = parse_ok("a:left (-) b:right\n");
    assert_eq!(f.links[0].chain.len(), 2);
    assert_eq!(
        f.links[0].op(),
        ChainOp::Measure(crate::ast::DrawOp::Linear)
    );
    // `(<)` binary — two line-like anchors.
    let f = parse_ok("body:flank (<) body:base\n");
    assert_eq!(f.links[0].chain.len(), 2);
    assert_eq!(f.links[0].op(), ChainOp::Measure(crate::ast::DrawOp::Angle));
    // One-ended with a tail label.
    let f = parse_ok("bolt <- \"THRU\"\n");
    assert_eq!(f.links[0].chain.len(), 1);
    assert_eq!(
        f.links[0].label.as_ref().map(|t| t.text.as_str()),
        Some("THRU")
    );
}

#[test]
fn mate_is_two_adjacent_pipes_at_op_position() {
    let f = parse_ok("nozzle:left || barrel:right { gap: 4 }\n");
    assert_eq!(f.links[0].op(), ChainOp::Mate);
    assert_eq!(f.links[0].chain.len(), 2);
    // Spaced pipes are not a mate — `a | b` stays an invalid statement.
    assert!(!parse_err("a | | b\n").is_empty());
}

#[test]
fn chain_past_a_label_errors() {
    assert!(parse_err("a <- b <- \"x\"\n").contains("a text callout ends its statement"));
}

#[test]
fn pen_items_parse_only_in_draw() {
    let f = parse_ok("|sketch#s| { draw: move(-80, 0) right(50):seat point():m1 close(); }\n");
    let draw = &instance(&f, 0).style[0];
    assert_eq!(draw.name, "draw");
    let items = &draw.groups[0];
    assert!(matches!(&items[0], Value::Call(c) if c.name == "move"));
    assert!(matches!(&items[1], Value::NamedCall(c, n) if c.name == "right" && n == "seat"));
    assert!(matches!(&items[2], Value::NamedCall(c, n) if c.name == "point" && n == "m1"));
    assert!(matches!(&items[3], Value::Call(c) if c.name == "close"));
    // Outside a draw:, a freestanding `:` keeps the runaway-decl diagnostic.
    assert!(parse_err("|box| { padding: :x }\n").contains("a declaration ends with ';'"));
}

#[test]
fn a_floating_segment_errors() {
    // One space must never flip meaning [SPEC 15.3]: a `:segment` glues to
    // its call; a station is `point():v`.
    assert!(
        parse_err("|sketch#s| { draw: move(0, 0) right(12) :v down(5); }\n")
            .contains("a ':segment' glues to its call — name a station with point():v")
    );
}

#[test]
fn call_arg_space_group() {
    // `hatch(45 -45, 6)` — one slot holding a space-group [SPEC 10.3].
    let f = parse_ok("|box| { fill: hatch(45 -45, 6) }\n");
    let fill = &instance(&f, 0).style[0];
    let Value::Call(c) = &fill.groups[0][0] else {
        panic!("expected a call");
    };
    assert!(matches!(&c.args[0], Value::Tuple(g) if g.len() == 2));
    assert!(matches!(&c.args[1], Value::Number(n) if *n == 6.0));
}

#[test]
fn mixed_operators() {
    // Wire hops each carry their own op [SPEC 9] — `a -> b -- c` parses, the
    // dashed hop its own; mixing operator *kinds* stays a parse error.
    let f = parse_ok("a -> b -- c\n");
    assert_eq!(f.links[0].ops.len(), 2);
    assert_ne!(f.links[0].ops[0], f.links[0].ops[1]);
    assert!(parse_err("a -> b (-) c\n").contains("mixes operators"));
}

// ── Statement classification ──

#[test]
fn bare_name_on_canvas_errors() {
    assert!(parse_err("cat\n").contains("leads with bars"));
}

#[test]
fn bare_string_is_a_text_node() {
    let f = parse_ok("\"a\" \"b\" \"c\"\n");
    assert_eq!(f.instances.len(), 3);
    assert!(f.instances.iter().all(|c| matches!(c, Child::Text(_))));
}

#[test]
fn text_node_takes_a_style_block() {
    let f = parse_ok("\"hi\" { color: red; translate: 0 -6 }\n\"plain\"\n");
    match &f.instances[0] {
        Child::Text(t) => assert_eq!(t.style.len(), 2),
        _ => panic!("styled text"),
    }
    match &f.instances[1] {
        Child::Text(t) => assert!(t.style.is_empty()),
        _ => panic!("bare text"),
    }
}

#[test]
fn text_node_takes_a_worn_class_chain() {
    let f = parse_ok("\"Starter\" .card-title.loud { color: red }\n");
    match &f.instances[0] {
        Child::Text(t) => {
            assert_eq!(t.text, "Starter");
            assert_eq!(t.classes, vec!["card-title", "loud"]);
            assert_eq!(t.style.len(), 1);
        }
        _ => panic!("classed text"),
    }
}

#[test]
fn text_class_rides_the_content_slot_not_the_head_label() {
    // `|box#api| "API" .hot` — the class after a head label is the *node's*, and
    // the lowered label leaf stays classless (the head-label disambiguation).
    let f = parse_ok("|box#api| \"API\" .hot\n");
    let n = instance(&f, 0);
    assert_eq!(n.classes, vec!["hot"]);
    assert_eq!(n.label.as_ref().unwrap().classes, Vec::<String>::new());
}

#[test]
fn a_link_bracket_label_takes_a_class() {
    // A link `[ ]` label is content position, so it wears classes; the head-label
    // class stays the link's (`a -> b "x" .loud`).
    let f = parse_ok("a -> b [ \"grow\" .loud ]\n");
    let w = &f.links[0];
    assert!(w.classes.is_empty());
    assert_eq!(w.labels[0].classes, vec!["loud"]);
}

#[test]
fn three_phases() {
    let f = parse_ok(
        "{\n  layout: grid;\n  |box| { radius: 6; }\n  .hot { stroke-width: 2; }\n}\n\
             |box#server|\n|box#client|\nserver -> client \"requests\"\n",
    );
    assert_eq!(f.stylesheet.len(), 3);
    assert_eq!(f.instances.len(), 2);
    assert_eq!(f.links.len(), 1);
}

#[test]
fn stylesheet_is_optional() {
    let f = parse_ok("|box#server|\nserver -> server\n");
    assert!(f.stylesheet.is_empty());
    assert_eq!(f.instances.len(), 1);
}

// ── Values ──

#[test]
fn hex_value_validates() {
    let f = parse_ok("|box#x| { fill: #f80; stroke: #ffaa00cc }\n");
    let n = instance(&f, 0);
    assert!(matches!(&n.style[0].groups[0][0], Value::Hex(h) if h == "f80"));
    assert!(parse_err("|box#x| { fill: #zz }\n").contains("invalid hex color"));
}

#[test]
fn call_and_var_values() {
    let f = parse_ok(
        "{\n  columns: repeat(3);\n  --brand: #ff6600;\n}\n|box#card| { fill: --brand; columns: 80 repeat(2, 40) }\n",
    );
    match &f.stylesheet[0] {
        StyleItem::RootDecl(d) => {
            assert!(matches!(&d.groups[0][0], Value::Call(c) if c.name == "repeat"))
        }
        _ => panic!(),
    }
    match &f.stylesheet[1] {
        StyleItem::Var(d) => assert_eq!(d.name, "brand"),
        _ => panic!(),
    }
}

#[test]
fn value_groups_space_and_comma() {
    let f = parse_ok("|line#x| { points: 0 0, 10 10, 20 0; translate: 100 50 }\n");
    let points = instance(&f, 0)
        .style
        .iter()
        .find(|d| d.name == "points")
        .unwrap();
    assert_eq!(points.groups.len(), 3);
    assert_eq!(points.groups[0].len(), 2);
}

#[test]
fn groups_and_expr_args_capture_raw() {
    let f = parse_ok("|box#a| { padding: (8 * 2); width: gain(5 * r, 10) }\n");
    let n = instance(&f, 0);
    // A free-standing `(…)` group keeps its inner text, outer parens stripped.
    assert!(matches!(&n.style[0].groups[0][0], Value::Expr(s) if s == "8 * 2"));
    // A call keeps an operator-bearing argument raw; a plain argument stays a value.
    match &n.style[1].groups[0][0] {
        Value::Call(c) => {
            assert_eq!(c.name, "gain");
            assert!(matches!(&c.args[0], Value::Expr(s) if s == "5 * r"));
            assert!(matches!(&c.args[1], Value::Number(v) if *v == 10.0));
        }
        other => panic!("expected a call, got {other:?}"),
    }
}

// ── Phase / context errors ──

#[test]
fn stylesheet_after_instance_errors() {
    assert!(parse_err("|box#x|\n{\n  |box| { radius: 4; }\n}\n").contains("must come first"));
}

#[test]
fn instances_and_links_interleave_at_root() {
    // [SPEC 3]: nodes and links interleave in source order (a `layout: sequence`
    // reads that order as time) — a node after a link is no longer an error.
    let f = parse_ok("a -> b\n|box#c|\n");
    assert_eq!(f.instances.len(), 1, "the |box#c| instance");
    assert_eq!(f.links.len(), 1, "the a -> b link");
}

#[test]
fn link_as_instance_errors() {
    assert!(parse_err("|link|\n").contains("drawn by operators"));
}

#[test]
fn node_type_as_instance_errors() {
    assert!(parse_err("|node|\n").contains("umbrella"));
}

#[test]
fn link_defaults_block_is_rejected() {
    assert!(parse_err("{\n  -> { stroke: #666; }\n}\na -> b\n").contains("draws a link"));
}

#[test]
fn decl_in_children_errors() {
    assert!(parse_err("|group#g| [\n  gap: 4\n]\n").contains("go in '{ }'"));
}

#[test]
fn body_children_and_links_interleave() {
    // A child may follow an internal link in a body [SPEC 3].
    let f = parse_ok("|group#g| [\n  |box#a|\n  a -> a\n  |box#b|\n]\n");
    let Child::Box(g) = &f.instances[0] else {
        panic!("group node");
    };
    assert_eq!(g.children.len(), 2, "boxes a and b");
    assert_eq!(g.links.len(), 1, "the a -> a link");
}

#[test]
fn empty_declaration_errors() {
    assert!(parse_err("|box#a| { gap: }\n").contains("needs a value"));
}

#[test]
fn a_missing_declaration_semicolon_errors() {
    assert!(parse_err("|box#a| { radius: 6 stroke: 2 }\n").contains("ends with ';'"));
}

// ── Expressions & functions [SPEC 10.7] ──

#[test]
fn binding_and_expr_values() {
    let f = parse_ok(
        "{ scale(n) = (100 * 1.2 ^ n); my_r = 5; }\n|box#a| { width: scale(3); padding: (8 * 2) }\n",
    );
    match &f.stylesheet[0] {
        StyleItem::Binding(fd) => {
            assert_eq!(fd.name, "scale");
            assert_eq!(fd.params, vec!["n"]);
            assert!(fd.body.contains("1.2"));
        }
        _ => panic!("expected a binding"),
    }
    // A scalar binding is a zero-parameter `FuncDef` [SPEC 10.7].
    match &f.stylesheet[1] {
        StyleItem::Binding(fd) => {
            assert_eq!(fd.name, "my_r");
            assert!(fd.params.is_empty());
            assert_eq!(fd.body, "5");
        }
        _ => panic!("expected a scalar binding"),
    }
    let n = instance(&f, 0);
    assert!(matches!(&n.style[0].groups[0][0], Value::Call(c) if c.name == "scale"));
    assert!(matches!(&n.style[1].groups[0][0], Value::Expr(s) if s.contains('8')));
}

#[test]
fn a_declaration_value_spans_lines_until_semicolon() {
    // Newlines inside a value are whitespace; the value runs to `;` [SPEC 2/3].
    let f = parse_ok("|line#x| { points: 0 0,\n  10 10,\n  20 0; }\n");
    let points = instance(&f, 0)
        .style
        .iter()
        .find(|d| d.name == "points")
        .unwrap();
    assert_eq!(points.groups.len(), 3);
}
