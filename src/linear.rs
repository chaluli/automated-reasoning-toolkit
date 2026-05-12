//! Linear algebra over the rationals.
//!
//! Provides:
//! - rational vectors and matrices ([`QQVector`], [`QQMatrix`], [`ZZVector`])
//!   as thin specialisations of the generic [`crate::ring`] types,
//! - solvers over rational matrices (nullspace, `solve`, rowspace
//!   intersection, pushout, division, Jordan chains),
//! - the [`vector_space`] submodule for working with subspaces of the
//!   rational vector space,
//! - and a bridge between rational vectors and the [`crate::syntax`]
//!   arithmetic-term representation (affine terms).

use std::collections::BTreeSet;
use std::fmt;

use crate::num::{QQ, ZZ};
use crate::ring::{self, Dim};
use crate::syntax::{ArithTerm, ArithTermView, Context, Expr, Symbol};

// ===========================================================================
// Type aliases
// ===========================================================================

/// A sparse vector with integer entries.
pub type ZZVector = ring::Vector<ZZ>;

/// A sparse vector with rational entries.
pub type QQVector = ring::Vector<QQ>;

/// A sparse matrix with rational entries.
pub type QQMatrix = ring::Matrix<QQ>;

// ===========================================================================
// Errors
// ===========================================================================

/// A system of linear equations has no solution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoSolution;

impl fmt::Display for NoSolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "no solution")
    }
}

impl std::error::Error for NoSolution {}

/// An arithmetic term contains a sub-expression that is not affine-linear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nonlinear;

impl fmt::Display for Nonlinear {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "term is not linear")
    }
}

impl std::error::Error for Nonlinear {}

// ===========================================================================
// Affine-term bridge
// ===========================================================================

/// Distinguished dimension reserved for the constant `1` coordinate of an
/// affine vector.
pub const CONST_DIM: Dim = -1;

/// Map a symbol to its dimension index. Always returns a non-negative
/// dimension distinct from [`CONST_DIM`].
pub fn dim_of_sym(sym: Symbol) -> Dim {
    #[expect(
        clippy::cast_possible_wrap,
        reason = "symbol indices fit in i32 in practice"
    )]
    let d = sym.as_u32() as Dim;
    d
}

/// Inverse of [`dim_of_sym`]. Returns `None` for the constant dimension.
pub fn sym_of_dim(dim: Dim) -> Option<Symbol> {
    if dim < 0 {
        None
    } else {
        #[expect(clippy::cast_sign_loss, reason = "dim >= 0 by the branch above")]
        let raw = dim as u32;
        Some(Symbol::from_raw(raw))
    }
}

/// Represent the rational `k` as an affine vector: a single entry of
/// magnitude `k` at the constant dimension.
pub fn const_linterm(k: QQ) -> QQVector {
    QQVector::of_term(k, CONST_DIM)
}

/// Number of nonzero coordinates of an affine vector.
pub fn linterm_size(v: &QQVector) -> usize {
    v.len()
}

/// If `v` represents a constant rational (every coordinate other than
/// [`CONST_DIM`] is zero), return that rational.
pub fn const_of_linterm(v: &QQVector) -> Option<QQ> {
    let (k, rest) = v.clone().pivot(CONST_DIM);
    if rest.is_zero() {
        Some(k)
    } else {
        None
    }
}

/// Convert an arithmetic term to its affine-vector representation.
///
/// Returns [`Nonlinear`] if `term` contains a sub-expression that cannot be
/// expressed as an affine combination of symbol values (uninterpreted
/// applications, bound variables, divisions by a non-constant, modulo /
/// floor over non-constants, array selects, and conditionals are all
/// rejected).
pub fn linterm_of<C>(ctx: &Context<C>, term: ArithTerm<C>) -> Result<QQVector, Nonlinear> {
    match ctx.view_arith_term(term) {
        ArithTermView::Real(q) => Ok(const_linterm(q)),
        ArithTermView::App(sym, args) => {
            if args.is_empty() {
                Ok(QQVector::of_term(QQ::one(), dim_of_sym(sym)))
            } else {
                Err(Nonlinear)
            }
        }
        ArithTermView::Var(_, _) => Err(Nonlinear),
        ArithTermView::Add(terms) => {
            let mut sum = QQVector::zero();
            for t in terms {
                sum = sum.add(&linterm_of(ctx, t)?);
            }
            Ok(sum)
        }
        ArithTermView::Mul(terms) => {
            let mut prod = const_linterm(QQ::one());
            for t in terms {
                let v = linterm_of(ctx, t)?;
                prod = mul_lin(&prod, &v)?;
            }
            Ok(prod)
        }
        ArithTermView::Div(a, b) => {
            let va = linterm_of(ctx, a)?;
            let vb = linterm_of(ctx, b)?;
            let k = constant_part(&vb)?;
            if k.is_zero() {
                Err(Nonlinear)
            } else {
                Ok(va.scalar_mul(&k.inverse()))
            }
        }
        ArithTermView::Mod(a, b) => {
            let va = linterm_of(ctx, a)?;
            let vb = linterm_of(ctx, b)?;
            let ka = constant_part(&va)?;
            let kb = constant_part(&vb)?;
            if kb.is_zero() {
                Err(Nonlinear)
            } else {
                Ok(const_linterm(ka.modulo(&kb)))
            }
        }
        ArithTermView::Floor(a) => {
            let va = linterm_of(ctx, a)?;
            let k = constant_part(&va)?;
            Ok(const_linterm(QQ::of_zz(&k.floor())))
        }
        ArithTermView::Neg(a) => Ok(linterm_of(ctx, a)?.negate()),
        ArithTermView::Select(_, _) | ArithTermView::Ite(_, _, _) => Err(Nonlinear),
    }
}

/// Extract the single constant coefficient of `v`, or [`Nonlinear`] if `v`
/// has any non-constant coordinate.
fn constant_part(v: &QQVector) -> Result<QQ, Nonlinear> {
    let (k, rest) = v.clone().pivot(CONST_DIM);
    if rest.is_zero() {
        Ok(k)
    } else {
        Err(Nonlinear)
    }
}

/// Multiply two affine vectors, succeeding only when at least one of them is
/// a constant.
fn mul_lin(x: &QQVector, y: &QQVector) -> Result<QQVector, Nonlinear> {
    if let Ok(kx) = constant_part(x) {
        Ok(y.scalar_mul(&kx))
    } else if let Ok(ky) = constant_part(y) {
        Ok(x.scalar_mul(&ky))
    } else {
        Err(Nonlinear)
    }
}

/// Inverse of [`linterm_of`]. Each non-constant coordinate `(d, c)` becomes
/// `c * sym_of_dim(d)`; the constant coordinate becomes a real literal.
/// Coordinates whose symbol is non-arithmetic are silently skipped.
pub fn of_linterm<C>(ctx: &mut Context<C>, v: &QQVector) -> ArithTerm<C> {
    let entries: Vec<(Dim, QQ)> = v.iter().map(|(d, c)| (d, c.clone())).collect();
    let mut summands: Vec<ArithTerm<C>> = Vec::new();
    for (dim, coeff) in entries {
        match sym_of_dim(dim) {
            Some(sym) => {
                if let Expr::ArithTerm(t) = ctx.mk_const(sym) {
                    let term = if coeff == QQ::one() {
                        t
                    } else {
                        let c = ctx.mk_real(coeff);
                        ctx.mk_mul(&[c, t])
                    };
                    summands.push(term);
                }
                // Non-arithmetic symbols are not representable here; skip.
            }
            None => summands.push(ctx.mk_real(coeff)),
        }
    }
    ctx.mk_add(&summands)
}

/// Evaluate an affine term under an interpretation that assigns each symbol
/// a rational value. The constant coordinate is evaluated to itself.
pub fn evaluate_linterm<F: Fn(Symbol) -> QQ>(interp: F, v: &QQVector) -> QQ {
    let mut sum = QQ::zero();
    for (dim, coeff) in v.iter() {
        let val = match sym_of_dim(dim) {
            Some(sym) => interp(sym).mul(coeff),
            None => coeff.clone(),
        };
        sum = sum.add(&val);
    }
    sum
}

/// Like [`evaluate_linterm`] but parameterised over raw dimensions; the
/// constant coordinate is fixed to evaluate to `1`.
pub fn evaluate_affine<F: Fn(Dim) -> QQ>(interp: F, v: &QQVector) -> QQ {
    let mut sum = QQ::zero();
    for (dim, coeff) in v.iter() {
        let val = if dim == CONST_DIM {
            coeff.clone()
        } else {
            interp(dim).mul(coeff)
        };
        sum = sum.add(&val);
    }
    sum
}

/// Build an arithmetic term from a rational vector, interpreting each
/// coordinate `(d, c)` as `c * term_of_dim(d)`.
pub fn term_of_vec<C, F: FnMut(&mut Context<C>, Dim) -> ArithTerm<C>>(
    ctx: &mut Context<C>,
    mut term_of_dim: F,
    v: &QQVector,
) -> ArithTerm<C> {
    let entries: Vec<(Dim, QQ)> = v.iter().map(|(d, c)| (d, c.clone())).collect();
    let mut summands: Vec<ArithTerm<C>> = Vec::new();
    for (dim, coeff) in entries {
        let dim_term = term_of_dim(ctx, dim);
        let coeff_term = ctx.mk_real(coeff);
        summands.push(ctx.mk_mul(&[coeff_term, dim_term]));
    }
    ctx.mk_add(&summands)
}

// ===========================================================================
// Solvers
// ===========================================================================

/// Reduce `mat` to row-echelon form by Gaussian elimination, leaving
/// `b_column` untouched as a pivot. Returns the list of `(pivot_column,
/// reduced_row)` pairs produced, in the order they were eliminated. Fails
/// with [`NoSolution`] if a row's only nonzero entry is in `b_column`.
fn row_echelon_form(mat: QQMatrix, b_column: Dim) -> Result<Vec<(Dim, QQVector)>, NoSolution> {
    let mut finished: Vec<(Dim, QQVector)> = Vec::new();
    let mut current = mat;
    while !current.is_zero() {
        let row_num = match current.min_row() {
            Some((r, _)) => r,
            None => break,
        };
        let (pivot_row, rest) = current.pivot(row_num);
        current = rest;

        let column = match pivot_row
            .iter()
            .find_map(|(d, _)| (d != b_column).then_some(d))
        {
            Some(c) => c,
            None => return Err(NoSolution),
        };

        let (cell, pivot_rest) = pivot_row.pivot(column);
        let scaled = pivot_rest.scalar_mul(&cell.inverse().negate());

        current = current.map_rows(|row| {
            let (coeff, row_rest) = row.pivot(column);
            row_rest.add(&scaled.scalar_mul(&coeff))
        });

        finished.push((column, scaled));
    }
    Ok(finished)
}

/// Compute a basis for the null space of `mat` projected onto `dimensions`:
/// the set of vectors `x` over `dimensions` such that `mat * x = 0`.
pub fn nullspace(mat: QQMatrix, dimensions: &[Dim]) -> Vec<QQVector> {
    let columns = mat.column_set();
    let max_col = columns.iter().copied().max().unwrap_or(0);
    let max_dim = dimensions.iter().copied().max().unwrap_or(0);
    let b_column = max_col.max(max_dim) + 1;

    let rr = match row_echelon_form(mat, b_column) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let pivots: BTreeSet<Dim> = rr.iter().map(|(c, _)| *c).collect();
    let free: Vec<Dim> = dimensions
        .iter()
        .copied()
        .filter(|d| !pivots.contains(d))
        .collect();

    free.into_iter()
        .map(|d| {
            let mut soln = QQVector::of_term(QQ::one(), d);
            for (lhs, rhs) in rr.iter().rev() {
                let prod = soln.dot(rhs);
                soln = soln.add_term(prod, *lhs);
            }
            soln
        })
        .collect()
}

/// Solve `mat * x = b` for `x`. Returns `None` if no solution exists.
pub fn solve(mat: &QQMatrix, b: &QQVector) -> Option<QQVector> {
    solve_inner(mat.clone(), b.clone()).ok()
}

fn solve_inner(mat: QQMatrix, b: QQVector) -> Result<QQVector, NoSolution> {
    let max_col = mat.column_set().iter().copied().max().unwrap_or(0);
    let b_column = max_col + 1;
    let augmented = mat.add_column(b_column, b);
    let rr = row_echelon_form(augmented, b_column)?;

    let mut soln = QQVector::of_term(QQ::one(), b_column);
    for (lhs, rhs) in rr.iter().rev() {
        let prod = soln.dot(rhs);
        soln = soln.add_term(prod, *lhs);
    }
    let (_, rest) = soln.pivot(b_column);
    Ok(rest.negate())
}

/// Given two matrices `A` and `B`, compute matrices `C` and `D` such that
/// `C*A = D*B` is a basis for the intersection of the rowspaces of `A` and
/// `B`.
pub fn intersect_rowspace(a: &QQMatrix, b: &QQMatrix) -> (QQMatrix, QQMatrix) {
    let mut mat_a = QQMatrix::zero();
    for (i, j, k) in a.entries() {
        mat_a = mat_a.add_entry(j, 2 * i, k.clone());
    }
    let mut mat = mat_a.clone();
    for (i, j, k) in b.entries() {
        mat = mat.add_entry(j, 2 * i + 1, k.negate());
    }

    let mut c = QQMatrix::zero();
    let mut d = QQMatrix::zero();
    let mut c_rows: Dim = 0;
    let mut d_rows: Dim = 0;
    let mut mat_rows = mat.rows_iter().map(|(i, _)| i).max().unwrap_or(-1) + 1;

    // Snapshot row indices: the loop body mutates `mat` but the iteration
    // walks the rows present at the start, as in the OCaml original.
    let snapshot: Vec<Dim> = mat.rows_iter().map(|(i, _)| i).collect();
    for col in snapshot {
        let row_a = mat_a.row(col);
        let candidate = mat.clone().add_row(mat_rows, row_a);
        if let Some(solution) = solve(&candidate, &QQVector::of_term(QQ::one(), mat_rows)) {
            let mut c_row = QQVector::zero();
            let mut d_row = QQVector::zero();
            for (i, entry) in solution.iter() {
                if i.rem_euclid(2) == 0 {
                    c_row = c_row.add_term(entry.clone(), i / 2);
                } else {
                    d_row = d_row.add_term(entry.clone(), i / 2);
                }
            }
            c = c.add_row(c_rows, c_row);
            d = d.add_row(d_rows, d_row);
            mat = candidate;
            c_rows += 1;
            d_rows += 1;
            mat_rows += 1;
        }
    }
    (c, d)
}

/// Compute the pushout of two matrices in the category of rational vector
/// spaces: a pair `(C, D)` such that `C*A = D*B` and that is universal among
/// all such pairs.
pub fn pushout(a: &QQMatrix, b: &QQMatrix) -> (QQMatrix, QQMatrix) {
    let a_t = a.transpose();
    let neg_b_t = b.scalar_mul(&QQ::of_i64(-1)).transpose();
    let interleaved = a_t.interlace_columns(&neg_b_t);
    let dims: Vec<Dim> = interleaved.column_set().into_iter().collect();
    let pairs = nullspace(interleaved, &dims);

    let mut c = QQMatrix::zero();
    let mut d = QQMatrix::zero();
    for (i, soln) in pairs.into_iter().enumerate() {
        let (sc, sd) = soln.deinterlace();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "row counts fit in i32 in practice"
        )]
        #[expect(
            clippy::cast_possible_wrap,
            reason = "row counts fit in i32 in practice"
        )]
        let id = i as Dim;
        c = c.add_row(id, sc);
        d = d.add_row(id, sd);
    }
    (c, d)
}

/// Given `A` and `B`, find `C` such that `C*B = A` if one exists. `C` exists
/// when the rowspace of `B` contains every row of `A`.
pub fn divide_right(a: &QQMatrix, b: &QQMatrix) -> Option<QQMatrix> {
    let b_t = b.transpose();
    let mut div = QQMatrix::zero();
    for (i, row) in a.rows_iter() {
        let solved = solve(&b_t, row)?;
        div = div.add_row(i, solved);
    }
    Some(div)
}

/// Given `A` and `B`, find `C` such that `B*C = A` if one exists. `C` exists
/// when the columnspace of `B` contains every column of `A`.
pub fn divide_left(a: &QQMatrix, b: &QQMatrix) -> Option<QQMatrix> {
    divide_right(&a.transpose(), &b.transpose()).map(|m| m.transpose())
}

/// The (left) Jordan chain generated by `v` with eigenvalue `lambda` of `a`:
/// the sequence `v_0, v_1, ..., v_n` where `v_0 = v`, `v_{i+1} = v_i*A -
/// lambda*v_i`, and `v_n*A - lambda*v_n = 0`.
pub fn jordan_chain(a: &QQMatrix, lambda: &QQ, v: QQVector) -> Vec<QQVector> {
    let mut chain: Vec<QQVector> = Vec::new();
    let mut current = v;
    loop {
        let residual = a.vector_left_mul(&current).sub(&current.scalar_mul(lambda));
        chain.push(current);
        if residual.is_zero() {
            return chain;
        }
        current = residual;
    }
}

/// Test whether `v` lies in the linear span of `basis`.
pub fn mem_vector_space(basis: &[QQVector], v: &QQVector) -> bool {
    let mut m = QQMatrix::zero();
    for (i, b) in basis.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "basis size fits in i32 in practice"
        )]
        #[expect(
            clippy::cast_possible_wrap,
            reason = "basis size fits in i32 in practice"
        )]
        let id = i as Dim;
        m = m.add_row(id, b.clone());
    }
    solve(&m.transpose(), v).is_some()
}

// ===========================================================================
// Vector spaces
// ===========================================================================

/// Operations on rational vector spaces, each represented as a list of basis
/// vectors.
pub mod vector_space {
    use super::{intersect_rowspace, mem_vector_space, Dim, QQMatrix, QQVector, QQ, ZZ};
    use std::collections::VecDeque;

    /// Empty vector space.
    pub fn empty() -> Vec<QQVector> {
        Vec::new()
    }

    /// Is `basis` the empty vector space?
    pub fn is_empty(basis: &[QQVector]) -> bool {
        basis.is_empty()
    }

    /// Membership in the span of `basis`.
    pub fn mem(basis: &[QQVector], v: &QQVector) -> bool {
        mem_vector_space(basis, v)
    }

    /// Is `u` a subspace of `v`?
    pub fn subspace(u: &[QQVector], v: &[QQVector]) -> bool {
        u.iter().all(|x| mem(v, x))
    }

    /// Do `u` and `v` span the same space?
    pub fn equal(u: &[QQVector], v: &[QQVector]) -> bool {
        subspace(u, v) && subspace(v, u)
    }

    /// Pack a basis into a matrix whose rows are the basis vectors.
    pub fn matrix_of(basis: &[QQVector]) -> QQMatrix {
        let mut m = QQMatrix::zero();
        for (i, b) in basis.iter().enumerate() {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "basis size fits in i32 in practice"
            )]
            #[expect(
                clippy::cast_possible_wrap,
                reason = "basis size fits in i32 in practice"
            )]
            let id = i as Dim;
            m = m.add_row(id, b.clone());
        }
        m
    }

    /// Treat the rows of `m` as a basis for the space they span. Assumes
    /// the rows are linearly independent.
    pub fn of_matrix(m: &QQMatrix) -> Vec<QQVector> {
        m.rows_iter().map(|(_, r)| r.clone()).collect()
    }

    /// Intersection of two vector spaces.
    pub fn intersect(u: &[QQVector], v: &[QQVector]) -> Vec<QQVector> {
        let mu = matrix_of(u);
        let mv = matrix_of(v);
        let (mc, _) = intersect_rowspace(&mu, &mv);
        of_matrix(&mc.mul(&mu))
    }

    /// A basis for the direct sum `u + v`.
    pub fn sum(u: &[QQVector], v: &[QQVector]) -> Vec<QQVector> {
        let mut result: Vec<QQVector> = v.to_vec();
        for x in u {
            if !mem(&result, x) {
                result.push(x.clone());
            }
        }
        result
    }

    /// A basis for the space spanned by `u`.
    pub fn basis(u: &[QQVector]) -> Vec<QQVector> {
        sum(u, &[])
    }

    /// A basis `w` such that `sum(diff(u, v), w) = u`.
    pub fn diff(u: &[QQVector], v: &[QQVector]) -> Vec<QQVector> {
        let mut result: Vec<QQVector> = Vec::new();
        for x in u {
            let mut combined = result.clone();
            combined.extend_from_slice(v);
            if !mem(&combined, x) {
                result.push(x.clone());
            }
        }
        result
    }

    /// The standard basis of n-dimensional space.
    pub fn standard_basis(dim: usize) -> Vec<QQVector> {
        (0..dim)
            .map(|i| {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "basis dimensions fit in i32 in practice"
                )]
                #[expect(
                    clippy::cast_possible_wrap,
                    reason = "basis dimensions fit in i32 in practice"
                )]
                let id = i as Dim;
                QQVector::of_term(QQ::one(), id)
            })
            .collect()
    }

    /// Simplify a basis by Gauss-Jordan elimination.
    pub fn simplify(basis: &[QQVector]) -> Vec<QQVector> {
        let mut xs: Vec<QQVector> = Vec::new();
        let mut ys: VecDeque<QQVector> = basis.iter().cloned().collect();
        while let Some(y) = ys.pop_front() {
            let dim = match y.min_support() {
                Some((d, _)) => d,
                None => continue,
            };
            let (coeff, rest) = y.pivot(dim);
            let normalized = rest.scalar_mul(&coeff.inverse()).add_term(QQ::one(), dim);

            for x in xs.iter_mut() {
                let c = x.coeff(dim);
                *x = x.add(&normalized.scalar_mul(&c.negate()));
            }
            for x in ys.iter_mut() {
                let c = x.coeff(dim);
                *x = x.add(&normalized.scalar_mul(&c.negate()));
            }

            xs.push(normalized);
        }
        xs
    }

    /// Scale each basis vector by the least common multiple of its
    /// denominators so that every coordinate becomes an integer.
    pub fn scale_integer(basis: &[QQVector]) -> Vec<QQVector> {
        basis
            .iter()
            .map(|v| {
                let common = v
                    .iter()
                    .fold(ZZ::one(), |acc, (_, c)| acc.lcm(&c.denominator()));
                v.scalar_mul(&QQ::of_zz(&common))
            })
            .collect()
    }

    /// Dimension of the spanned space (number of basis vectors).
    pub fn dimension(basis: &[QQVector]) -> usize {
        basis.len()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::Typ;

    fn qq(n: i64) -> QQ {
        QQ::of_i64(n)
    }

    // Phantom type for tests that need a Context.
    #[derive(Debug, PartialEq, Eq)]
    enum TestCtx {}

    #[test]
    fn affine_const_round_trip() {
        let v = const_linterm(qq(7));
        assert_eq!(const_of_linterm(&v), Some(qq(7)));
        assert_eq!(linterm_size(&v), 1);
    }

    #[test]
    fn dim_sym_round_trip() {
        let mut ctx = Context::<TestCtx>::new();
        let s = ctx.mk_symbol("x", Typ::Real);
        let d = dim_of_sym(s);
        assert_eq!(sym_of_dim(d), Some(s));
        assert_eq!(sym_of_dim(CONST_DIM), None);
    }

    fn arith_const<C>(ctx: &mut Context<C>, sym: Symbol) -> ArithTerm<C> {
        let Expr::ArithTerm(t) = ctx.mk_const(sym) else {
            panic!("expected ArithTerm for arithmetic symbol");
        };
        t
    }

    #[test]
    fn linterm_of_basic() {
        let mut ctx = Context::<TestCtx>::new();
        let x_sym = ctx.mk_symbol("x", Typ::Real);
        let y_sym = ctx.mk_symbol("y", Typ::Real);
        let x = arith_const(&mut ctx, x_sym);
        let y = arith_const(&mut ctx, y_sym);
        let two = ctx.mk_real(qq(2));
        let three = ctx.mk_real(qq(3));
        // 2*x + 3*y + 7
        let two_x = ctx.mk_mul(&[two, x]);
        let three_y = ctx.mk_mul(&[three, y]);
        let seven = ctx.mk_real(qq(7));
        let term = ctx.mk_add(&[two_x, three_y, seven]);

        let v = linterm_of(&ctx, term).expect("affine term");
        assert_eq!(v.coeff(dim_of_sym(x_sym)), qq(2));
        assert_eq!(v.coeff(dim_of_sym(y_sym)), qq(3));
        assert_eq!(v.coeff(CONST_DIM), qq(7));
    }

    #[test]
    fn linterm_of_nonlinear() {
        let mut ctx = Context::<TestCtx>::new();
        let x_sym = ctx.mk_symbol("x", Typ::Real);
        let x = arith_const(&mut ctx, x_sym);
        let xx = ctx.mk_mul(&[x, x]); // x * x — nonlinear
        assert_eq!(linterm_of(&ctx, xx), Err(Nonlinear));
    }

    #[test]
    fn solve_basic() {
        // [[1,2],[3,5]] * x = [4,11]  =>  x = [2,1]
        let a = QQMatrix::of_dense(&[vec![qq(1), qq(2)], vec![qq(3), qq(5)]]);
        let b = QQVector::of_term(qq(4), 0).add_term(qq(11), 1);
        let x = solve(&a, &b).expect("system has a solution");
        assert_eq!(x.coeff(0), qq(2));
        assert_eq!(x.coeff(1), qq(1));
        assert_eq!(a.vector_right_mul(&x), b);
    }

    #[test]
    fn solve_no_solution() {
        // [[1,1],[2,2]] * x = [1, 5] is inconsistent.
        let a = QQMatrix::of_dense(&[vec![qq(1), qq(1)], vec![qq(2), qq(2)]]);
        let b = QQVector::of_term(qq(1), 0).add_term(qq(5), 1);
        assert!(solve(&a, &b).is_none());
    }

    #[test]
    fn nullspace_basic() {
        // [[1,1,-1]] has nullspace { (x, y, x+y) }: basis (1,0,1) and (0,1,1)
        let a = QQMatrix::of_dense(&[vec![qq(1), qq(1), qq(-1)]]);
        let basis = nullspace(a.clone(), &[0, 1, 2]);
        // Each basis vector should yield a * v == 0.
        for v in &basis {
            let av = a.vector_right_mul(v);
            assert!(av.is_zero(), "expected null vector, got {av:?}");
        }
        assert_eq!(basis.len(), 2);
    }

    #[test]
    fn divide_right_basic() {
        // a*b^-1 when b is invertible.
        let a = QQMatrix::of_dense(&[vec![qq(2), qq(0)], vec![qq(0), qq(3)]]);
        let b = QQMatrix::of_dense(&[vec![qq(1), qq(0)], vec![qq(0), qq(1)]]);
        let c = divide_right(&a, &b).expect("b is invertible");
        assert_eq!(c.mul(&b), a);
    }

    #[test]
    fn vector_space_intersection() {
        use vector_space::*;
        // u: span{(1,0)}, v: span{(1,1)} -> intersect = {0}
        let u = vec![QQVector::of_term(qq(1), 0)];
        let v = vec![QQVector::of_term(qq(1), 0).add_term(qq(1), 1)];
        let inter = intersect(&u, &v);
        assert!(inter.is_empty() || inter.iter().all(|x| x.is_zero()));

        // u: span{(1,0), (0,1)}, v: span{(1,1)} -> intersect = v
        let u2 = vec![QQVector::of_term(qq(1), 0), QQVector::of_term(qq(1), 1)];
        let v2 = vec![QQVector::of_term(qq(1), 0).add_term(qq(1), 1)];
        let inter2 = intersect(&u2, &v2);
        assert!(equal(&inter2, &v2));
    }

    #[test]
    fn vector_space_simplify() {
        use vector_space::*;
        // (2, 0), (1, 1) -> simplifies to (1, 0), (0, 1)
        let basis = vec![
            QQVector::of_term(qq(2), 0),
            QQVector::of_term(qq(1), 0).add_term(qq(1), 1),
        ];
        let simp = simplify(&basis);
        assert_eq!(simp.len(), 2);
        // every standard basis vector is now reachable
        let e0 = QQVector::of_term(qq(1), 0);
        let e1 = QQVector::of_term(qq(1), 1);
        assert!(mem(&simp, &e0));
        assert!(mem(&simp, &e1));
    }

    #[test]
    fn vector_space_scale_integer() {
        let v = QQVector::of_term(QQ::of_frac(1, 2), 0).add_term(QQ::of_frac(1, 3), 1);
        let scaled = vector_space::scale_integer(&[v]);
        assert_eq!(scaled.len(), 1);
        let s = &scaled[0];
        assert_eq!(s.coeff(0), qq(3));
        assert_eq!(s.coeff(1), qq(2));
    }
}
