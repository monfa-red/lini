//! Stylesheet building: theme values, `--var` declarations, and `funcdef`s
//! folded into the var and function tables.

use super::*;
use crate::error::Code;

// ─────────────────────────── Variables ───────────────────────────

pub(super) fn apply_theme(vars: &mut VarTable, theme: &[(String, String)]) {
    for (name, raw) in theme {
        vars.set(name.clone(), parse_theme_value(raw));
    }
}

/// Parse a `--theme` value: a `light-dark()` / `rgba()` / `var()` call, a number,
/// a `#hex`, a bare ident, else raw CSS (a font stack stays verbatim).
fn parse_theme_value(raw: &str) -> ResolvedValue {
    let s = raw.trim();
    // Function form: NAME( ARGS ) — light-dark(), rgb/rgba/hsl/hsla(), var().
    if let Some(open) = s.find('(')
        && s.ends_with(')')
        && is_func_name(&s[..open])
    {
        let name = &s[..open];
        let inner = &s[open + 1..s.len() - 1];
        if name == "var" {
            let v = inner.trim();
            if let Some(rest) = v.strip_prefix("--lini-") {
                return ResolvedValue::LiveVar {
                    name: rest.to_string(),
                    raw: false,
                };
            }
            if let Some(rest) = v.strip_prefix("--") {
                return ResolvedValue::LiveVar {
                    name: rest.to_string(),
                    raw: true,
                };
            }
            return ResolvedValue::RawCss(s.to_string());
        }
        let args = split_top_commas(inner)
            .iter()
            .map(|a| parse_theme_value(a))
            .collect();
        return ResolvedValue::Call(ResolvedCall {
            name: name.to_string(),
            args,
        });
    }
    if let Ok(n) = s.parse::<f64>() {
        return ResolvedValue::Number(n);
    }
    if let Some(hex) = s.strip_prefix('#')
        && matches!(hex.len(), 3 | 6 | 8)
        && hex.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return ResolvedValue::Hex(hex.to_string());
    }
    if !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return ResolvedValue::Ident(s.to_string());
    }
    ResolvedValue::RawCss(s.to_string())
}

/// A CSS function name: letters/digits/`-`, starting with a letter (so a value
/// like `translate(…)` is a call, but a `#hex` or font stack is not).
fn is_func_name(s: &str) -> bool {
    s.bytes().next().is_some_and(|b| b.is_ascii_alphabetic())
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Split on top-level commas (ignoring commas inside nested parens), for the
/// arguments of a `light-dark()` / `rgba()` value.
fn split_top_commas(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

/// Apply `--name: value` declarations in source order (each sees the prior).
/// All vars are visual [SPEC 10.2]; a built-in `--lini-*` name keeps its
/// meaning, a new name is the author's.
pub(super) fn apply_var_decls(
    vars: &mut VarTable,
    file: &File,
    funcs: &FuncTable,
) -> Result<(), Error> {
    for item in &file.stylesheet {
        if let StyleItem::Var(d) = item {
            let value = resolve_groups(&d.groups, d.span, vars, funcs)?;
            vars.set(d.name.clone(), value);
        }
    }
    Ok(())
}

/// Parse the stylesheet's `funcdef`s into a [`FuncTable`] and reject reference
/// cycles [SPEC 10.7]. Arity and unknown-name errors surface at fold time.
pub(super) fn build_funcs(file: &File) -> Result<FuncTable, Error> {
    let mut parsed = Vec::new();
    for item in &file.stylesheet {
        if let StyleItem::Binding(f) = item {
            let body = Expr::parse(&f.body).map_err(|e| Error::at(f.span, e.0))?;
            parsed.push((f, body));
        }
    }
    let names: HashSet<&str> = parsed.iter().map(|(f, _)| f.name.as_str()).collect();
    // Edges to other user functions only (math builtins / params are not nodes).
    let graph: HashMap<&str, Vec<String>> = parsed
        .iter()
        .map(|(f, body)| {
            let refs = body
                .referenced_names()
                .into_iter()
                .filter(|n| names.contains(n.as_str()))
                .collect();
            (f.name.as_str(), refs)
        })
        .collect();
    for (f, _) in &parsed {
        detect_cycle(&f.name, &graph, &mut Vec::new())?;
    }

    let mut funcs = FuncTable::new();
    for (f, body) in parsed {
        funcs.insert(f.name.clone(), f.params.clone(), body);
    }
    Ok(funcs)
}

/// Depth-first cycle check over the function reference graph.
fn detect_cycle(
    name: &str,
    graph: &HashMap<&str, Vec<String>>,
    stack: &mut Vec<String>,
) -> Result<(), Error> {
    if stack.iter().any(|n| n == name) {
        stack.push(name.to_string());
        return Err(Error::at(
            crate::span::Span::empty(),
            format!("cycle in '{}'", stack.join(" → ")),
        )
        .code(Code::INHERIT_CYCLE));
    }
    stack.push(name.to_string());
    if let Some(refs) = graph.get(name) {
        for r in refs {
            detect_cycle(r, graph, stack)?;
        }
    }
    stack.pop();
    Ok(())
}
