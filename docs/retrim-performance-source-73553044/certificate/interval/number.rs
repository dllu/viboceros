//! Finite outward-rounded arithmetic. Subnormal operations fall back to rationals.
use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct I {
    pub lo: Real,
    pub hi: Real,
}
impl I {
    fn new(lo: Real, hi: Real) -> Option<Self> {
        let normal = |x: Real| x.is_finite() && (x == 0. || x.abs() >= Real::MIN_POSITIVE);
        (normal(lo) && normal(hi) && lo <= hi).then_some(Self { lo, hi })
    }
    pub fn point(x: Real) -> Option<Self> {
        Self::new(x, x)
    }
    pub fn exact(r: &Rational) -> Option<Self> {
        let value = scalar(r).ok()?;
        let stored = rational(value);
        let result = match stored.cmp(r) {
            std::cmp::Ordering::Equal => Self::point(value),
            std::cmp::Ordering::Less => Self::new(value, value.next_up()),
            std::cmp::Ordering::Greater => Self::new(value.next_down(), value),
        }?;
        // Conversion is itself checked exactly; one adjacent float must enclose r.
        (rational(result.lo) <= *r && rational(result.hi) >= *r).then_some(result)
    }
    fn rounded(x: Real) -> Option<Self> {
        if x == 0. {
            return None;
        }
        Self::new(x.next_down(), x.next_up())
    }
    fn sum(a: Real, b: Real) -> Option<Self> {
        if a == 0. {
            return Self::point(b);
        }
        if b == 0. {
            return Self::point(a);
        }
        if a == -b {
            return Self::point(0.);
        }
        Self::rounded(a + b)
    }
    fn product(a: Real, b: Real) -> Option<Self> {
        if a == 0. || b == 0. {
            return Self::point(0.);
        }
        if a == 1. {
            return Self::point(b);
        }
        if b == 1. {
            return Self::point(a);
        }
        if a == -1. {
            return Self::point(-b);
        }
        if b == -1. {
            return Self::point(-a);
        }
        if b.abs() < 1. && a.abs() < Real::MIN_POSITIVE / b.abs() {
            return None;
        }
        Self::rounded(a * b)
    }
    fn quotient(a: Real, b: Real) -> Option<Self> {
        if b == 0. {
            return None;
        }
        if a == 0. {
            return Self::point(0.);
        }
        if b == 1. {
            return Self::point(a);
        }
        if b == -1. {
            return Self::point(-a);
        }
        if b.abs() > 1. && a.abs() < Real::MIN_POSITIVE * b.abs() {
            return None;
        }
        Self::rounded(a / b)
    }
    pub fn add(self, b: Self) -> Option<Self> {
        Self::new(Self::sum(self.lo, b.lo)?.lo, Self::sum(self.hi, b.hi)?.hi)
    }
    pub fn sub(self, b: Self) -> Option<Self> {
        self.add(Self {
            lo: -b.hi,
            hi: -b.lo,
        })
    }
    pub fn mul(self, b: Self) -> Option<Self> {
        let values = [
            Self::product(self.lo, b.lo)?,
            Self::product(self.lo, b.hi)?,
            Self::product(self.hi, b.lo)?,
            Self::product(self.hi, b.hi)?,
        ];
        Self::new(
            values.iter().map(|v| v.lo).fold(Real::INFINITY, Real::min),
            values
                .iter()
                .map(|v| v.hi)
                .fold(Real::NEG_INFINITY, Real::max),
        )
    }
    pub fn div(self, b: Self) -> Option<Self> {
        if b.lo <= 0. && b.hi >= 0. {
            return None;
        }
        let values = [
            Self::quotient(self.lo, b.lo)?,
            Self::quotient(self.lo, b.hi)?,
            Self::quotient(self.hi, b.lo)?,
            Self::quotient(self.hi, b.hi)?,
        ];
        Self::new(
            values.iter().map(|v| v.lo).fold(Real::INFINITY, Real::min),
            values
                .iter()
                .map(|v| v.hi)
                .fold(Real::NEG_INFINITY, Real::max),
        )
    }
    pub fn abs(self) -> Option<Self> {
        let lo = if self.lo <= 0. && self.hi >= 0. {
            0.
        } else {
            self.lo.abs().min(self.hi.abs())
        };
        Self::new(lo, self.lo.abs().max(self.hi.abs()))
    }
}
