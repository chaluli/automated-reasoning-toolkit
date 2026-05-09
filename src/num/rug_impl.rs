use rug::ops::Pow;
use rug::{Integer, Rational};
use std::cmp::Ordering;
use std::fmt;

/// Unbounded integers.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ZZ(Integer);

impl ZZ {
    pub fn zero() -> Self {
        Self(Integer::new())
    }

    pub fn one() -> Self {
        Self(Integer::from(1))
    }

    pub fn of_i64(n: i64) -> Self {
        Self(Integer::from(n))
    }

    pub fn of_string(s: &str) -> Option<Self> {
        Integer::from_str_radix(s, 10).ok().map(Self)
    }

    pub fn to_i64(&self) -> Option<i64> {
        self.0.to_i64()
    }

    pub fn add(&self, other: &Self) -> Self {
        Self(Integer::from(&self.0 + &other.0))
    }

    pub fn sub(&self, other: &Self) -> Self {
        Self(Integer::from(&self.0 - &other.0))
    }

    pub fn mul(&self, other: &Self) -> Self {
        Self(Integer::from(&self.0 * &other.0))
    }

    pub fn negate(&self) -> Self {
        Self(Integer::from(-&self.0))
    }

    pub fn abs(&self) -> Self {
        Self(self.0.clone().abs())
    }

    /// Integer division conforming to SMTLIB2:
    /// `a == (a/b)*b + a%b` and `0 <= a%b < |b|`
    pub fn div(&self, other: &Self) -> Self {
        let (q, _) = self.0.clone().div_rem_floor(other.0.clone());
        Self(q)
    }

    /// Modulo conforming to SMTLIB2:
    /// `a == (a/b)*b + a%b` and `0 <= a%b < |b|`
    pub fn modulo(&self, other: &Self) -> Self {
        let (_, r) = self.0.clone().div_rem_floor(other.0.clone());
        Self(r)
    }

    pub fn gcd(&self, other: &Self) -> Self {
        Self(self.0.clone().gcd(&other.0))
    }

    pub fn lcm(&self, other: &Self) -> Self {
        Self(self.0.clone().lcm(&other.0))
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
        self.0.cmp0() == Ordering::Equal
    }

    pub fn is_positive(&self) -> bool {
        self.0.cmp0() == Ordering::Greater
    }

    pub fn is_negative(&self) -> bool {
        self.0.cmp0() == Ordering::Less
    }

    /// Access the underlying `Integer`.
    pub fn as_integer(&self) -> &Integer {
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

impl From<Integer> for ZZ {
    fn from(n: Integer) -> Self {
        Self(n)
    }
}

/// Unbounded rationals.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QQ(Rational);

impl QQ {
    pub fn zero() -> Self {
        Self(Rational::new())
    }

    pub fn one() -> Self {
        Self(Rational::from(1))
    }

    pub fn of_i64(n: i64) -> Self {
        Self(Rational::from(n))
    }

    pub fn of_zz(z: &ZZ) -> Self {
        Self(Rational::from(z.0.clone()))
    }

    pub fn of_frac(num: i64, den: i64) -> Self {
        Self(Rational::from((num, den)))
    }

    pub fn of_zz_frac(num: &ZZ, den: &ZZ) -> Self {
        Self(Rational::from((num.0.clone(), den.0.clone())))
    }

    pub fn of_string(s: &str) -> Option<Self> {
        if let Some((n, d)) = s.split_once('/') {
            let num = Integer::from_str_radix(n.trim(), 10).ok()?;
            let den = Integer::from_str_radix(d.trim(), 10).ok()?;
            if den.cmp0() == Ordering::Equal {
                return None;
            }
            Some(Self(Rational::from((num, den))))
        } else {
            Integer::from_str_radix(s, 10)
                .ok()
                .map(|n| Self(Rational::from(n)))
        }
    }

    pub fn numerator(&self) -> ZZ {
        ZZ(self.0.numer().clone())
    }

    pub fn denominator(&self) -> ZZ {
        ZZ(self.0.denom().clone())
    }

    pub fn to_zz(&self) -> Option<ZZ> {
        if self.is_integer() {
            Some(ZZ(self.0.numer().clone()))
        } else {
            None
        }
    }

    pub fn to_f64(&self) -> f64 {
        self.0.to_f64()
    }

    pub fn to_i64(&self) -> Option<i64> {
        self.to_zz().and_then(|z| z.to_i64())
    }

    pub fn add(&self, other: &Self) -> Self {
        Self(Rational::from(&self.0 + &other.0))
    }

    pub fn sub(&self, other: &Self) -> Self {
        Self(Rational::from(&self.0 - &other.0))
    }

    pub fn mul(&self, other: &Self) -> Self {
        Self(Rational::from(&self.0 * &other.0))
    }

    pub fn div(&self, other: &Self) -> Self {
        Self(Rational::from(&self.0 / &other.0))
    }

    /// Integer division: floor(a/b).
    pub fn idiv(&self, other: &Self) -> ZZ {
        let q = Rational::from(&self.0 / &other.0);
        let (num, den) = q.into_numer_denom();
        let (floor, _) = num.div_rem_floor(den);
        ZZ(floor)
    }

    /// Modulo: `a - b * idiv(a, b)`.
    pub fn modulo(&self, other: &Self) -> Self {
        let quotient_floor = self.idiv(other);
        let prod = Rational::from(&other.0 * &quotient_floor.0);
        Self(&self.0 - prod)
    }

    pub fn negate(&self) -> Self {
        Self(Rational::from(-&self.0))
    }

    pub fn abs(&self) -> Self {
        Self(self.0.clone().abs())
    }

    pub fn inverse(&self) -> Self {
        Self(self.0.clone().recip())
    }

    pub fn floor(&self) -> ZZ {
        let num = self.0.numer().clone();
        let den = self.0.denom().clone();
        let (q, _) = num.div_rem_floor(den);
        ZZ(q)
    }

    pub fn ceiling(&self) -> ZZ {
        // ceil(a/b) = -floor(-a/b)
        let num = Integer::from(-self.0.numer());
        let den = self.0.denom().clone();
        let (q, _) = num.div_rem_floor(den);
        ZZ(-q)
    }

    pub fn exp(&self, n: u32) -> Self {
        Self(self.0.clone().pow(n))
    }

    pub fn gcd(&self, other: &Self) -> Self {
        if self.is_zero() {
            return other.abs();
        }
        if other.is_zero() {
            return self.abs();
        }
        // gcd(a/b, c/d) = gcd(a*d, c*b) / (b*d)
        let ad = ZZ(Integer::from(self.0.numer() * other.0.denom()));
        let cb = ZZ(Integer::from(other.0.numer() * self.0.denom()));
        let n = ad.gcd(&cb);
        let d = ZZ(Integer::from(self.0.denom() * other.0.denom()));
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
        self.0.cmp0() == Ordering::Equal
    }

    pub fn is_positive(&self) -> bool {
        self.0.cmp0() == Ordering::Greater
    }

    pub fn is_negative(&self) -> bool {
        self.0.cmp0() == Ordering::Less
    }

    pub fn is_integer(&self) -> bool {
        self.0.denom().cmp0() == Ordering::Greater && self.0.denom().to_i32() == Some(1)
    }

    pub fn leq(&self, other: &Self) -> bool {
        self.0 <= other.0
    }

    pub fn lt(&self, other: &Self) -> bool {
        self.0 < other.0
    }

    /// Access the underlying `Rational`.
    pub fn as_rational(&self) -> &Rational {
        &self.0
    }
}

impl PartialOrd for QQ {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QQ {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl fmt::Display for QQ {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_integer() {
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
        Self(Rational::from(z.0))
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
