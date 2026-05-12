//! Signatures for algebraic structures.

use crate::num::{QQ, ZZ};

/// A set with an associative binary operation `mul`.
pub trait Semigroup: Clone {
    fn mul(a: &Self, b: &Self) -> Self;
}

/// A commutative group: associative, commutative `add` with identity `zero`
/// and inverse `negate`.
pub(crate) trait AbelianGroup: Clone + PartialEq {
    fn zero() -> Self;
    fn add(a: &Self, b: &Self) -> Self;
    fn negate(a: &Self) -> Self;

    fn sub(a: &Self, b: &Self) -> Self {
        Self::add(a, &Self::negate(b))
    }
    fn equal(a: &Self, b: &Self) -> bool {
        a == b
    }
    fn is_zero(a: &Self) -> bool {
        Self::equal(a, &Self::zero())
    }
}

/// A (unital) ring. Self-contained: every Ring is automatically a
/// [`Semigroup`] (under `mul`) via the blanket impl below.
pub trait Ring: Clone + PartialEq {
    fn zero() -> Self;
    fn one() -> Self;
    fn add(a: &Self, b: &Self) -> Self;
    fn mul(a: &Self, b: &Self) -> Self;
    fn negate(a: &Self) -> Self;

    fn sub(a: &Self, b: &Self) -> Self {
        Self::add(a, &Self::negate(b))
    }
    fn equal(a: &Self, b: &Self) -> bool {
        a == b
    }
    fn is_zero(a: &Self) -> bool {
        Self::equal(a, &Self::zero())
    }
}

impl<R: Ring> Semigroup for R {
    fn mul(a: &Self, b: &Self) -> Self {
        <R as Ring>::mul(a, b)
    }
}

impl<R: Ring> AbelianGroup for R {
    fn zero() -> Self {
        <R as Ring>::zero()
    }
    fn add(a: &Self, b: &Self) -> Self {
        <R as Ring>::add(a, b)
    }
    fn negate(a: &Self) -> Self {
        <R as Ring>::negate(a)
    }
    fn sub(a: &Self, b: &Self) -> Self {
        <R as Ring>::sub(a, b)
    }
    fn is_zero(a: &Self) -> bool {
        <R as Ring>::is_zero(a)
    }
}

/// A semilattice: an associative, commutative, idempotent `join`.
pub trait Semilattice: Clone + PartialEq {
    fn join(a: &Self, b: &Self) -> Self;
    fn equal(a: &Self, b: &Self) -> bool {
        a == b
    }
}

/// A lattice: a semilattice with a dual `meet` operation.
pub trait Lattice: Semilattice {
    fn meet(a: &Self, b: &Self) -> Self;
}

// ---------------------------------------------------------------------------
// Ring impls for the project's bignum types
// ---------------------------------------------------------------------------

impl Ring for ZZ {
    fn zero() -> Self {
        Self::zero()
    }
    fn one() -> Self {
        Self::one()
    }
    fn add(a: &Self, b: &Self) -> Self {
        a.add(b)
    }
    fn mul(a: &Self, b: &Self) -> Self {
        a.mul(b)
    }
    fn negate(a: &Self) -> Self {
        a.negate()
    }
    fn sub(a: &Self, b: &Self) -> Self {
        a.sub(b)
    }
}

impl Ring for QQ {
    fn zero() -> Self {
        Self::zero()
    }
    fn one() -> Self {
        Self::one()
    }
    fn add(a: &Self, b: &Self) -> Self {
        a.add(b)
    }
    fn mul(a: &Self, b: &Self) -> Self {
        a.mul(b)
    }
    fn negate(a: &Self) -> Self {
        a.negate()
    }
    fn sub(a: &Self, b: &Self) -> Self {
        a.sub(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring_axioms<R: Ring + std::fmt::Debug>(a: &R, b: &R, c: &R) {
        let zero = R::zero();
        let one = R::one();
        assert!(R::equal(&R::add(a, &zero), a));
        assert!(R::equal(&R::mul(a, &one), a));
        assert!(R::equal(
            &R::add(&R::add(a, b), c),
            &R::add(a, &R::add(b, c))
        ));
        assert!(R::is_zero(&R::add(a, &R::negate(a))));
    }

    #[test]
    fn zz_ring_axioms() {
        ring_axioms(&ZZ::of_i64(2), &ZZ::of_i64(3), &ZZ::of_i64(5));
    }

    #[test]
    fn qq_ring_axioms() {
        ring_axioms(&QQ::of_frac(1, 2), &QQ::of_frac(3, 4), &QQ::of_frac(-5, 6));
    }
}
