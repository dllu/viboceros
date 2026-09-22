//! Small outward-rounded arithmetic filter; any unbounded result requests exact recovery.
use crate::Real;
use std::ops::{Add, Div, Mul, Sub};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug)]
pub(super) struct Bound {
    lo: Real,
    hi: Real,
}

impl Bound {
    const WHOLE: Self = Self {
        lo: Real::NEG_INFINITY,
        hi: Real::INFINITY,
    };
    pub(super) fn point(value: Real) -> Self {
        Self {
            lo: value,
            hi: value,
        }
    }
    fn outward(lo: Real, hi: Real) -> Self {
        if !lo.is_finite() || !hi.is_finite() {
            return Self::WHOLE;
        }
        Self {
            lo: lo.next_down(),
            hi: hi.next_up(),
        }
    }
    fn is_zero(self) -> bool {
        self.lo == 0. && self.hi == 0.
    }
    pub(super) fn excludes_zero(self) -> bool {
        self.lo > 0. || self.hi < 0.
    }
    pub(super) fn magnitude(self) -> Real {
        self.lo.abs().max(self.hi.abs())
    }
    pub(super) fn midpoint(self) -> Real {
        self.lo * 0.5 + self.hi * 0.5
    }
    pub(super) fn error_bound(self, value: Real) -> Real {
        (value - self.lo)
            .abs()
            .max((value - self.hi).abs())
            .next_up()
    }
    pub(super) fn positive_sqrt(self) -> Option<Self> {
        (self.lo > 0. && self.hi.is_finite()).then(|| Self::outward(self.lo.sqrt(), self.hi.sqrt()))
    }
}

impl Add for Bound {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        if self.is_zero() {
            return rhs;
        }
        if rhs.is_zero() {
            return self;
        }
        Self::outward(self.lo + rhs.lo, self.hi + rhs.hi)
    }
}
impl Sub for Bound {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        if rhs.is_zero() {
            return self;
        }
        if self.lo == self.hi && self.lo == rhs.lo && rhs.lo == rhs.hi {
            return Self::point(0.);
        }
        Self::outward(self.lo - rhs.hi, self.hi - rhs.lo)
    }
}
impl Mul for Bound {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        if self.is_zero() || rhs.is_zero() {
            return Self::point(0.);
        }
        let products = [
            self.lo * rhs.lo,
            self.lo * rhs.hi,
            self.hi * rhs.lo,
            self.hi * rhs.hi,
        ];
        if products.iter().any(|x| !x.is_finite()) {
            return Self::WHOLE;
        }
        Self::outward(
            products.into_iter().fold(Real::INFINITY, Real::min),
            products.into_iter().fold(Real::NEG_INFINITY, Real::max),
        )
    }
}
impl Div for Bound {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        if !rhs.excludes_zero() {
            return Self::WHOLE;
        }
        if self.is_zero() {
            return self;
        }
        let quotients = [
            self.lo / rhs.lo,
            self.lo / rhs.hi,
            self.hi / rhs.lo,
            self.hi / rhs.hi,
        ];
        if quotients.iter().any(|x| !x.is_finite()) {
            return Self::WHOLE;
        }
        Self::outward(
            quotients.into_iter().fold(Real::INFINITY, Real::min),
            quotients.into_iter().fold(Real::NEG_INFINITY, Real::max),
        )
    }
}
