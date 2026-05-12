//! Sparse vectors and matrices over a ring.
//!
//! Both [`Vector`] and [`Matrix`] are sparse: only nonzero entries are stored.
//! The invariant "no stored entry equals zero" is preserved by every public
//! operation, so equality and `is_zero` are correct without explicit
//! normalisation.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::algebra::{AbelianGroup, Ring};

/// Signed integer dimension used to index vectors and matrix rows/columns.
pub type Dim = i32;

// ===========================================================================
// Vector
// ===========================================================================

/// A sparse vector with coefficients drawn from a ring.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Vector<R> {
    entries: BTreeMap<Dim, R>,
}

impl<R: fmt::Debug> fmt::Debug for Vector<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.entries.iter()).finish()
    }
}

impl<R: Ring> Default for Vector<R> {
    fn default() -> Self {
        Self::zero()
    }
}

impl<R: Ring> Vector<R> {
    /// The zero vector.
    pub fn zero() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// A vector whose only nonzero entry is `coeff` at position `dim`.
    pub fn of_term(coeff: R, dim: Dim) -> Self {
        if R::is_zero(&coeff) {
            Self::zero()
        } else {
            let mut entries = BTreeMap::new();
            entries.insert(dim, coeff);
            Self { entries }
        }
    }

    /// Number of nonzero entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True iff every entry is zero.
    pub fn is_zero(&self) -> bool {
        self.entries.is_empty()
    }

    /// Alias for [`is_zero`](Self::is_zero); a sparse vector with no stored
    /// entries is exactly the zero vector.
    pub fn is_empty(&self) -> bool {
        self.is_zero()
    }

    /// Coefficient at `dim` (zero if no entry is stored there).
    pub fn coeff(&self, dim: Dim) -> R {
        self.entries.get(&dim).cloned().unwrap_or_else(R::zero)
    }

    /// Iterate `(dim, &coeff)` for each nonzero entry in ascending `dim` order.
    pub fn iter(&self) -> impl Iterator<Item = (Dim, &R)> + '_ {
        self.entries.iter().map(|(d, v)| (*d, v))
    }

    /// The smallest-dim nonzero entry, or `None` if the vector is zero.
    pub fn min_support(&self) -> Option<(Dim, &R)> {
        self.entries.iter().next().map(|(d, v)| (*d, v))
    }

    /// `self + other`.
    pub fn add(&self, other: &Self) -> Self {
        let mut entries = self.entries.clone();
        for (d, v) in &other.entries {
            match entries.entry(*d) {
                Entry::Occupied(mut o) => {
                    let new = R::add(o.get(), v);
                    if R::is_zero(&new) {
                        o.remove();
                    } else {
                        *o.get_mut() = new;
                    }
                }
                Entry::Vacant(slot) => {
                    slot.insert(v.clone());
                }
            }
        }
        Self { entries }
    }

    /// `-self`.
    pub fn negate(&self) -> Self {
        Self {
            entries: self
                .entries
                .iter()
                .map(|(d, v)| (*d, R::negate(v)))
                .collect(),
        }
    }

    /// `self - other`.
    pub fn sub(&self, other: &Self) -> Self {
        self.add(&other.negate())
    }

    /// `k * self`.
    pub fn scalar_mul(&self, k: &R) -> Self {
        if R::is_zero(k) {
            return Self::zero();
        }
        if R::equal(k, &R::one()) {
            return self.clone();
        }
        let entries = self
            .entries
            .iter()
            .filter_map(|(d, v)| {
                let prod = R::mul(k, v);
                if R::is_zero(&prod) {
                    None
                } else {
                    Some((*d, prod))
                }
            })
            .collect();
        Self { entries }
    }

    /// Inner product `sum_i self_i * other_i`.
    pub fn dot(&self, other: &Self) -> R {
        let mut acc = R::zero();
        for (d, v) in &self.entries {
            if let Some(w) = other.entries.get(d) {
                acc = R::add(&acc, &R::mul(v, w));
            }
        }
        acc
    }

    /// Add `coeff` to the entry at position `dim`.
    pub fn add_term(mut self, coeff: R, dim: Dim) -> Self {
        if R::is_zero(&coeff) {
            return self;
        }
        match self.entries.entry(dim) {
            Entry::Occupied(mut o) => {
                let new = R::add(o.get(), &coeff);
                if R::is_zero(&new) {
                    o.remove();
                } else {
                    *o.get_mut() = new;
                }
            }
            Entry::Vacant(slot) => {
                slot.insert(coeff);
            }
        }
        self
    }

    /// Replace the entry at `dim` with `coeff` (or remove it if zero).
    pub fn set(mut self, dim: Dim, coeff: R) -> Self {
        if R::is_zero(&coeff) {
            self.entries.remove(&dim);
        } else {
            self.entries.insert(dim, coeff);
        }
        self
    }

    /// Remove the entry at `dim`. Returns the coefficient that was there
    /// (`R::zero()` if none) and the vector with that position cleared.
    pub fn pivot(mut self, dim: Dim) -> (R, Self) {
        let v = self.entries.remove(&dim).unwrap_or_else(R::zero);
        (v, self)
    }

    /// Remove the smallest-dim entry. Returns `Err(self)` if `self` is zero.
    pub fn pop_min(mut self) -> Result<((Dim, R), Self), Self> {
        match self.entries.pop_first() {
            Some(pair) => Ok((pair, self)),
            None => Err(self),
        }
    }

    /// Apply `f` to each `(dim, coeff)`; entries that become zero are dropped.
    pub fn map<F>(self, mut f: F) -> Self
    where
        F: FnMut(Dim, R) -> R,
    {
        let entries = self
            .entries
            .into_iter()
            .filter_map(|(d, v)| {
                let new = f(d, v);
                if R::is_zero(&new) {
                    None
                } else {
                    Some((d, new))
                }
            })
            .collect();
        Self { entries }
    }

    /// Pointwise binary operation, treating missing entries as zero on either
    /// side. Entries that become zero are dropped.
    pub fn merge<F>(self, mut other: Self, mut f: F) -> Self
    where
        F: FnMut(Dim, R, R) -> R,
    {
        let mut a = self.entries;
        let keys: BTreeSet<Dim> = a.keys().chain(other.entries.keys()).copied().collect();
        let mut entries = BTreeMap::new();
        for k in keys {
            let va = a.remove(&k).unwrap_or_else(R::zero);
            let vb = other.entries.remove(&k).unwrap_or_else(R::zero);
            let v = f(k, va, vb);
            if !R::is_zero(&v) {
                entries.insert(k, v);
            }
        }
        Self { entries }
    }

    /// Combine `self` and `other` into a single vector with `self`'s entries
    /// at even positions and `other`'s at odd positions.
    pub fn interlace(&self, other: &Self) -> Self {
        let mut entries = BTreeMap::new();
        for (d, v) in &self.entries {
            entries.insert(2 * d, v.clone());
        }
        for (d, v) in &other.entries {
            entries.insert(2 * d + 1, v.clone());
        }
        Self { entries }
    }

    /// Inverse of [`interlace`](Self::interlace).
    pub fn deinterlace(&self) -> (Self, Self) {
        let mut even = BTreeMap::new();
        let mut odd = BTreeMap::new();
        for (d, v) in &self.entries {
            if d.rem_euclid(2) == 0 {
                even.insert(d / 2, v.clone());
            } else {
                odd.insert(d / 2, v.clone());
            }
        }
        (Self { entries: even }, Self { entries: odd })
    }
}

impl<R: Ring + fmt::Display> fmt::Display for Vector<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        let mut first = true;
        for (dim, v) in self.iter() {
            if !first {
                write!(f, ", ")?;
            }
            first = false;
            write!(f, "{dim}:{v}")?;
        }
        write!(f, "]")
    }
}

impl<R: Ring> FromIterator<(Dim, R)> for Vector<R> {
    fn from_iter<I: IntoIterator<Item = (Dim, R)>>(iter: I) -> Self {
        let mut v = Self::zero();
        for (d, c) in iter {
            v = v.add_term(c, d);
        }
        v
    }
}

impl<R: Ring> IntoIterator for Vector<R> {
    type Item = (Dim, R);
    type IntoIter = std::collections::btree_map::IntoIter<Dim, R>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<R: Ring> AbelianGroup for Vector<R> {
    fn zero() -> Self {
        Self::zero()
    }
    fn add(a: &Self, b: &Self) -> Self {
        a.add(b)
    }
    fn negate(a: &Self) -> Self {
        a.negate()
    }
    fn is_zero(a: &Self) -> bool {
        a.is_zero()
    }
}

// ===========================================================================
// Matrix
// ===========================================================================

/// A sparse matrix with coefficients drawn from a ring.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Matrix<R> {
    rows: BTreeMap<Dim, Vector<R>>,
}

impl<R: fmt::Debug> fmt::Debug for Matrix<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.rows.iter()).finish()
    }
}

impl<R: Ring> Default for Matrix<R> {
    fn default() -> Self {
        Self::zero()
    }
}

impl<R: Ring> Matrix<R> {
    /// The all-zero matrix.
    pub fn zero() -> Self {
        Self {
            rows: BTreeMap::new(),
        }
    }

    pub fn is_zero(&self) -> bool {
        self.rows.is_empty()
    }

    /// The identity matrix restricted to the given diagonal positions.
    pub fn identity<I: IntoIterator<Item = Dim>>(dims: I) -> Self {
        let mut m = Self::zero();
        for d in dims {
            m = m.add_entry(d, d, R::one());
        }
        m
    }

    /// Row `i` (zero vector if absent).
    pub fn row(&self, i: Dim) -> Vector<R> {
        self.rows.get(&i).cloned().unwrap_or_else(Vector::zero)
    }

    /// Column `j` as a vector indexed by row position.
    pub fn column(&self, j: Dim) -> Vector<R> {
        let mut col = Vector::zero();
        for (i, row) in &self.rows {
            col = col.add_term(row.coeff(j), *i);
        }
        col
    }

    /// Iterate `(i, &row)` for each nonzero row in ascending row order.
    pub fn rows_iter(&self) -> impl Iterator<Item = (Dim, &Vector<R>)> + '_ {
        self.rows.iter().map(|(i, r)| (*i, r))
    }

    /// The smallest-row-index nonzero row, or `None`.
    pub fn min_row(&self) -> Option<(Dim, &Vector<R>)> {
        self.rows.iter().next().map(|(i, r)| (*i, r))
    }

    /// `(i,j)` entry (zero if absent).
    pub fn entry(&self, i: Dim, j: Dim) -> R {
        match self.rows.get(&i) {
            Some(row) => row.coeff(j),
            None => R::zero(),
        }
    }

    /// Iterate `(i, j, &coeff)` for every nonzero entry.
    pub fn entries(&self) -> impl Iterator<Item = (Dim, Dim, &R)> + '_ {
        self.rows
            .iter()
            .flat_map(|(i, row)| row.iter().map(move |(j, v)| (*i, j, v)))
    }

    /// Set of row indices with a nonzero row.
    pub fn row_set(&self) -> BTreeSet<Dim> {
        self.rows.keys().copied().collect()
    }

    /// Set of column indices with at least one nonzero entry.
    pub fn column_set(&self) -> BTreeSet<Dim> {
        let mut s = BTreeSet::new();
        for row in self.rows.values() {
            for (j, _) in row.iter() {
                s.insert(j);
            }
        }
        s
    }

    pub fn nb_rows(&self) -> usize {
        self.rows.len()
    }

    pub fn nb_columns(&self) -> usize {
        self.column_set().len()
    }

    /// `self + other`.
    pub fn add(&self, other: &Self) -> Self {
        let mut rows = self.rows.clone();
        for (i, row) in &other.rows {
            match rows.entry(*i) {
                Entry::Occupied(mut o) => {
                    let merged = o.get().add(row);
                    if merged.is_zero() {
                        o.remove();
                    } else {
                        *o.get_mut() = merged;
                    }
                }
                Entry::Vacant(slot) => {
                    slot.insert(row.clone());
                }
            }
        }
        Self { rows }
    }

    /// `k * self`.
    pub fn scalar_mul(&self, k: &R) -> Self {
        if R::is_zero(k) {
            return Self::zero();
        }
        if R::equal(k, &R::one()) {
            return self.clone();
        }
        let rows = self
            .rows
            .iter()
            .filter_map(|(i, row)| {
                let scaled = row.scalar_mul(k);
                if scaled.is_zero() {
                    None
                } else {
                    Some((*i, scaled))
                }
            })
            .collect();
        Self { rows }
    }

    /// `self * other`.
    pub fn mul(&self, other: &Self) -> Self {
        let other_t = other.transpose();
        let mut result = Self::zero();
        for (i, row) in &self.rows {
            let mut out_row = Vector::zero();
            for (j, col) in &other_t.rows {
                out_row = out_row.add_term(row.dot(col), *j);
            }
            result = result.add_row(*i, out_row);
        }
        result
    }

    /// Transpose.
    pub fn transpose(&self) -> Self {
        let mut t = Self::zero();
        for (i, j, k) in self.entries() {
            t = t.add_entry(j, i, k.clone());
        }
        t
    }

    /// Add `v` (as a row vector) to row `i`.
    pub fn add_row(mut self, i: Dim, v: Vector<R>) -> Self {
        if v.is_zero() {
            return self;
        }
        match self.rows.entry(i) {
            Entry::Occupied(mut o) => {
                let merged = o.get().add(&v);
                if merged.is_zero() {
                    o.remove();
                } else {
                    *o.get_mut() = merged;
                }
            }
            Entry::Vacant(slot) => {
                slot.insert(v);
            }
        }
        self
    }

    /// Add `v` (as a column vector) to column `j`.
    pub fn add_column(mut self, j: Dim, v: Vector<R>) -> Self {
        for (i, coeff) in v {
            self = self.add_entry(i, j, coeff);
        }
        self
    }

    /// Add `k` to entry `(i, j)`.
    pub fn add_entry(self, i: Dim, j: Dim, k: R) -> Self {
        self.add_row(i, Vector::of_term(k, j))
    }

    /// Remove row `i`. Returns the removed row (zero if absent) and the rest.
    pub fn pivot(mut self, i: Dim) -> (Vector<R>, Self) {
        let row = self.rows.remove(&i).unwrap_or_else(Vector::zero);
        (row, self)
    }

    /// Remove column `j`. Returns the removed column (as a vector indexed by
    /// row) and the matrix with that column cleared.
    pub fn pivot_column(self, j: Dim) -> (Vector<R>, Self) {
        let mut col = Vector::zero();
        let mut rows = BTreeMap::new();
        for (i, row) in self.rows {
            let (entry, rest) = row.pivot(j);
            col = col.add_term(entry, i);
            if !rest.is_zero() {
                rows.insert(i, rest);
            }
        }
        (col, Self { rows })
    }

    /// Apply `f` to each row, dropping rows that become zero.
    pub fn map_rows<F>(self, mut f: F) -> Self
    where
        F: FnMut(Vector<R>) -> Vector<R>,
    {
        let rows = self
            .rows
            .into_iter()
            .filter_map(|(i, r)| {
                let r = f(r);
                if r.is_zero() {
                    None
                } else {
                    Some((i, r))
                }
            })
            .collect();
        Self { rows }
    }

    /// `self * v`, treating `v` as a column vector.
    pub fn vector_right_mul(&self, v: &Vector<R>) -> Vector<R> {
        let mut result = Vector::zero();
        for (i, row) in &self.rows {
            result = result.add_term(row.dot(v), *i);
        }
        result
    }

    /// `v^T * self`, treating `v` as a column vector.
    pub fn vector_left_mul(&self, v: &Vector<R>) -> Vector<R> {
        let mut result = Vector::zero();
        for (i, scalar) in v.iter() {
            let scaled = self.row(i).scalar_mul(scalar);
            result = result.add(&scaled);
        }
        result
    }

    /// Build a matrix from a dense row-major representation.
    pub fn of_dense(rows: &[Vec<R>]) -> Self {
        let mut m = Self::zero();
        for (i, row) in rows.iter().enumerate() {
            let mut v = Vector::zero();
            for (j, entry) in row.iter().enumerate() {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "dense matrix dimensions fit in i32 in practice"
                )]
                let jd = j as Dim;
                v = v.add_term(entry.clone(), jd);
            }
            #[expect(
                clippy::cast_possible_truncation,
                reason = "dense matrix dimensions fit in i32 in practice"
            )]
            let id = i as Dim;
            m = m.add_row(id, v);
        }
        m
    }

    /// Materialise this sparse matrix as a dense `rows`-by-`cols` array.
    /// Entries outside the stored support are filled with zero.
    pub fn dense_of(&self, rows: usize, cols: usize) -> Vec<Vec<R>> {
        (0..rows)
            .map(|i| {
                (0..cols)
                    .map(|j| {
                        #[expect(
                            clippy::cast_possible_truncation,
                            reason = "dimensions fit in i32 in practice"
                        )]
                        let id = i as Dim;
                        #[expect(
                            clippy::cast_possible_truncation,
                            reason = "dimensions fit in i32 in practice"
                        )]
                        let jd = j as Dim;
                        self.entry(id, jd)
                    })
                    .collect()
            })
            .collect()
    }

    /// Build a matrix whose `i`-th row is the `i`-th element of `rows`.
    pub fn of_rows<I: IntoIterator<Item = Vector<R>>>(rows: I) -> Self {
        let mut m = Self::zero();
        for (i, v) in rows.into_iter().enumerate() {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "row counts fit in i32 in practice"
            )]
            let id = i as Dim;
            m = m.add_row(id, v);
        }
        m
    }

    /// Matrix exponentiation by a non-negative integer power. The exponent
    /// zero produces the identity matrix restricted to the dimensions that
    /// appear in `self`'s row or column support.
    pub fn pow(&self, n: u32) -> Self {
        let dims: BTreeSet<Dim> = self.row_set().union(&self.column_set()).copied().collect();
        let id = Self::identity(dims);
        if n == 0 {
            return id;
        }
        let mut result = id;
        let mut base = self.clone();
        let mut k = n;
        while k > 0 {
            if k & 1 == 1 {
                result = result.mul(&base);
            }
            k >>= 1;
            if k > 0 {
                base = base.mul(&base);
            }
        }
        result
    }

    /// Combine two matrices column-wise, interleaving `self`'s columns at even
    /// positions and `other`'s columns at odd positions.
    pub fn interlace_columns(&self, other: &Self) -> Self {
        let row_keys: BTreeSet<Dim> = self.rows.keys().chain(other.rows.keys()).copied().collect();
        let mut m = Self::zero();
        for i in row_keys {
            m = m.add_row(i, self.row(i).interlace(&other.row(i)));
        }
        m
    }
}

impl<R: Ring + fmt::Display> fmt::Display for Matrix<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_zero() {
            return write!(f, "0");
        }
        let cols = self.column_set();
        for (i, row) in &self.rows {
            write!(f, "row {i}: [")?;
            let mut first = true;
            for j in &cols {
                if !first {
                    write!(f, ", ")?;
                }
                first = false;
                write!(f, "{}", row.coeff(*j))?;
            }
            writeln!(f, "]")?;
        }
        Ok(())
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::num::QQ;

    fn qq(n: i64) -> QQ {
        QQ::of_i64(n)
    }

    #[test]
    fn vector_basic() {
        let u: Vector<QQ> = Vector::of_term(qq(2), 0).add_term(qq(3), 1);
        let v: Vector<QQ> = Vector::of_term(qq(5), 1);
        assert_eq!(u.coeff(0), qq(2));
        assert_eq!(u.coeff(1), qq(3));
        assert_eq!(u.add(&v).coeff(1), qq(8));
        assert_eq!(u.sub(&v).coeff(1), QQ::of_i64(-2));
        assert!(u.sub(&u).is_zero());
    }

    #[test]
    fn vector_abelian_group_default_sub() {
        let u: Vector<QQ> = Vector::of_term(qq(2), 0).add_term(qq(3), 1);
        let v: Vector<QQ> = Vector::of_term(qq(5), 1);
        assert_eq!(<Vector<QQ> as AbelianGroup>::sub(&u, &v), u.sub(&v));
    }

    #[test]
    fn vector_zero_suppression() {
        let u: Vector<QQ> = Vector::of_term(qq(2), 0).add_term(QQ::of_i64(-2), 0);
        assert!(u.is_zero());
        assert_eq!(u.len(), 0);

        let v: Vector<QQ> = Vector::of_term(qq(2), 0).set(0, QQ::zero());
        assert!(v.is_zero());
    }

    #[test]
    fn vector_scalar_mul_and_dot() {
        let u: Vector<QQ> = Vector::of_term(qq(2), 0).add_term(qq(3), 1);
        let v: Vector<QQ> = Vector::of_term(qq(4), 0).add_term(qq(5), 1);
        assert_eq!(u.scalar_mul(&qq(2)).coeff(0), qq(4));
        assert!(u.scalar_mul(&QQ::zero()).is_zero());
        assert_eq!(u.dot(&v), qq(2 * 4 + 3 * 5));
    }

    #[test]
    fn vector_pivot_and_pop_min() {
        let u: Vector<QQ> = Vector::of_term(qq(2), 0).add_term(qq(3), 1);
        let (c, rest) = u.clone().pivot(1);
        assert_eq!(c, qq(3));
        assert_eq!(rest.coeff(1), QQ::zero());
        assert_eq!(rest.coeff(0), qq(2));

        let ((d, c), rest) = u.pop_min().expect("non-zero vector should pop");
        assert_eq!(d, 0);
        assert_eq!(c, qq(2));
        assert_eq!(rest.coeff(1), qq(3));
    }

    #[test]
    fn vector_interlace_deinterlace() {
        let u: Vector<QQ> = Vector::of_term(qq(1), 0).add_term(qq(2), 1);
        let v: Vector<QQ> = Vector::of_term(qq(3), 0).add_term(qq(4), 1);
        let w = u.interlace(&v);
        assert_eq!(w.coeff(0), qq(1));
        assert_eq!(w.coeff(1), qq(3));
        assert_eq!(w.coeff(2), qq(2));
        assert_eq!(w.coeff(3), qq(4));
        let (a, b) = w.deinterlace();
        assert_eq!(a, u);
        assert_eq!(b, v);
    }

    #[test]
    fn matrix_identity_and_entry() {
        let m: Matrix<QQ> = Matrix::identity([0, 1, 2]);
        assert_eq!(m.entry(0, 0), qq(1));
        assert_eq!(m.entry(1, 1), qq(1));
        assert_eq!(m.entry(0, 1), QQ::zero());
        assert_eq!(m.nb_rows(), 3);
        assert_eq!(m.nb_columns(), 3);
    }

    #[test]
    fn matrix_add_and_scalar_mul() {
        let a: Matrix<QQ> = Matrix::of_dense(&[vec![qq(1), qq(2)], vec![qq(3), qq(4)]]);
        let b: Matrix<QQ> = Matrix::of_dense(&[vec![qq(5), qq(6)], vec![qq(7), qq(8)]]);
        let sum = a.add(&b);
        assert_eq!(sum.entry(0, 0), qq(6));
        assert_eq!(sum.entry(1, 1), qq(12));
        let scaled = a.scalar_mul(&qq(2));
        assert_eq!(scaled.entry(0, 0), qq(2));
        assert_eq!(scaled.entry(1, 1), qq(8));
    }

    #[test]
    fn matrix_mul_and_transpose() {
        // [[1,2],[3,4]] * [[2,0],[1,2]] = [[4,4],[10,8]]
        let a: Matrix<QQ> = Matrix::of_dense(&[vec![qq(1), qq(2)], vec![qq(3), qq(4)]]);
        let b: Matrix<QQ> = Matrix::of_dense(&[vec![qq(2), qq(0)], vec![qq(1), qq(2)]]);
        let p = a.mul(&b);
        assert_eq!(p.entry(0, 0), qq(4));
        assert_eq!(p.entry(0, 1), qq(4));
        assert_eq!(p.entry(1, 0), qq(10));
        assert_eq!(p.entry(1, 1), qq(8));

        let t = a.transpose();
        assert_eq!(t.entry(0, 1), qq(3));
        assert_eq!(t.entry(1, 0), qq(2));
        assert_eq!(t.transpose(), a);
    }

    #[test]
    fn matrix_vector_mul() {
        let a: Matrix<QQ> = Matrix::of_dense(&[vec![qq(1), qq(2)], vec![qq(3), qq(4)]]);
        let v: Vector<QQ> = Vector::of_term(qq(5), 0).add_term(qq(6), 1);
        // a*v = [1*5+2*6, 3*5+4*6] = [17, 39]
        let av = a.vector_right_mul(&v);
        assert_eq!(av.coeff(0), qq(17));
        assert_eq!(av.coeff(1), qq(39));
        // v^T * a = [5*1+6*3, 5*2+6*4] = [23, 34]
        let va = a.vector_left_mul(&v);
        assert_eq!(va.coeff(0), qq(23));
        assert_eq!(va.coeff(1), qq(34));
    }

    #[test]
    fn matrix_pivot_column() {
        let a: Matrix<QQ> = Matrix::of_dense(&[vec![qq(1), qq(2)], vec![qq(3), qq(4)]]);
        let (col, rest) = a.pivot_column(1);
        assert_eq!(col.coeff(0), qq(2));
        assert_eq!(col.coeff(1), qq(4));
        assert_eq!(rest.entry(0, 1), QQ::zero());
        assert_eq!(rest.entry(0, 0), qq(1));
        assert_eq!(rest.entry(1, 0), qq(3));
    }

    #[test]
    fn matrix_interlace_columns() {
        let a: Matrix<QQ> = Matrix::of_dense(&[vec![qq(1), qq(2)]]);
        let b: Matrix<QQ> = Matrix::of_dense(&[vec![qq(3), qq(4)]]);
        let c = a.interlace_columns(&b);
        // row 0 should be: a0, b0, a1, b1 = 1, 3, 2, 4
        assert_eq!(c.entry(0, 0), qq(1));
        assert_eq!(c.entry(0, 1), qq(3));
        assert_eq!(c.entry(0, 2), qq(2));
        assert_eq!(c.entry(0, 3), qq(4));
    }
}
