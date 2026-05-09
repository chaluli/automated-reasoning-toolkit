use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use std::fmt;

/// Unbounded integers.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ZZ(BigInt);

impl ZZ {
    pub fn zero() -> Self {
        Self(BigInt::zero())
    }

    pub fn one() -> Self {
        Self(BigInt::one())
    }

    pub fn of_i64(n: i64) -> Self {
        Self(BigInt::from(n))
    }

    pub fn of_string(s: &str) -> Option<Self> {
        s.parse::<BigInt>().ok().map(Self)
    }

    pub fn to_i64(&self) -> Option<i64> {
        self.0.to_i64()
    }

    pub fn add(&self, other: &Self) -> Self {
        Self(&self.0 + &other.0)
    }

    pub fn sub(&self, other: &Self) -> Self {
        Self(&self.0 - &other.0)
    }

    pub fn mul(&self, other: &Self) -> Self {
        Self(&self.0 * &other.0)
    }

    pub fn negate(&self) -> Self {
        Self(-&self.0)
    }

    pub fn abs(&self) -> Self {
        Self(self.0.abs())
    }

    /// Integer division conforming to SMTLIB2:
    /// `a == (a/b)*b + a%b` and `0 <= a%b < |b|`
    pub fn div(&self, other: &Self) -> Self {
        Self(self.0.div_floor(&other.0))
    }

    /// Modulo conforming to SMTLIB2:
    /// `a == (a/b)*b + a%b` and `0 <= a%b < |b|`
    pub fn modulo(&self, other: &Self) -> Self {
        Self(self.0.mod_floor(&other.0))
    }

    pub fn gcd(&self, other: &Self) -> Self {
        Self(self.0.gcd(&other.0))
    }

    pub fn lcm(&self, other: &Self) -> Self {
        Self(self.0.lcm(&other.0))
    }

    pub fn min(&self, other: &Self) -> Self {
        Self(std::cmp::min(&self.0, &other.0).clone())
    }

    pub fn max(&self, other: &Self) -> Self {
        Self(std::cmp::max(&self.0, &other.0).clone())
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    pub fn is_positive(&self) -> bool {
        self.0.is_positive()
    }

    pub fn is_negative(&self) -> bool {
        self.0.is_negative()
    }

    /// Access the underlying `BigInt`.
    pub fn as_bigint(&self) -> &BigInt {
        &self.0
    }
}

impl fmt::Display for ZZ {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<i64> for ZZ {
    fn from(n: i64) -> Self {
        Self::of_i64(n)
    }
}

impl From<BigInt> for ZZ {
    fn from(n: BigInt) -> Self {
        Self(n)
    }
}

/// Unbounded rationals.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QQ(BigRational);

impl QQ {
    pub fn zero() -> Self {
        Self(BigRational::zero())
    }

    pub fn one() -> Self {
        Self(BigRational::one())
    }

    pub fn of_i64(n: i64) -> Self {
        Self(BigRational::from(BigInt::from(n)))
    }

    pub fn of_zz(z: &ZZ) -> Self {
        Self(BigRational::from(z.0.clone()))
    }

    pub fn of_frac(num: i64, den: i64) -> Self {
        Self(BigRational::new(BigInt::from(num), BigInt::from(den)))
    }

    pub fn of_zz_frac(num: &ZZ, den: &ZZ) -> Self {
        Self(BigRational::new(num.0.clone(), den.0.clone()))
    }

    pub fn of_string(s: &str) -> Option<Self> {
        // Try parsing as "num/den" first, then as integer
        if let Some((n, d)) = s.split_once('/') {
            let num = n.trim().parse::<BigInt>().ok()?;
            let den = d.trim().parse::<BigInt>().ok()?;
            Some(Self(BigRational::new(num, den)))
        } else {
            s.parse::<BigInt>().ok().map(|n| Self(BigRational::from(n)))
        }
    }

    pub fn numerator(&self) -> ZZ {
        ZZ(self.0.numer().clone())
    }

    pub fn denominator(&self) -> ZZ {
        ZZ(self.0.denom().clone())
    }

    pub fn to_zz(&self) -> Option<ZZ> {
        if self.0.is_integer() {
            Some(ZZ(self.0.numer().clone()))
        } else {
            None
        }
    }

    pub fn to_f64(&self) -> f64 {
        self.0.numer().to_f64().unwrap_or(f64::INFINITY) / self.0.denom().to_f64().unwrap_or(1.0)
    }

    pub fn to_i64(&self) -> Option<i64> {
        self.to_zz().and_then(|z| z.to_i64())
    }

    pub fn add(&self, other: &Self) -> Self {
        Self(&self.0 + &other.0)
    }

    pub fn sub(&self, other: &Self) -> Self {
        Self(&self.0 - &other.0)
    }

    pub fn mul(&self, other: &Self) -> Self {
        Self(&self.0 * &other.0)
    }

    pub fn div(&self, other: &Self) -> Self {
        Self(&self.0 / &other.0)
    }

    /// Integer division: floor(a/b).
    pub fn idiv(&self, other: &Self) -> ZZ {
        let q = &self.0 / &other.0;
        ZZ(q.floor().numer().clone())
    }

    /// Modulo: `a - b * idiv(a, b)`.
    pub fn modulo(&self, other: &Self) -> Self {
        let quotient_floor = (&self.0 / &other.0).floor();
        Self(&self.0 - &other.0 * quotient_floor)
    }

    pub fn negate(&self) -> Self {
        Self(-&self.0)
    }

    pub fn abs(&self) -> Self {
        Self(self.0.abs())
    }

    pub fn inverse(&self) -> Self {
        Self(self.0.recip())
    }

    pub fn floor(&self) -> ZZ {
        ZZ(self.0.floor().numer().clone())
    }

    pub fn ceiling(&self) -> ZZ {
        ZZ(self.0.ceil().numer().clone())
    }

    pub fn exp(&self, n: u32) -> Self {
        Self(num_traits::pow::Pow::pow(&self.0, n as usize))
    }

    pub fn gcd(&self, other: &Self) -> Self {
        if self.is_zero() {
            return other.abs();
        }
        if other.is_zero() {
            return self.abs();
        }
        // gcd(a/b, c/d) = gcd(a*d, c*b) / (b*d)
        let n = ZZ(self.0.numer() * other.0.denom()).gcd(&ZZ(other.0.numer() * self.0.denom()));
        let d = ZZ(self.0.denom() * other.0.denom());
        Self::of_zz_frac(&n, &d)
    }

    pub fn lcm(&self, other: &Self) -> Self {
        if self.is_zero() || other.is_zero() {
            return Self::zero();
        }
        let product = self.mul(other).abs();
        product.div(&self.gcd(other))
    }

    pub fn min(&self, other: &Self) -> Self {
        if self.0 <= other.0 {
            self.clone()
        } else {
            other.clone()
        }
    }

    pub fn max(&self, other: &Self) -> Self {
        if self.0 >= other.0 {
            self.clone()
        } else {
            other.clone()
        }
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    pub fn is_positive(&self) -> bool {
        self.0.is_positive()
    }

    pub fn is_negative(&self) -> bool {
        self.0.is_negative()
    }

    pub fn is_integer(&self) -> bool {
        self.0.is_integer()
    }

    pub fn leq(&self, other: &Self) -> bool {
        self.0 <= other.0
    }

    pub fn lt(&self, other: &Self) -> bool {
        self.0 < other.0
    }

    /// Access the underlying `BigRational`.
    pub fn as_bigrational(&self) -> &BigRational {
        &self.0
    }
}

impl PartialOrd for QQ {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QQ {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl fmt::Display for QQ {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_integer() {
            write!(f, "{}", self.0.numer())
        } else {
            write!(f, "{}/{}", self.0.numer(), self.0.denom())
        }
    }
}

impl From<i64> for QQ {
    fn from(n: i64) -> Self {
        Self::of_i64(n)
    }
}

impl From<ZZ> for QQ {
    fn from(z: ZZ) -> Self {
        Self(BigRational::from(z.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zz_basic() {
        let a = ZZ::of_i64(10);
        let b = ZZ::of_i64(3);
        assert_eq!(a.add(&b), ZZ::of_i64(13));
        assert_eq!(a.mul(&b), ZZ::of_i64(30));
        assert_eq!(a.sub(&b), ZZ::of_i64(7));
        assert_eq!(a.div(&b), ZZ::of_i64(3));
        assert_eq!(a.modulo(&b), ZZ::of_i64(1));
    }

    #[test]
    fn test_zz_smtlib2_division() {
        // SMTLIB2: -7 div 3 = -3, -7 mod 3 = 2 (0 <= r < |b|)
        let a = ZZ::of_i64(-7);
        let b = ZZ::of_i64(3);
        assert_eq!(a.div(&b), ZZ::of_i64(-3));
        assert_eq!(a.modulo(&b), ZZ::of_i64(2));
    }

    #[test]
    fn test_qq_basic() {
        let half = QQ::of_frac(1, 2);
        let third = QQ::of_frac(1, 3);
        assert_eq!(half.add(&third), QQ::of_frac(5, 6));
        assert_eq!(half.mul(&third), QQ::of_frac(1, 6));
        assert_eq!(half.floor(), ZZ::of_i64(0));
        assert_eq!(half.ceiling(), ZZ::of_i64(1));
    }

    #[test]
    fn test_qq_display() {
        assert_eq!(format!("{}", QQ::of_i64(5)), "5");
        assert_eq!(format!("{}", QQ::of_frac(3, 4)), "3/4");
    }

    #[test]
    fn test_qq_negative_floor_ceil() {
        let neg = QQ::of_frac(-7, 2); // -3.5
        assert_eq!(neg.floor(), ZZ::of_i64(-4));
        assert_eq!(neg.ceiling(), ZZ::of_i64(-3));
    }
}
