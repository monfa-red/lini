//! v4 value resolution: map a declaration's value groups into the
//! layout/render [`ResolvedValue`].
//!
//! v4 values are space-separated scalar groups, comma-separated into a list
//! (SPEC §2): `at: 100 50` is one group of two, `points: 0 0, 10 10` is two
//! groups of two. One scalar stays a scalar, a multi-scalar group becomes a
//! `Tuple`, and several groups become a `List` of those. Layout (`as_pair`,
//! `expand_box_value`) and render (`format_value`) read exactly that shape, so
//! the value bridge carries straight over from v3.
//!
//! `--name` references resolve to a `LiveVar`; a layout var bakes its numeric
//! value into the reference so it reads as a number at layout time yet still
//! prints `var(--lini-name)` in live mode.

use super::ir::{ResolvedCall, ResolvedValue, VarEntry, VarKind, VarTable};
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
        Value::String(s) => ResolvedValue::String(s.clone()),
        Value::Hex(h) => ResolvedValue::Hex(h.clone()),
        Value::Ident(s) => ResolvedValue::Ident(s.clone()),
        Value::Var(name) => resolve_var_ref(name, vars),
        Value::Call(c) => resolve_call(c, span, vars)?,
    })
}

/// `--name` → a `LiveVar`. A layout var bakes its value into the reference so it
/// reads as a number at layout time (SPEC §11.2); a visual var stays live.
fn resolve_var_ref(name: &str, vars: &VarTable) -> ResolvedValue {
    let baked = match vars.get(name) {
        Some(VarEntry {
            kind: VarKind::Layout,
            value,
        }) => Some(Box::new(value.clone())),
        _ => None,
    };
    ResolvedValue::LiveVar {
        name: name.to_string(),
        raw: false,
        baked,
    }
}

fn resolve_call(c: &Call, span: Span, vars: &VarTable) -> Result<ResolvedValue, Error> {
    let mut args = Vec::with_capacity(c.args.len());
    for a in &c.args {
        args.push(resolve_scalar(a, span, vars)?);
    }
    Ok(ResolvedValue::Call(ResolvedCall {
        name: c.name.clone(),
        args,
    }))
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
                span: Span::empty(),
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
    fn layout_var_reference_bakes_its_number() {
        let mut vars = VarTable::new();
        vars.set("gap", VarKind::Layout, ResolvedValue::Number(30.0));
        let v = resolve_groups(&[vec![Value::Var("gap".into())]], Span::empty(), &vars).unwrap();
        assert_eq!(v.as_number(), Some(30.0));
    }

    #[test]
    fn call_resolves_its_arguments() {
        let v = resolve(&[vec![Value::Call(Call {
            name: "rgb".into(),
            args: vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)],
            span: Span::empty(),
        })]]);
        match v {
            ResolvedValue::Call(c) => {
                assert_eq!(c.name, "rgb");
                assert_eq!(c.args.len(), 3);
            }
            other => panic!("expected call, got {:?}", other),
        }
    }
}
