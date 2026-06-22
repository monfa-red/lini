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
use crate::span::Span;
use crate::syntax::ast::{Call, Value};

/// Resolve a declaration's comma-separated value groups into one value: one
/// group collapses to a scalar or `Tuple`, several groups form a `List`.
pub fn resolve_groups(
    groups: &[Vec<Value>],
    span: Span,
    vars: &VarTable,
) -> Result<ResolvedValue, Error> {
    if let [only] = groups {
        return resolve_group(only, span, vars);
    }
    let mut items = Vec::with_capacity(groups.len());
    for g in groups {
        items.push(resolve_group(g, span, vars)?);
    }
    Ok(ResolvedValue::List(items))
}

/// One space-separated group: a lone scalar stays a scalar, several become a
/// `Tuple`.
fn resolve_group(group: &[Value], span: Span, vars: &VarTable) -> Result<ResolvedValue, Error> {
    match group {
        [] => Err(Error::at(span, "empty value group")),
        [only] => resolve_scalar(only, span, vars),
        many => {
            let mut items = Vec::with_capacity(many.len());
            for v in many {
                items.push(resolve_scalar(v, span, vars)?);
            }
            Ok(ResolvedValue::Tuple(items))
        }
    }
}

fn resolve_scalar(v: &Value, span: Span, vars: &VarTable) -> Result<ResolvedValue, Error> {
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
        Value::Call(c) => resolve_call(c, span, vars)?,
    })
}

fn resolve_call(c: &Call, span: Span, vars: &VarTable) -> Result<ResolvedValue, Error> {
    let mut args = Vec::with_capacity(c.args.len());
    for a in &c.args {
        args.push(resolve_scalar(a, span, vars)?);
    }
    // `oklch()` is the palette's own colour space (SPEC §2/§11.2): fold it to a hex
    // at compile time so it renders in browsers, resvg, and email alike.
    if c.name == "oklch" {
        return resolve_oklch(&args, span);
    }
    Ok(ResolvedValue::Call(ResolvedCall {
        name: c.name.clone(),
        args,
    }))
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
        resolve_groups(groups, Span::empty(), &novars()).expect("resolve")
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
}
