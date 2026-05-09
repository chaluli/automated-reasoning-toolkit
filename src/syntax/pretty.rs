//! Context-aware pretty-printing for syntax handles.
//!
//! Typed handles (`ArithTerm`, `ArrTerm`, `Formula`, `Term`, `Expr`) cannot
//! implement `Display` directly because they don't carry their `Context`.
//! Instead, [`Context::display`] returns an [`ExprDisplay`] adapter that
//! borrows the context and implements `Display`.
//!
//! ```ignore
//! let s = format!("{}", ctx.display(formula));
//! ```
//!
//! Output uses ASCII infix notation:
//!   - boolean: `&&`, `||`, `!`, `=>`, `<=>`, quantifiers `forall x : t. ...`
//!   - arithmetic: `+`, `*`, `-`, `/`, `mod`, `floor(...)`
//!   - relations: `=`, `<`, `<=`
//!   - arrays: `a[i]`, `a[i := v]`
//!   - if-then-else: `(if c then t else e)`
//!
//! Bound de Bruijn variables are printed using the binder's name; free
//! variables print as `#<index>`.

use std::fmt;

use super::context::Context;
use super::expr::{
    ArithCmp, ArithTerm, ArithTermView, ArrTerm, ArrTermView, Atom, Expr, Formula, FormulaView,
    Proposition, Quantifier,
};

/// `Display` adapter produced by [`Context::display`].
#[derive(Debug)]
pub struct ExprDisplay<'a, C> {
    ctx: &'a Context<C>,
    expr: Expr<C>,
}

impl<C> Context<C> {
    /// Return a `Display` adapter that pretty-prints `e` against this context.
    ///
    /// Accepts any handle convertible to `Expr<C>` (`ArithTerm`, `ArrTerm`,
    /// `Formula`, `Term`, or `Expr` itself).
    pub fn display<E: Into<Expr<C>>>(&self, e: E) -> ExprDisplay<'_, C> {
        ExprDisplay {
            ctx: self,
            expr: e.into(),
        }
    }
}

impl<C> fmt::Display for ExprDisplay<'_, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut env: Vec<String> = Vec::new();
        fmt_expr(self.ctx, &mut env, self.expr, f)
    }
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

fn fmt_expr<C>(
    ctx: &Context<C>,
    env: &mut Vec<String>,
    e: Expr<C>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match e {
        Expr::ArithTerm(t) => fmt_arith(ctx, env, t, f),
        Expr::ArrTerm(t) => fmt_arr(ctx, env, t, f),
        Expr::Formula(p) => fmt_formula(ctx, env, p, f),
    }
}

// ---------------------------------------------------------------------------
// Arithmetic terms
// ---------------------------------------------------------------------------

fn fmt_arith<C>(
    ctx: &Context<C>,
    env: &mut Vec<String>,
    t: ArithTerm<C>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match ctx.view_arith_term(t) {
        ArithTermView::Real(q) => write!(f, "{q}"),
        ArithTermView::App(sym, args) => fmt_app(ctx, env, ctx.show_symbol(sym), &args, f),
        ArithTermView::Var(i, _) => fmt_var(env, i, f),
        ArithTermView::Add(cs) => fmt_nary_arith(ctx, env, &cs, " + ", f),
        ArithTermView::Mul(cs) => fmt_nary_arith(ctx, env, &cs, " * ", f),
        ArithTermView::Div(a, b) => fmt_binop_arith(ctx, env, a, " / ", b, f),
        ArithTermView::Mod(a, b) => fmt_binop_arith(ctx, env, a, " mod ", b, f),
        ArithTermView::Floor(a) => {
            write!(f, "floor(")?;
            fmt_arith(ctx, env, a, f)?;
            write!(f, ")")
        }
        ArithTermView::Neg(a) => {
            write!(f, "-")?;
            fmt_arith(ctx, env, a, f)
        }
        ArithTermView::Ite(c, then_, else_) => {
            write!(f, "(if ")?;
            fmt_formula(ctx, env, c, f)?;
            write!(f, " then ")?;
            fmt_arith(ctx, env, then_, f)?;
            write!(f, " else ")?;
            fmt_arith(ctx, env, else_, f)?;
            write!(f, ")")
        }
        ArithTermView::Select(arr, idx) => {
            fmt_arr(ctx, env, arr, f)?;
            write!(f, "[")?;
            fmt_arith(ctx, env, idx, f)?;
            write!(f, "]")
        }
    }
}

// ---------------------------------------------------------------------------
// Array terms
// ---------------------------------------------------------------------------

fn fmt_arr<C>(
    ctx: &Context<C>,
    env: &mut Vec<String>,
    t: ArrTerm<C>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match ctx.view_arr_term(t) {
        ArrTermView::App(sym, args) => fmt_app(ctx, env, ctx.show_symbol(sym), &args, f),
        ArrTermView::Var(i) => fmt_var(env, i, f),
        ArrTermView::Store(arr, idx, val) => {
            fmt_arr(ctx, env, arr, f)?;
            write!(f, "[")?;
            fmt_arith(ctx, env, idx, f)?;
            write!(f, " := ")?;
            fmt_arith(ctx, env, val, f)?;
            write!(f, "]")
        }
        ArrTermView::Ite(c, then_, else_) => {
            write!(f, "(if ")?;
            fmt_formula(ctx, env, c, f)?;
            write!(f, " then ")?;
            fmt_arr(ctx, env, then_, f)?;
            write!(f, " else ")?;
            fmt_arr(ctx, env, else_, f)?;
            write!(f, ")")
        }
    }
}

// ---------------------------------------------------------------------------
// Formulas
// ---------------------------------------------------------------------------

fn fmt_formula<C>(
    ctx: &Context<C>,
    env: &mut Vec<String>,
    p: Formula<C>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match ctx.view_formula(p) {
        FormulaView::True => write!(f, "true"),
        FormulaView::False => write!(f, "false"),
        FormulaView::And(cs) => fmt_nary_formula(ctx, env, &cs, " && ", "true", f),
        FormulaView::Or(cs) => fmt_nary_formula(ctx, env, &cs, " || ", "false", f),
        FormulaView::Not(q) => {
            write!(f, "!")?;
            fmt_formula(ctx, env, q, f)
        }
        FormulaView::Quantify(q, name, typ, body) => {
            let kw = match q {
                Quantifier::Forall => "forall",
                Quantifier::Exists => "exists",
            };
            write!(f, "({kw} {name} : {typ}. ")?;
            env.push(name);
            let res = fmt_formula(ctx, env, body, f);
            env.pop();
            res?;
            write!(f, ")")
        }
        FormulaView::Atom(Atom::Arith(cmp, a, b)) => {
            let op = match cmp {
                ArithCmp::Eq => " = ",
                ArithCmp::Leq => " <= ",
                ArithCmp::Lt => " < ",
            };
            fmt_binop_arith(ctx, env, a, op, b, f)
        }
        FormulaView::Atom(Atom::ArrEq(a, b)) => {
            write!(f, "(")?;
            fmt_arr(ctx, env, a, f)?;
            write!(f, " = ")?;
            fmt_arr(ctx, env, b, f)?;
            write!(f, ")")
        }
        FormulaView::Proposition(Proposition::Var(i)) => fmt_var(env, i, f),
        FormulaView::Proposition(Proposition::App(sym, args)) => {
            fmt_app(ctx, env, ctx.show_symbol(sym), &args, f)
        }
        FormulaView::Ite(c, then_, else_) => {
            write!(f, "(if ")?;
            fmt_formula(ctx, env, c, f)?;
            write!(f, " then ")?;
            fmt_formula(ctx, env, then_, f)?;
            write!(f, " else ")?;
            fmt_formula(ctx, env, else_, f)?;
            write!(f, ")")
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fmt_var(env: &[String], i: u32, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let idx = i as usize;
    if let Some(j) = env.len().checked_sub(idx + 1) {
        if let Some(name) = env.get(j) {
            return write!(f, "{name}");
        }
    }
    write!(f, "#{i}")
}

fn fmt_app<C>(
    ctx: &Context<C>,
    env: &mut Vec<String>,
    name: &str,
    args: &[Expr<C>],
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if args.is_empty() {
        return write!(f, "{name}");
    }
    write!(f, "{name}(")?;
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            write!(f, ", ")?;
        }
        fmt_expr(ctx, env, *arg, f)?;
    }
    write!(f, ")")
}

fn fmt_binop_arith<C>(
    ctx: &Context<C>,
    env: &mut Vec<String>,
    a: ArithTerm<C>,
    op: &str,
    b: ArithTerm<C>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    write!(f, "(")?;
    fmt_arith(ctx, env, a, f)?;
    write!(f, "{op}")?;
    fmt_arith(ctx, env, b, f)?;
    write!(f, ")")
}

fn fmt_nary_arith<C>(
    ctx: &Context<C>,
    env: &mut Vec<String>,
    terms: &[ArithTerm<C>],
    sep: &str,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    write!(f, "(")?;
    for (i, t) in terms.iter().enumerate() {
        if i > 0 {
            write!(f, "{sep}")?;
        }
        fmt_arith(ctx, env, *t, f)?;
    }
    write!(f, ")")
}

fn fmt_nary_formula<C>(
    ctx: &Context<C>,
    env: &mut Vec<String>,
    terms: &[Formula<C>],
    sep: &str,
    empty: &str,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if terms.is_empty() {
        return write!(f, "{empty}");
    }
    write!(f, "(")?;
    for (i, t) in terms.iter().enumerate() {
        if i > 0 {
            write!(f, "{sep}")?;
        }
        fmt_formula(ctx, env, *t, f)?;
    }
    write!(f, ")")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::types::{Typ, TypFo};

    #[derive(Debug, PartialEq, Eq)]
    enum TestCtx {}

    #[test]
    fn test_display_arith_basic() {
        let mut ctx = Context::<TestCtx>::new();
        let a = ctx.mk_int(2);
        let b = ctx.mk_int(3);
        let sum = ctx.mk_add(&[a, b]);
        assert_eq!(format!("{}", ctx.display(sum)), "(2 + 3)");
    }

    #[test]
    fn test_display_named_const_and_app() {
        let mut ctx = Context::<TestCtx>::new();
        let x = ctx.mk_symbol("x", Typ::Int);
        let f = ctx.mk_symbol("f", Typ::Fun(vec![TypFo::Int], TypFo::Int));
        let x_const = ctx.mk_const(x);
        let app = ctx.mk_app(f, &[x_const]);
        assert_eq!(format!("{}", ctx.display(app)), "f(x)");
    }

    #[test]
    fn test_display_formula_relations() {
        let mut ctx = Context::<TestCtx>::new();
        let one = ctx.mk_int(1);
        let two = ctx.mk_int(2);
        let lt = ctx.mk_lt(one, two);
        let leq = ctx.mk_leq(one, two);
        let eq = ctx.mk_eq(one, two);
        assert_eq!(format!("{}", ctx.display(lt)), "(1 < 2)");
        assert_eq!(format!("{}", ctx.display(leq)), "(1 <= 2)");
        assert_eq!(format!("{}", ctx.display(eq)), "(1 = 2)");
    }

    #[test]
    fn test_display_and_or_not() {
        let mut ctx = Context::<TestCtx>::new();
        let t = ctx.mk_true();
        let fls = ctx.mk_false();
        let and = ctx.mk_and(&[t, fls]);
        let or = ctx.mk_or(&[t, fls]);
        let not_t = ctx.mk_not(t);
        assert_eq!(format!("{}", ctx.display(and)), "(true && false)");
        assert_eq!(format!("{}", ctx.display(or)), "(true || false)");
        assert_eq!(format!("{}", ctx.display(not_t)), "!true");
    }

    #[test]
    fn test_display_quantifier_uses_bound_name() {
        let mut ctx = Context::<TestCtx>::new();
        let v0 = ctx.mk_var(0, TypFo::Int);
        let one = ctx.mk_int(1);
        // Build (v0 < 1) where v0 is the de Bruijn var, then bind it.
        if let Expr::ArithTerm(v) = v0 {
            let body = ctx.mk_lt(v, one);
            let quant = ctx.mk_forall("x", TypFo::Int, body);
            assert_eq!(
                format!("{}", ctx.display(quant)),
                "(forall x : int. (x < 1))"
            );
        } else {
            panic!("expected ArithTerm");
        }
    }

    #[test]
    fn test_display_array_select_store() {
        let mut ctx = Context::<TestCtx>::new();
        let a_sym = ctx.mk_symbol("a", Typ::Arr);
        let arr = match ctx.mk_const(a_sym) {
            Expr::ArrTerm(a) => a,
            _ => panic!("expected ArrTerm"),
        };
        let i = ctx.mk_int(0);
        let v = ctx.mk_int(7);
        let sel = ctx.mk_select(arr, i);
        let stored = ctx.mk_store(arr, i, v);
        assert_eq!(format!("{}", ctx.display(sel)), "a[0]");
        assert_eq!(format!("{}", ctx.display(stored)), "a[0 := 7]");
    }

    #[test]
    fn test_display_free_var() {
        let mut ctx = Context::<TestCtx>::new();
        let v = ctx.mk_var(2, TypFo::Int);
        assert_eq!(format!("{}", ctx.display(v)), "#2");
    }
}
