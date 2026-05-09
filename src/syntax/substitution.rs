use super::context::Context;
use super::expr::{Expr, ExprId, Formula, SExprNode};
use super::symbol::{Symbol, SymbolMap};
use super::types::TypFo;

/// A rewriter function that transforms an expression within a context.
type Rewriter<'a, C> = dyn Fn(&mut Context<C>, Expr<C>) -> Expr<C> + 'a;

// ---------------------------------------------------------------------------
// De Bruijn index shifting ("decapture")
// ---------------------------------------------------------------------------

/// Shift free variables in `id` by `incr`, treating variables below `depth` as bound.
///
/// This prevents variable capture when substituting under binders.
fn decapture<C>(ctx: &mut Context<C>, depth: u32, incr: i32, id: ExprId) -> ExprId {
    match ctx.node(id).clone() {
        SExprNode::Exists(name, typ, body) => {
            let body2 = decapture(ctx, depth + 1, incr, body);
            ctx.intern_node(SExprNode::Exists(name, typ, body2))
        }
        SExprNode::Forall(name, typ, body) => {
            let body2 = decapture(ctx, depth + 1, incr, body);
            ctx.intern_node(SExprNode::Forall(name, typ, body2))
        }
        SExprNode::Var(v, typ) => {
            if v < depth {
                id // bound variable — don't shift
            } else {
                let new_v = (v as i32 + incr) as u32;
                ctx.intern_node(SExprNode::Var(new_v, typ))
            }
        }
        node => {
            let shifted = map_children(node, |child| decapture(ctx, depth, incr, child));
            ctx.intern_node(shifted)
        }
    }
}

// ---------------------------------------------------------------------------
// Substitute de Bruijn variables
// ---------------------------------------------------------------------------

/// Replace each free de Bruijn variable `i` of type `typ` with `subst(i, typ)`.
///
/// Capture is avoided by shifting the substituted expression under binders.
pub fn substitute<C, F>(ctx: &mut Context<C>, subst: &F, expr: Expr<C>) -> Expr<C>
where
    F: Fn(u32, TypFo) -> Expr<C>,
{
    let id = substitute_rec(ctx, subst, 0, expr.id());
    ctx.id_to_expr_pub(id)
}

fn substitute_rec<C, F>(ctx: &mut Context<C>, subst: &F, depth: u32, id: ExprId) -> ExprId
where
    F: Fn(u32, TypFo) -> Expr<C>,
{
    match ctx.node(id).clone() {
        SExprNode::Exists(name, typ, body) => {
            let body2 = substitute_rec(ctx, subst, depth + 1, body);
            ctx.intern_node(SExprNode::Exists(name, typ, body2))
        }
        SExprNode::Forall(name, typ, body) => {
            let body2 = substitute_rec(ctx, subst, depth + 1, body);
            ctx.intern_node(SExprNode::Forall(name, typ, body2))
        }
        SExprNode::Var(v, typ) => {
            if v < depth {
                id // bound variable
            } else {
                let replacement = subst(v - depth, typ);
                decapture(ctx, 0, depth as i32, replacement.id())
            }
        }
        node => {
            let rebuilt = map_children(node, |child| substitute_rec(ctx, subst, depth, child));
            ctx.intern_node(rebuilt)
        }
    }
}

// ---------------------------------------------------------------------------
// Substitute constant symbols
// ---------------------------------------------------------------------------

/// Replace each occurrence of a constant symbol `s` (nullary application) with `subst(s)`.
///
/// Function applications (non-nullary) are not affected. Capture is avoided.
pub fn substitute_const<C, F>(ctx: &mut Context<C>, subst: &F, expr: Expr<C>) -> Expr<C>
where
    F: Fn(Symbol) -> Expr<C>,
{
    let id = substitute_const_rec(ctx, subst, 0, expr.id());
    ctx.id_to_expr_pub(id)
}

fn substitute_const_rec<C, F>(ctx: &mut Context<C>, subst: &F, depth: u32, id: ExprId) -> ExprId
where
    F: Fn(Symbol) -> Expr<C>,
{
    match ctx.node(id).clone() {
        SExprNode::Exists(name, typ, body) => {
            let body2 = substitute_const_rec(ctx, subst, depth + 1, body);
            ctx.intern_node(SExprNode::Exists(name, typ, body2))
        }
        SExprNode::Forall(name, typ, body) => {
            let body2 = substitute_const_rec(ctx, subst, depth + 1, body);
            ctx.intern_node(SExprNode::Forall(name, typ, body2))
        }
        SExprNode::App(sym, ref args) if args.is_empty() => {
            let replacement = subst(sym);
            decapture(ctx, 0, depth as i32, replacement.id())
        }
        node => {
            let rebuilt =
                map_children(node, |child| substitute_const_rec(ctx, subst, depth, child));
            ctx.intern_node(rebuilt)
        }
    }
}

// ---------------------------------------------------------------------------
// Substitute from a symbol map
// ---------------------------------------------------------------------------

/// Replace each constant symbol in `map`'s domain with the corresponding expression.
///
/// Symbols not in the map are left unchanged.
pub fn substitute_map<C>(ctx: &mut Context<C>, map: &SymbolMap<Expr<C>>, expr: Expr<C>) -> Expr<C> {
    // Build a snapshot of what mk_const would produce for symbols not in the map.
    // We pre-compute these to avoid borrowing ctx inside the closure.
    let id = substitute_map_rec(ctx, map, 0, expr.id());
    ctx.id_to_expr_pub(id)
}

fn substitute_map_rec<C>(
    ctx: &mut Context<C>,
    map: &SymbolMap<Expr<C>>,
    depth: u32,
    id: ExprId,
) -> ExprId {
    match ctx.node(id).clone() {
        SExprNode::Exists(name, typ, body) => {
            let body2 = substitute_map_rec(ctx, map, depth + 1, body);
            ctx.intern_node(SExprNode::Exists(name, typ, body2))
        }
        SExprNode::Forall(name, typ, body) => {
            let body2 = substitute_map_rec(ctx, map, depth + 1, body);
            ctx.intern_node(SExprNode::Forall(name, typ, body2))
        }
        SExprNode::App(sym, ref args) if args.is_empty() => {
            if let Some(&replacement) = map.get(&sym) {
                decapture(ctx, 0, depth as i32, replacement.id())
            } else {
                id // symbol not in map — leave unchanged
            }
        }
        node => {
            let rebuilt = map_children(node, |child| substitute_map_rec(ctx, map, depth, child));
            ctx.intern_node(rebuilt)
        }
    }
}

// ---------------------------------------------------------------------------
// Rewrite
// ---------------------------------------------------------------------------

/// Rewrite an expression by applying transformations on the way down and/or up the tree.
///
/// - `down` is applied to each node before its children are rewritten.
/// - `up` is applied to each node after its children have been rewritten.
pub fn rewrite<C>(
    ctx: &mut Context<C>,
    down: Option<&Rewriter<'_, C>>,
    up: Option<&Rewriter<'_, C>>,
    expr: Expr<C>,
) -> Expr<C> {
    let id = rewrite_rec(ctx, down, up, expr.id());
    ctx.id_to_expr_pub(id)
}

fn rewrite_rec<C>(
    ctx: &mut Context<C>,
    down: Option<&Rewriter<'_, C>>,
    up: Option<&Rewriter<'_, C>>,
    id: ExprId,
) -> ExprId {
    // Apply down
    let id = if let Some(down_fn) = down {
        let expr = ctx.id_to_expr_pub(id);
        down_fn(ctx, expr).id()
    } else {
        id
    };

    // Recursively rewrite children
    let node = ctx.node(id).clone();
    let rebuilt = map_children(node, |child| rewrite_rec(ctx, down, up, child));
    let rebuilt_id = ctx.intern_node(rebuilt);

    // Apply up
    if let Some(up_fn) = up {
        let expr = ctx.id_to_expr_pub(rebuilt_id);
        up_fn(ctx, expr).id()
    } else {
        rebuilt_id
    }
}

// ---------------------------------------------------------------------------
// NNF (negation normal form)
// ---------------------------------------------------------------------------

/// Convert a formula to negation normal form.
///
/// Pushes negations inward so that `Not` only appears directly above atoms.
pub fn nnf<C>(ctx: &mut Context<C>, formula: Formula<C>) -> Formula<C> {
    let id = nnf_rec(ctx, formula.id);
    Formula {
        id,
        _phantom: std::marker::PhantomData,
    }
}

fn nnf_rec<C>(ctx: &mut Context<C>, id: ExprId) -> ExprId {
    match ctx.node(id).clone() {
        SExprNode::Not(inner_id) => nnf_not(ctx, inner_id),
        SExprNode::And(cs) => {
            let new_cs: Vec<ExprId> = cs.into_iter().map(|c| nnf_rec(ctx, c)).collect();
            ctx.intern_node(SExprNode::And(new_cs))
        }
        SExprNode::Or(cs) => {
            let new_cs: Vec<ExprId> = cs.into_iter().map(|c| nnf_rec(ctx, c)).collect();
            ctx.intern_node(SExprNode::Or(new_cs))
        }
        SExprNode::Exists(name, typ, body) => {
            let body2 = nnf_rec(ctx, body);
            ctx.intern_node(SExprNode::Exists(name, typ, body2))
        }
        SExprNode::Forall(name, typ, body) => {
            let body2 = nnf_rec(ctx, body);
            ctx.intern_node(SExprNode::Forall(name, typ, body2))
        }
        SExprNode::Ite(cond, then_, else_) => {
            let then2 = nnf_rec(ctx, then_);
            let else2 = nnf_rec(ctx, else_);
            ctx.intern_node(SExprNode::Ite(cond, then2, else2))
        }
        _ => id, // atoms, propositions — leave unchanged
    }
}

/// Push a negation inward (the core NNF step).
fn nnf_not<C>(ctx: &mut Context<C>, inner_id: ExprId) -> ExprId {
    match ctx.node(inner_id).clone() {
        // ¬¬φ → φ (then continue NNF)
        SExprNode::Not(phi) => nnf_rec(ctx, phi),

        // ¬(φ ∧ ψ) → ¬φ ∨ ¬ψ
        SExprNode::And(cs) => {
            let negated: Vec<ExprId> = cs
                .into_iter()
                .map(|c| {
                    let neg = ctx.intern_node(SExprNode::Not(c));
                    nnf_rec(ctx, neg)
                })
                .collect();
            ctx.intern_node(SExprNode::Or(negated))
        }

        // ¬(φ ∨ ψ) → ¬φ ∧ ¬ψ
        SExprNode::Or(cs) => {
            let negated: Vec<ExprId> = cs
                .into_iter()
                .map(|c| {
                    let neg = ctx.intern_node(SExprNode::Not(c));
                    nnf_rec(ctx, neg)
                })
                .collect();
            ctx.intern_node(SExprNode::And(negated))
        }

        // ¬(a ≤ b) → b < a
        SExprNode::Leq(a, b) => ctx.intern_node(SExprNode::Lt(b, a)),

        // ¬(a = b) → a < b ∨ b < a
        SExprNode::Eq(a, b) => {
            let lt1 = ctx.intern_node(SExprNode::Lt(a, b));
            let lt2 = ctx.intern_node(SExprNode::Lt(b, a));
            ctx.intern_node(SExprNode::Or(vec![lt1, lt2]))
        }

        // ¬(a < b) → b ≤ a
        SExprNode::Lt(a, b) => ctx.intern_node(SExprNode::Leq(b, a)),

        // ¬∃x.φ → ∀x.¬φ
        SExprNode::Exists(name, typ, body) => {
            let neg_body = ctx.intern_node(SExprNode::Not(body));
            let nnf_body = nnf_rec(ctx, neg_body);
            ctx.intern_node(SExprNode::Forall(name, typ, nnf_body))
        }

        // ¬∀x.φ → ∃x.¬φ
        SExprNode::Forall(name, typ, body) => {
            let neg_body = ctx.intern_node(SExprNode::Not(body));
            let nnf_body = nnf_rec(ctx, neg_body);
            ctx.intern_node(SExprNode::Exists(name, typ, nnf_body))
        }

        // ¬(ite c t e) → ite c (¬t) (¬e)
        SExprNode::Ite(cond, then_, else_) => {
            let neg_then = ctx.intern_node(SExprNode::Not(then_));
            let neg_else = ctx.intern_node(SExprNode::Not(else_));
            let nnf_then = nnf_rec(ctx, neg_then);
            let nnf_else = nnf_rec(ctx, neg_else);
            ctx.intern_node(SExprNode::Ite(cond, nnf_then, nnf_else))
        }

        // Everything else (atoms, propositions): keep the negation
        _ => ctx.intern_node(SExprNode::Not(inner_id)),
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Map a function over the children of an `SExprNode`, producing a new node.
fn map_children(node: SExprNode, mut f: impl FnMut(ExprId) -> ExprId) -> SExprNode {
    match node {
        SExprNode::Real(_) | SExprNode::True | SExprNode::False | SExprNode::Var(..) => node,
        SExprNode::Add(cs) => SExprNode::Add(cs.into_iter().map(&mut f).collect()),
        SExprNode::Mul(cs) => SExprNode::Mul(cs.into_iter().map(&mut f).collect()),
        SExprNode::And(cs) => SExprNode::And(cs.into_iter().map(&mut f).collect()),
        SExprNode::Or(cs) => SExprNode::Or(cs.into_iter().map(&mut f).collect()),
        SExprNode::App(sym, cs) => SExprNode::App(sym, cs.into_iter().map(&mut f).collect()),
        SExprNode::Div(a, b) => SExprNode::Div(f(a), f(b)),
        SExprNode::Mod(a, b) => SExprNode::Mod(f(a), f(b)),
        SExprNode::Eq(a, b) => SExprNode::Eq(f(a), f(b)),
        SExprNode::Leq(a, b) => SExprNode::Leq(f(a), f(b)),
        SExprNode::Lt(a, b) => SExprNode::Lt(f(a), f(b)),
        SExprNode::ArrEq(a, b) => SExprNode::ArrEq(f(a), f(b)),
        SExprNode::Select(a, b) => SExprNode::Select(f(a), f(b)),
        SExprNode::Floor(a) => SExprNode::Floor(f(a)),
        SExprNode::Neg(a) => SExprNode::Neg(f(a)),
        SExprNode::Not(a) => SExprNode::Not(f(a)),
        SExprNode::Exists(name, typ, body) => SExprNode::Exists(name, typ, f(body)),
        SExprNode::Forall(name, typ, body) => SExprNode::Forall(name, typ, f(body)),
        SExprNode::Store(a, b, c) => SExprNode::Store(f(a), f(b), f(c)),
        SExprNode::Ite(a, b, c) => SExprNode::Ite(f(a), f(b), f(c)),
    }
}
