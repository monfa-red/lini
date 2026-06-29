//! Value resolution: map a declaration's value groups into the
//! layout/render [`ResolvedValue`].
//!
//! Values are space-separated scalar groups, comma-separated into a list
//! (SPEC §2): `at: 100 50` is one group of two, `points: 0 0, 10 10` is two
//! groups of two. One scalar stays a scalar, a multi-scalar group becomes a
//! `Tuple`, and several groups become a `List` of those. Layout (`as_pair`,
//! `expand_box_value`) and render (`format_value`) read exactly that shape.
//!
//! A `--name` reference resolves to a `LiveVar` that prints `var(--lini-name)`;
//! these are visual vars only (SPEC §11.2), never layout numbers.

use super::ir::{ResolvedCall, ResolvedValue, VarTable};
use crate::error::Error;
use crate::expr::{self, Env, Expr, FuncTable, Value as ExprValue};
use crate::span::Span;
use crate::syntax::ast::{Call, Value};

/// The colour / track builders (SPEC §11.3, §5): these make a typed value and stay
/// a `Call` for the renderer / layout. Every other call is compute (a math builtin
/// or a user function) and folds to a number via the expression engine.
fn is_builder(name: &str) -> bool {
    matches!(
        name,
        "oklch"
            | "gradient"
            | "linear-gradient"
            | "radial-gradient"
            | "rgb"
            | "rgba"
            | "hsl"
            | "hsla"
            | "repeat"
    )
}

fn from_expr(v: ExprValue) -> ResolvedValue {
    match v {
        ExprValue::Number(n) => ResolvedValue::Number(n),
        ExprValue::Point(x, y) => {
            ResolvedValue::Tuple(vec![ResolvedValue::Number(x), ResolvedValue::Number(y)])
        }
    }
}

/// Fold a backtick body to a value, in a plain (non-geometry) context.
fn fold_expr(body: &str, span: Span, funcs: &FuncTable) -> Result<ExprValue, Error> {
    let expr = Expr::parse(body).map_err(|e| Error::at(span, e.0))?;
    expr.eval(&Env::new(), funcs)
        .map_err(|e| Error::at(span, e.0))
}

/// Evaluate a compute call (`scale(3)`, `min(a, b)`) to a number / point: each arg
/// folds to an expression value, then the engine applies the math builtin or user
/// function (SPEC §11.7).
fn fold_call(c: &Call, span: Span, funcs: &FuncTable) -> Result<ExprValue, Error> {
    let mut args = Vec::with_capacity(c.args.len());
    for a in &c.args {
        args.push(fold_arg(a, span, funcs)?);
    }
    expr::call(funcs, &c.name, &args).map_err(|e| Error::at(span, e.0))
}

fn fold_arg(v: &Value, span: Span, funcs: &FuncTable) -> Result<ExprValue, Error> {
    match v {
        Value::Number(n) => Ok(ExprValue::Number(*n)),
        Value::Expr(s) => fold_expr(s, span, funcs),
        Value::Call(c) if !is_builder(&c.name) => fold_call(c, span, funcs),
        _ => Err(Error::at(
            span,
            "a computed argument must be a number, a `…` expression, or another compute call",
        )),
    }
}

/// Resolve a declaration's comma-separated value groups into one value: one
/// group collapses to a scalar or `Tuple`, several groups form a `List`.
pub fn resolve_groups(
    groups: &[Vec<Value>],
    span: Span,
    vars: &VarTable,
    funcs: &FuncTable,
) -> Result<ResolvedValue, Error> {
    if let [only] = groups {
        return resolve_group(only, span, vars, funcs);
    }
    let mut items = Vec::with_capacity(groups.len());
    for g in groups {
        items.push(resolve_group(g, span, vars, funcs)?);
    }
    Ok(ResolvedValue::List(items))
}

/// Resolve a **property** declaration's value (SPEC §2): like [`resolve_groups`],
/// but a string-valued property (`title`, `href`, `src`, `path`) must be a quoted
/// string — a bare word there is an identifier, so it is an error. Variable
/// declarations and internal defaults call [`resolve_groups`] directly.
pub fn resolve_property(
    name: &str,
    groups: &[Vec<Value>],
    span: Span,
    vars: &VarTable,
    funcs: &FuncTable,
) -> Result<ResolvedValue, Error> {
    let value = resolve_groups(groups, span, vars, funcs)?;
    if is_string_valued(name) && has_bare_ident(&value) {
        return Err(Error::at(
            span,
            format!("'{name}' takes a quoted string — write {name}: \"…\""),
        ));
    }
    Ok(value)
}

/// Properties whose value is literal **text** — free text, a URL, an SVG path — and
/// so must be written quoted (SPEC §2). A *name* value (`symbol`, `font-family`, a
/// colour name) is a bare identifier instead, so it is not listed here.
fn is_string_valued(name: &str) -> bool {
    matches!(
        name,
        // Core text-valued props (SPEC §2)…
        "title" | "href" | "src" | "path"
        // …and the chart props that carry user text ([CHARTS.md] §2/§4/§5): tick / spoke
        // labels, the unit suffix, and a series' per-datum `tags`. Keyword chart props
        // (direction, scale, side, tooltip, …) stay bare identifiers.
        | "categories" | "labels" | "unit" | "tags"
    )
}

/// Whether a resolved value is, or contains, a bare identifier (an unquoted word) —
/// the test for a string-valued property given a non-string.
fn has_bare_ident(value: &ResolvedValue) -> bool {
    match value {
        ResolvedValue::Ident(_) => true,
        ResolvedValue::Tuple(items) | ResolvedValue::List(items) => {
            items.iter().any(has_bare_ident)
        }
        _ => false,
    }
}

/// One space-separated group: a lone scalar stays a scalar, several become a
/// `Tuple`.
fn resolve_group(
    group: &[Value],
    span: Span,
    vars: &VarTable,
    funcs: &FuncTable,
) -> Result<ResolvedValue, Error> {
    match group {
        [] => Err(Error::at(span, "empty value group")),
        [only] => resolve_scalar(only, span, vars, funcs),
        many => {
            let mut items = Vec::with_capacity(many.len());
            for v in many {
                items.push(resolve_scalar(v, span, vars, funcs)?);
            }
            Ok(ResolvedValue::Tuple(items))
        }
    }
}

fn resolve_scalar(
    v: &Value,
    span: Span,
    vars: &VarTable,
    funcs: &FuncTable,
) -> Result<ResolvedValue, Error> {
    Ok(match v {
        Value::Number(n) => ResolvedValue::Number(*n),
        Value::Percent(n) => ResolvedValue::Percent(*n),
        Value::String(s) => ResolvedValue::String(s.clone()),
        Value::Hex(h) => ResolvedValue::Hex(h.clone()),
        Value::Ident(s) => ResolvedValue::Ident(s.clone()),
        // `--name` → a live `var(--lini-name)`; visual-only (SPEC §11.2).
        Value::Var(name) => ResolvedValue::LiveVar {
            name: name.clone(),
            raw: false,
        },
        // A colour / track builder stays a typed Call; any other call is compute.
        Value::Call(c) if is_builder(&c.name) => resolve_call(c, span, vars, funcs)?,
        Value::Call(c) => from_expr(fold_call(c, span, funcs)?),
        // A backtick expression folds to a number / point (SPEC §11.7).
        Value::Expr(s) => from_expr(fold_expr(s, span, funcs)?),
    })
}

fn resolve_call(
    c: &Call,
    span: Span,
    vars: &VarTable,
    funcs: &FuncTable,
) -> Result<ResolvedValue, Error> {
    let mut args = Vec::with_capacity(c.args.len());
    for a in &c.args {
        args.push(resolve_scalar(a, span, vars, funcs)?);
    }
    // `oklch()` is the palette's own colour space (SPEC §2/§11.2): fold it to a hex
    // at compile time so it renders in browsers, resvg, and email alike.
    if c.name == "oklch" {
        return resolve_oklch(&args, span);
    }
    // Gradients (SPEC §11.3) stay a Call for the renderer to intern as a `url(#…)`
    // def; validate the shape here so a malformed one errors with a span rather than
    // emitting invalid CSS.
    if matches!(
        c.name.as_str(),
        "gradient" | "linear-gradient" | "radial-gradient"
    ) {
        validate_gradient(&c.name, &args, span)?;
    }
    Ok(ResolvedValue::Call(ResolvedCall {
        name: c.name.clone(),
        args,
    }))
}

/// A gradient needs ≥ 2 colour stops; `linear-gradient` additionally takes a
/// leading numeric angle (SPEC §11.3). Shape only — the renderer interns it.
fn validate_gradient(name: &str, args: &[ResolvedValue], span: Span) -> Result<(), Error> {
    let stops = if name == "linear-gradient" {
        if !matches!(args.first(), Some(ResolvedValue::Number(_))) {
            return Err(Error::at(
                span,
                "linear-gradient needs an angle first, then ≥ 2 colour stops — e.g. linear-gradient(135, --teal, --sky)",
            ));
        }
        &args[1..]
    } else {
        args
    };
    if stops.len() < 2 {
        return Err(Error::at(
            span,
            format!("{name}() needs at least two colour stops"),
        ));
    }
    Ok(())
}

/// Evaluate `oklch(L, C, H)` / `oklch(L, C, H, A)` to a `#rrggbb[aa]` literal. L and
/// A are 0..1 (a `%` is accepted too), C is the chroma, H is in degrees.
fn resolve_oklch(args: &[ResolvedValue], span: Span) -> Result<ResolvedValue, Error> {
    let frac = |v: &ResolvedValue| match v {
        ResolvedValue::Number(n) => Some(*n),
        ResolvedValue::Percent(p) => Some(p / 100.0),
        _ => None,
    };
    let num = |v: &ResolvedValue| match v {
        ResolvedValue::Number(n) => Some(*n),
        _ => None,
    };
    let bad = || {
        Error::at(
            span,
            "oklch expects (L, C, H) or (L, C, H, A) — L and A in 0..1, C ≥ 0, H in degrees",
        )
    };
    let (l, c, h, a) = match args {
        [l, c, h] => (
            frac(l).ok_or_else(bad)?,
            num(c).ok_or_else(bad)?,
            num(h).ok_or_else(bad)?,
            None,
        ),
        [l, c, h, a] => (
            frac(l).ok_or_else(bad)?,
            num(c).ok_or_else(bad)?,
            num(h).ok_or_else(bad)?,
            Some(frac(a).ok_or_else(bad)?),
        ),
        _ => return Err(bad()),
    };
    if !(0.0..=1.0).contains(&l) || c < 0.0 || a.is_some_and(|a| !(0.0..=1.0).contains(&a)) {
        return Err(bad());
    }
    let mut hex = crate::palette::oklch::oklch_to_hex(l, c, h);
    if let Some(a) = a {
        hex.push_str(&format!("{:02x}", (a * 255.0).round() as u8));
    }
    Ok(ResolvedValue::Hex(hex))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn novars() -> VarTable {
        VarTable::new()
    }

    fn resolve(groups: &[Vec<Value>]) -> ResolvedValue {
        resolve_groups(groups, Span::empty(), &novars(), &FuncTable::new()).expect("resolve")
    }

    fn resolve_with(groups: &[Vec<Value>], funcs: &FuncTable) -> ResolvedValue {
        resolve_groups(groups, Span::empty(), &novars(), funcs).expect("resolve")
    }

    #[test]
    fn backtick_expression_folds_to_a_number() {
        let v = resolve(&[vec![Value::Expr("8 * 2".into())]]);
        assert!(matches!(v, ResolvedValue::Number(n) if n == 16.0));
    }

    #[test]
    fn a_user_function_call_folds() {
        let mut funcs = FuncTable::new();
        funcs.insert(
            "scale".into(),
            vec!["n".into()],
            Expr::parse("100 * 1.2 ^ n").unwrap(),
        );
        let v = resolve_with(
            &[vec![Value::Call(Call {
                name: "scale".into(),
                args: vec![Value::Number(0.0)],
            })]],
            &funcs,
        );
        assert!(matches!(v, ResolvedValue::Number(n) if n == 100.0));
    }

    #[test]
    fn a_computed_argument_in_a_builder_folds() {
        // `repeat(3, `80 * 2`)` — repeat stays a Call, its computed arg folds to 160.
        let v = resolve(&[vec![Value::Call(Call {
            name: "repeat".into(),
            args: vec![Value::Number(3.0), Value::Expr("80 * 2".into())],
        })]]);
        match v {
            ResolvedValue::Call(c) => {
                assert_eq!(c.name, "repeat");
                assert!(matches!(c.args[1], ResolvedValue::Number(n) if n == 160.0));
            }
            other => panic!("expected a repeat call, got {other:?}"),
        }
    }

    #[test]
    fn an_unknown_function_errors() {
        let r = resolve_groups(
            &[vec![Value::Call(Call {
                name: "nope".into(),
                args: vec![],
            })]],
            Span::empty(),
            &novars(),
            &FuncTable::new(),
        );
        assert!(r.is_err());
    }

    #[test]
    fn single_scalar_stays_scalar() {
        let v = resolve(&[vec![Value::Number(5.0)]]);
        assert!(matches!(v, ResolvedValue::Number(n) if n == 5.0));
    }

    #[test]
    fn space_separated_group_becomes_a_tuple() {
        // `at: 100 50` — one group, two scalars.
        let v = resolve(&[vec![Value::Number(100.0), Value::Number(50.0)]]);
        match v {
            ResolvedValue::Tuple(items) => assert_eq!(items.len(), 2),
            other => panic!("expected tuple, got {:?}", other),
        }
    }

    #[test]
    fn comma_groups_become_a_list_of_tuples() {
        // `points: 0 0, 10 10`.
        let v = resolve(&[
            vec![Value::Number(0.0), Value::Number(0.0)],
            vec![Value::Number(10.0), Value::Number(10.0)],
        ]);
        match v {
            ResolvedValue::List(items) => {
                assert_eq!(items.len(), 2);
                assert!(matches!(&items[0], ResolvedValue::Tuple(t) if t.len() == 2));
            }
            other => panic!("expected list, got {:?}", other),
        }
    }

    #[test]
    fn mixed_track_list_keeps_idents_and_calls() {
        // `columns: auto 40 repeat(2)` — one group of three mixed scalars.
        let v = resolve(&[vec![
            Value::Ident("auto".into()),
            Value::Number(40.0),
            Value::Call(Call {
                name: "repeat".into(),
                args: vec![Value::Number(2.0)],
            }),
        ]]);
        match v {
            ResolvedValue::Tuple(items) => {
                assert!(matches!(items[0], ResolvedValue::Ident(_)));
                assert!(matches!(items[1], ResolvedValue::Number(_)));
                assert!(matches!(items[2], ResolvedValue::Call(_)));
            }
            other => panic!("expected tuple, got {:?}", other),
        }
    }

    #[test]
    fn var_reference_resolves_to_a_live_var() {
        let v = resolve(&[vec![Value::Var("accent".into())]]);
        assert!(matches!(v, ResolvedValue::LiveVar { name, .. } if name == "accent"));
    }

    #[test]
    fn call_resolves_its_arguments() {
        let v = resolve(&[vec![Value::Call(Call {
            name: "rgb".into(),
            args: vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)],
        })]]);
        match v {
            ResolvedValue::Call(c) => {
                assert_eq!(c.name, "rgb");
                assert_eq!(c.args.len(), 3);
            }
            other => panic!("expected call, got {:?}", other),
        }
    }

    fn oklch(args: Vec<Value>) -> Result<ResolvedValue, Error> {
        resolve_groups(
            &[vec![Value::Call(Call {
                name: "oklch".into(),
                args,
            })]],
            Span::empty(),
            &novars(),
            &FuncTable::new(),
        )
    }

    #[test]
    fn oklch_folds_to_a_hex() {
        // oklch(1, 0, 0) is white.
        let v = oklch(vec![
            Value::Number(1.0),
            Value::Number(0.0),
            Value::Number(0.0),
        ])
        .unwrap();
        assert!(matches!(v, ResolvedValue::Hex(h) if h == "ffffff"));
    }

    #[test]
    fn oklch_with_alpha_folds_to_hex8() {
        let v = oklch(vec![
            Value::Number(0.0),
            Value::Number(0.0),
            Value::Number(0.0),
            Value::Number(1.0),
        ])
        .unwrap();
        assert!(matches!(v, ResolvedValue::Hex(h) if h == "000000ff"));
    }

    #[test]
    fn oklch_bad_arity_errors() {
        assert!(oklch(vec![Value::Number(0.5), Value::Number(0.1)]).is_err());
    }

    #[test]
    fn oklch_out_of_range_lightness_errors() {
        assert!(
            oklch(vec![
                Value::Number(1.5),
                Value::Number(0.1),
                Value::Number(180.0)
            ])
            .is_err()
        );
    }

    fn grad(name: &str, args: Vec<Value>) -> Result<ResolvedValue, Error> {
        resolve_groups(
            &[vec![Value::Call(Call {
                name: name.into(),
                args,
            })]],
            Span::empty(),
            &novars(),
            &FuncTable::new(),
        )
    }

    #[test]
    fn valid_gradient_stays_a_call() {
        let v = grad(
            "gradient",
            vec![Value::Var("teal".into()), Value::Var("sky".into())],
        )
        .unwrap();
        assert!(matches!(v, ResolvedValue::Call(c) if c.name == "gradient"));
    }

    #[test]
    fn gradient_with_one_stop_errors() {
        assert!(grad("gradient", vec![Value::Var("teal".into())]).is_err());
    }

    #[test]
    fn linear_gradient_without_angle_errors() {
        assert!(
            grad(
                "linear-gradient",
                vec![Value::Var("teal".into()), Value::Var("sky".into())],
            )
            .is_err()
        );
    }
}
