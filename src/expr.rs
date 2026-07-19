//! The `(…)` expression sub-language [SPEC 10.7] — a small, total compile-time
//! calculator: a Pratt parser and a tree-walk evaluator. It is the **only** place
//! operators live: the main parser captures a parenthesized region raw (its outer
//! parens stripped) and hands the body here to be re-lexed and folded.
//!
//! Values are numbers and points (`(x, y)`, for geometry) — no strings, no loops.
//! A body is `{ name = expr ; }* expr [ , expr ]`: leading `name = expr;` bindings are
//! locals (whole-body scope), the final expression is the value, and a top-level `,`
//! makes it a point. Functions defined in the stylesheet ([`FuncTable`]) are called by
//! name; the math library, `pi` / `e`, and the ambient sample parameter `u` are built in.
//!
//! Tokens come from the main [`crate::lexer`] in expression mode ([`lexer::lex_expr`]):
//! there is no second lexer — the parser below is a Pratt parser over that one stream.

use crate::lexer::{self, TokKind, Token};
use std::collections::HashMap;

/// A folded expression value [SPEC 10.7]: a number, or a point for geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Number(f64),
    Point(f64, f64),
}

/// An expression error, message-only; the caller attaches the source span.
#[derive(Debug, Clone)]
pub struct ExprError(pub String);

impl ExprError {
    fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

/// The ambient environment for an evaluation — the names available besides
/// locals, params, and the built-in constants. Geometry sampling injects `u`;
/// charts will inject `x` the same way.
pub type Env = HashMap<String, Value>;

// ─────────────────────────── AST ───────────────────────────

/// A parsed expression body: leading local bindings, then the value expression.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    locals: Vec<(String, Node)>,
    value: Node,
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Num(f64),
    /// A bare name — a local, a param, an ambient (`u`), a constant (`pi` / `e`),
    /// or a zero-arg function.
    Var(String),
    Neg(Box<Node>),
    Bin(BinOp, Box<Node>, Box<Node>),
    Ternary(Box<Node>, Box<Node>, Box<Node>),
    Call(String, Vec<Node>),
    Point(Box<Node>, Box<Node>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Binding power of an infix operator (higher binds tighter). `^` is handled in
/// `parse_power`, not here, so it is absent.
fn binop_bp(op: BinOp) -> u8 {
    match op {
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 1,
        BinOp::Add | BinOp::Sub => 2,
        BinOp::Mul | BinOp::Div => 3,
        BinOp::Pow => 4,
    }
}

// ─────────────────────────── Parser ───────────────────────────

/// A Pratt parser over the main lexer's tokens (in expression mode). It never
/// lexes — [`lex_expr`] already produced the stream; `src` backs the one
/// unexpected-token message so no token-printer is needed.
struct Parser<'a> {
    src: &'a str,
    toks: Vec<Token>,
    pos: usize,
}

impl Parser<'_> {
    fn kind(&self) -> Option<&TokKind> {
        self.toks.get(self.pos).map(|t| &t.kind)
    }

    fn kind_at(&self, n: usize) -> Option<&TokKind> {
        self.toks.get(self.pos + n).map(|t| &t.kind)
    }

    fn eat(&mut self, k: &TokKind) -> bool {
        if self.kind() == Some(k) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// `{ name = expr ; }* expr` — locals bind in order, the final expr is the value.
    fn parse_body(&mut self) -> Result<Expr, ExprError> {
        let mut locals = Vec::new();
        while matches!(self.kind(), Some(TokKind::Ident(_)))
            && matches!(self.kind_at(1), Some(TokKind::Assign))
        {
            let name = match self.kind() {
                Some(TokKind::Ident(s)) => s.clone(),
                _ => unreachable!(),
            };
            self.pos += 2; // ident '='
            let val = self.parse_ternary()?;
            if !self.eat(&TokKind::Semi) {
                return Err(ExprError::new("a local binding ends with ';'"));
            }
            locals.push((name, val));
        }
        let mut value = self.parse_ternary()?;
        // A top-level `,` makes the value a point `(x, y)` — the group's own parens
        // are stripped before it reaches here, so the comma is the point [SPEC 10.7].
        if self.eat(&TokKind::Comma) {
            let second = self.parse_ternary()?;
            value = Node::Point(Box::new(value), Box::new(second));
        }
        if self.pos != self.toks.len() {
            return Err(ExprError::new("trailing tokens after the expression"));
        }
        Ok(Expr { locals, value })
    }

    fn parse_ternary(&mut self) -> Result<Node, ExprError> {
        let cond = self.parse_binary(0)?;
        if self.eat(&TokKind::Question) {
            let a = self.parse_ternary()?;
            if !self.eat(&TokKind::Colon) {
                return Err(ExprError::new("a ternary 'cond ? a : b' needs ':'"));
            }
            let b = self.parse_ternary()?;
            Ok(Node::Ternary(Box::new(cond), Box::new(a), Box::new(b)))
        } else {
            Ok(cond)
        }
    }

    fn parse_binary(&mut self, min_bp: u8) -> Result<Node, ExprError> {
        let mut left = self.parse_unary()?;
        while let Some(op) = self.peek_binop() {
            let bp = binop_bp(op);
            if bp < min_bp {
                break;
            }
            self.pos += 1;
            let right = self.parse_binary(bp + 1)?; // all left-associative
            left = Node::Bin(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    /// The infix operators handled by `parse_binary` — `^` is right-associative
    /// and binds tighter than unary `-`, so it lives in `parse_power`.
    fn peek_binop(&self) -> Option<BinOp> {
        Some(match self.kind()? {
            TokKind::Plus => BinOp::Add,
            TokKind::Minus => BinOp::Sub,
            TokKind::Star => BinOp::Mul,
            TokKind::Slash => BinOp::Div,
            TokKind::EqEq => BinOp::Eq,
            TokKind::Ne => BinOp::Ne,
            TokKind::Lt => BinOp::Lt,
            TokKind::Le => BinOp::Le,
            TokKind::Gt => BinOp::Gt,
            TokKind::Ge => BinOp::Ge,
            _ => return None,
        })
    }

    fn parse_unary(&mut self) -> Result<Node, ExprError> {
        if self.eat(&TokKind::Minus) {
            Ok(Node::Neg(Box::new(self.parse_unary()?)))
        } else {
            self.parse_power()
        }
    }

    /// `atom ^ unary` — `^` binds tighter than unary `-` (so `-2^2` is `-(2^2)`)
    /// and is right-associative (`2^3^2` is `2^(3^2)`).
    fn parse_power(&mut self) -> Result<Node, ExprError> {
        let base = self.parse_atom()?;
        if self.eat(&TokKind::Caret) {
            let exp = self.parse_unary()?;
            Ok(Node::Bin(BinOp::Pow, Box::new(base), Box::new(exp)))
        } else {
            Ok(base)
        }
    }

    fn parse_atom(&mut self) -> Result<Node, ExprError> {
        let Some(tok) = self.toks.get(self.pos).cloned() else {
            return Err(ExprError::new("unexpected end of an expression"));
        };
        self.pos += 1;
        match tok.kind {
            TokKind::Number(n) => Ok(Node::Num(n)),
            TokKind::Ident(name) => {
                if self.eat(&TokKind::LParen) {
                    let mut args = Vec::new();
                    if !matches!(self.kind(), Some(TokKind::RParen)) {
                        args.push(self.parse_ternary()?);
                        while self.eat(&TokKind::Comma) {
                            args.push(self.parse_ternary()?);
                        }
                    }
                    if !self.eat(&TokKind::RParen) {
                        return Err(ExprError::new(format!("call to '{name}' needs ')'")));
                    }
                    Ok(Node::Call(name, args))
                } else {
                    Ok(Node::Var(name))
                }
            }
            // `( a )` groups; `( a , b )` is a point (for geometry).
            TokKind::LParen => {
                let first = self.parse_ternary()?;
                if self.eat(&TokKind::Comma) {
                    let second = self.parse_ternary()?;
                    if !self.eat(&TokKind::RParen) {
                        return Err(ExprError::new("a point '(x, y)' needs ')'"));
                    }
                    Ok(Node::Point(Box::new(first), Box::new(second)))
                } else if self.eat(&TokKind::RParen) {
                    Ok(first)
                } else {
                    Err(ExprError::new("expected ',' or ')' after '('"))
                }
            }
            _ => Err(ExprError::new(format!(
                "unexpected '{}' in an expression",
                &self.src[tok.span.start..tok.span.end]
            ))),
        }
    }
}

// ─────────────────────────── Functions ───────────────────────────

struct Func {
    params: Vec<String>,
    body: Expr,
}

/// The stylesheet's defined functions [SPEC 10.7], built at resolve time.
#[derive(Default)]
pub struct FuncTable {
    funcs: HashMap<String, Func>,
}

impl FuncTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Define `name(params) body`. A later definition replaces an earlier one.
    pub fn insert(&mut self, name: String, params: Vec<String>, body: Expr) {
        self.funcs.insert(name, Func { params, body });
    }

    /// Whether a user function by this name is defined.
    pub fn contains(&self, name: &str) -> bool {
        self.funcs.contains_key(name)
    }

    fn get(&self, name: &str) -> Option<&Func> {
        self.funcs.get(name)
    }
}

// ─────────────────────────── Evaluation ───────────────────────────

impl Expr {
    /// Parse a raw expression body (a group's inner text) into an expression.
    pub fn parse(src: &str) -> Result<Expr, ExprError> {
        let toks = lexer::lex_expr(src).map_err(|e| ExprError(e.message))?;
        Parser { src, toks, pos: 0 }.parse_body()
    }

    /// Whether this reads without a `(…)` group — a lone number, name, or call
    /// (optionally negated), with no locals, operators, ternary, or point [SPEC 10.7].
    /// Drives the formatter's choice between `x = 5` and `x = (a + b)`.
    pub fn is_atomic(&self) -> bool {
        self.locals.is_empty()
            && matches!(
                self.value,
                Node::Num(_) | Node::Var(_) | Node::Call(..) | Node::Neg(_)
            )
    }

    /// Fold to a value against the ambient environment and the function table.
    pub fn eval(&self, ambient: &Env, funcs: &FuncTable) -> Result<Value, ExprError> {
        eval_body(self, HashMap::new(), ambient, funcs)
    }

    /// Every name referenced as a bare var or a call — the resolver's cycle-check
    /// and geometry-detection input (it keeps the names that are `u` or functions).
    pub fn referenced_names(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (_, n) in &self.locals {
            collect_names(n, &mut out);
        }
        collect_names(&self.value, &mut out);
        out
    }
}

/// Evaluate a math builtin or user-function `name(args)` with already-evaluated
/// args, in a plain-value context (empty ambient — no `u`). For geometry, eval the
/// whole expression with a `u`-bearing ambient instead.
pub fn call(funcs: &FuncTable, name: &str, args: &[Value]) -> Result<Value, ExprError> {
    eval_call(name, args, &Env::new(), funcs)
}

/// Sample `expr` with the ambient `name` bound to each of `values` in turn. The one
/// seam for ambient sampling: parametric `points:` binds `u` (0→1, [SPEC 10.7]), a
/// chart `fn:` binds `x` over its domain [SPEC 14.3].
pub fn sample(
    expr: &Expr,
    name: &str,
    values: &[f64],
    funcs: &FuncTable,
) -> Result<Vec<Value>, ExprError> {
    let mut out = Vec::with_capacity(values.len());
    for &v in values {
        let mut env = Env::new();
        env.insert(name.to_string(), Value::Number(v));
        out.push(expr.eval(&env, funcs)?);
    }
    Ok(out)
}

fn collect_names(node: &Node, out: &mut Vec<String>) {
    match node {
        Node::Num(_) => {}
        Node::Var(name) => out.push(name.clone()),
        Node::Neg(e) => collect_names(e, out),
        Node::Bin(_, a, b) | Node::Point(a, b) => {
            collect_names(a, out);
            collect_names(b, out);
        }
        Node::Ternary(c, a, b) => {
            collect_names(c, out);
            collect_names(a, out);
            collect_names(b, out);
        }
        Node::Call(name, args) => {
            out.push(name.clone());
            for a in args {
                collect_names(a, out);
            }
        }
    }
}

/// Evaluate a body, with `base` pre-seeding the frame (a function's params).
/// Locals bind in order over the same frame; the final expression is the value.
fn eval_body(
    expr: &Expr,
    base: HashMap<String, Value>,
    ambient: &Env,
    funcs: &FuncTable,
) -> Result<Value, ExprError> {
    let mut vars = base;
    for (name, node) in &expr.locals {
        let v = eval_node(node, &vars, ambient, funcs)?;
        vars.insert(name.clone(), v);
    }
    eval_node(&expr.value, &vars, ambient, funcs)
}

fn eval_node(
    node: &Node,
    vars: &HashMap<String, Value>,
    ambient: &Env,
    funcs: &FuncTable,
) -> Result<Value, ExprError> {
    match node {
        Node::Num(n) => Ok(Value::Number(*n)),
        Node::Var(name) => eval_var(name, vars, ambient, funcs),
        Node::Neg(e) => Ok(Value::Number(-as_num(
            eval_node(e, vars, ambient, funcs)?,
            "negation",
        )?)),
        Node::Bin(op, a, b) => {
            let x = as_num(eval_node(a, vars, ambient, funcs)?, "an operator")?;
            let y = as_num(eval_node(b, vars, ambient, funcs)?, "an operator")?;
            Ok(Value::Number(apply_binop(*op, x, y)))
        }
        Node::Ternary(c, a, b) => {
            let cond = as_num(eval_node(c, vars, ambient, funcs)?, "a condition")?;
            if cond != 0.0 {
                eval_node(a, vars, ambient, funcs)
            } else {
                eval_node(b, vars, ambient, funcs)
            }
        }
        Node::Point(a, b) => {
            let x = as_num(eval_node(a, vars, ambient, funcs)?, "a point coordinate")?;
            let y = as_num(eval_node(b, vars, ambient, funcs)?, "a point coordinate")?;
            Ok(Value::Point(x, y))
        }
        Node::Call(name, args) => {
            let mut argv = Vec::with_capacity(args.len());
            for a in args {
                argv.push(eval_node(a, vars, ambient, funcs)?);
            }
            eval_call(name, &argv, ambient, funcs)
        }
    }
}

fn eval_var(
    name: &str,
    vars: &HashMap<String, Value>,
    ambient: &Env,
    funcs: &FuncTable,
) -> Result<Value, ExprError> {
    if let Some(v) = vars.get(name).or_else(|| ambient.get(name)) {
        return Ok(*v);
    }
    match name {
        "pi" => Ok(Value::Number(std::f64::consts::PI)),
        "e" => Ok(Value::Number(std::f64::consts::E)),
        _ => match funcs.get(name) {
            // A zero-arg function used bare is a named constant.
            Some(f) if f.params.is_empty() => eval_body(&f.body, HashMap::new(), ambient, funcs),
            Some(f) => Err(ExprError::new(format!(
                "'{name}' takes {} argument(s), got 0",
                f.params.len()
            ))),
            None => Err(ExprError::new(format!(
                "unknown name '{name}' in an expression"
            ))),
        },
    }
}

fn eval_call(
    name: &str,
    args: &[Value],
    ambient: &Env,
    funcs: &FuncTable,
) -> Result<Value, ExprError> {
    // The math library takes precedence over a user function of the same name.
    if let Some(arity) = math_arity(name) {
        return eval_math(name, args, arity);
    }
    match funcs.get(name) {
        Some(f) => {
            if f.params.len() != args.len() {
                return Err(ExprError::new(format!(
                    "'{name}' takes {} argument(s), got {}",
                    f.params.len(),
                    args.len()
                )));
            }
            let base = f.params.iter().cloned().zip(args.iter().copied()).collect();
            eval_body(&f.body, base, ambient, funcs)
        }
        None => Err(ExprError::new(format!("unknown function '{name}'"))),
    }
}

/// Fixed arity of a math builtin, or `None` for the variadic / user case.
/// `min` / `max` (variadic) and `clamp` (3) are handled in `eval_math`.
fn math_arity(name: &str) -> Option<Arity> {
    Some(match name {
        "sin" | "cos" | "tan" | "exp" | "ln" | "log" | "sqrt" | "abs" | "floor" | "round" => {
            Arity::One
        }
        "pow" => Arity::Two,
        "clamp" => Arity::Three,
        "min" | "max" => Arity::Variadic,
        _ => return None,
    })
}

enum Arity {
    One,
    Two,
    Three,
    Variadic,
}

fn eval_math(name: &str, args: &[Value], arity: Arity) -> Result<Value, ExprError> {
    let n = |i: usize| as_num(args[i], name);
    match arity {
        Arity::One => {
            check_arity(name, args, 1)?;
            let x = n(0)?;
            Ok(Value::Number(match name {
                "sin" => x.sin(),
                "cos" => x.cos(),
                "tan" => x.tan(),
                "exp" => x.exp(),
                "ln" => x.ln(),
                "log" => x.log10(),
                "sqrt" => x.sqrt(),
                "abs" => x.abs(),
                "floor" => x.floor(),
                "round" => x.round(),
                _ => unreachable!(),
            }))
        }
        Arity::Two => {
            check_arity(name, args, 2)?;
            Ok(Value::Number(n(0)?.powf(n(1)?)))
        }
        Arity::Three => {
            check_arity(name, args, 3)?;
            // clamp(x, lo, hi); `max(lo).min(hi)` never panics on lo > hi.
            Ok(Value::Number(n(0)?.max(n(1)?).min(n(2)?)))
        }
        Arity::Variadic => {
            if args.is_empty() {
                return Err(ExprError::new(format!(
                    "'{name}' needs at least one argument"
                )));
            }
            let mut acc = n(0)?;
            for i in 1..args.len() {
                let v = n(i)?;
                acc = if name == "min" {
                    acc.min(v)
                } else {
                    acc.max(v)
                };
            }
            Ok(Value::Number(acc))
        }
    }
}

fn check_arity(name: &str, args: &[Value], want: usize) -> Result<(), ExprError> {
    if args.len() == want {
        Ok(())
    } else {
        Err(ExprError::new(format!(
            "'{name}' takes {want} argument(s), got {}",
            args.len()
        )))
    }
}

fn apply_binop(op: BinOp, x: f64, y: f64) -> f64 {
    let b = |cond: bool| if cond { 1.0 } else { 0.0 };
    match op {
        BinOp::Add => x + y,
        BinOp::Sub => x - y,
        BinOp::Mul => x * y,
        BinOp::Div => x / y,
        BinOp::Pow => x.powf(y),
        BinOp::Eq => b(x == y),
        BinOp::Ne => b(x != y),
        BinOp::Lt => b(x < y),
        BinOp::Le => b(x <= y),
        BinOp::Gt => b(x > y),
        BinOp::Ge => b(x >= y),
    }
}

fn as_num(v: Value, ctx: &str) -> Result<f64, ExprError> {
    match v {
        Value::Number(n) => Ok(n),
        Value::Point(..) => Err(ExprError::new(format!("{ctx} needs a number, got a point"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(src: &str) -> f64 {
        let env = Env::new();
        let funcs = FuncTable::new();
        match Expr::parse(src)
            .expect("parse")
            .eval(&env, &funcs)
            .expect("eval")
        {
            Value::Number(n) => n,
            v => panic!("expected a number, got {v:?}"),
        }
    }

    fn err(src: &str) -> String {
        let env = Env::new();
        let funcs = FuncTable::new();
        match Expr::parse(src).and_then(|e| e.eval(&env, &funcs)) {
            Err(ExprError(m)) => m,
            Ok(v) => panic!("expected an error, got {v:?}"),
        }
    }

    #[test]
    fn arithmetic_and_precedence() {
        assert_eq!(eval("8 * 2"), 16.0);
        assert_eq!(eval("2 + 3 * 4"), 14.0);
        assert_eq!(eval("(2 + 3) * 4"), 20.0);
        assert_eq!(eval("10 / 4"), 2.5);
    }

    #[test]
    fn power_is_right_assoc_and_binds_over_unary_minus() {
        assert_eq!(eval("2 ^ 3 ^ 2"), 512.0); // 2^(3^2)
        assert_eq!(eval("-2 ^ 2"), -4.0); // -(2^2)
        assert_eq!(eval("2 ^ -1"), 0.5);
        assert_eq!(eval("100 * 1.2 ^ 2"), 144.0);
    }

    #[test]
    fn comparisons_and_ternary() {
        assert_eq!(eval("1 < 2 ? 10 : 20"), 10.0);
        assert_eq!(eval("3 < 2 ? 10 : 20"), 20.0);
        assert_eq!(eval("2 == 2"), 1.0);
        assert_eq!(eval("2 != 2"), 0.0);
    }

    #[test]
    fn constants_and_scientific() {
        assert!((eval("pi") - std::f64::consts::PI).abs() < 1e-12);
        assert_eq!(eval("1e3"), 1000.0);
        assert_eq!(eval("1.5e-2"), 0.015);
    }

    #[test]
    fn locals_bind_in_order() {
        assert_eq!(eval("r = 40; n = 8; 2 * r / n"), 10.0);
        assert_eq!(eval("a = 2; b = a + 1; a * b"), 6.0);
    }

    #[test]
    fn math_library() {
        assert_eq!(eval("abs(-5)"), 5.0);
        assert_eq!(eval("floor(3.7)"), 3.0);
        assert_eq!(eval("min(3, 1, 2)"), 1.0);
        assert_eq!(eval("max(3, 1, 2)"), 3.0);
        assert_eq!(eval("clamp(12, 0, 10)"), 10.0);
        assert_eq!(eval("clamp(-4, 0, 10)"), 0.0);
        assert_eq!(eval("pow(2, 10)"), 1024.0);
        assert!((eval("sin(0)")).abs() < 1e-12);
    }

    #[test]
    fn ident_minus_is_subtraction_not_a_dash_name() {
        assert_eq!(eval("r = 5; r-1"), 4.0);
    }

    #[test]
    fn user_functions() {
        let mut funcs = FuncTable::new();
        funcs.insert(
            "scale".into(),
            vec!["n".into()],
            Expr::parse("100 * 1.2 ^ n").unwrap(),
        );
        funcs.insert("unit".into(), vec![], Expr::parse("8").unwrap());
        let env = Env::new();
        let num = |s: &str| match Expr::parse(s).unwrap().eval(&env, &funcs).unwrap() {
            Value::Number(n) => n,
            v => panic!("{v:?}"),
        };
        assert_eq!(num("scale(0)"), 100.0);
        assert!((num("scale(2)") - 144.0).abs() < 1e-9);
        assert_eq!(num("unit"), 8.0); // a zero-arg function used bare
        assert_eq!(num("unit() + 4"), 12.0);
        assert_eq!(num("scale(2) + 4"), num("scale(2)") + 4.0);
    }

    #[test]
    fn points_for_geometry() {
        let mut env = Env::new();
        env.insert("u".into(), Value::Number(0.5));
        let funcs = FuncTable::new();
        let v = Expr::parse("(u * 300, 20)")
            .unwrap()
            .eval(&env, &funcs)
            .unwrap();
        assert_eq!(v, Value::Point(150.0, 20.0));
    }

    #[test]
    fn errors() {
        assert!(err("foo + 1").contains("unknown name 'foo'"));
        assert!(err("sqrt(1, 2)").contains("takes 1"));
        assert!(err("(1, 2) + 3").contains("needs a number, got a point"));
        assert!(err("r = 1 2").contains("ends with ';'"));
    }
}
