//! Closed-triangle Voronoi regions with outward-rounded error certificates.
//! Cross products avoid the squared conditioning of Gram determinants. Any
//! uncertain region, intermediate range loss or loose coordinate uses the
//! exact rational query instead. The bound is independent of model tolerance.
use crate::{Point3, interval::I};

#[cfg(test)]
mod tests;

// Below this floor, an FMA residual could itself underflow to zero. A product
// of two binary64 significands has at most 106 bits; this leaves its least
// significant bit within the subnormal range. Smaller products use I/fallback.
const RESIDUAL_FLOOR: f64 = f64::MIN_POSITIVE * (1u64 << 54) as f64;

#[derive(Clone, Copy)]
struct F {
    value: f64,
    interval: I,
}
impl F {
    fn point(value: f64) -> Option<Self> {
        Some(Self {
            value,
            interval: I::point(value)?,
        })
    }
    fn add(self, b: Self) -> Option<Self> {
        if self.exact_pair(b) && exact_sum(self.value, b.value) {
            return Self::point(self.value + b.value);
        }
        Some(Self {
            value: self.value + b.value,
            interval: self.interval.add(b.interval)?,
        })
    }
    fn sub(self, b: Self) -> Option<Self> {
        if self.exact_pair(b) && exact_sum(self.value, -b.value) {
            return Self::point(self.value - b.value);
        }
        Some(Self {
            value: self.value - b.value,
            interval: self.interval.sub(b.interval)?,
        })
    }
    fn mul(self, b: Self) -> Option<Self> {
        let value = self.value * b.value;
        if self.exact_pair(b)
            && (self.value == 0.
                || b.value == 0.
                || value.is_finite()
                    && value.abs() >= RESIDUAL_FLOOR
                    && self.value.mul_add(b.value, -value) == 0.)
        {
            return Self::point(value);
        }
        Some(Self {
            value,
            interval: self.interval.mul(b.interval)?,
        })
    }
    fn div(self, b: Self) -> Option<Self> {
        let value = self.value / b.value;
        if self.exact_pair(b)
            && value.is_finite()
            && self.value.abs() >= RESIDUAL_FLOOR
            && value.mul_add(b.value, -self.value) == 0.
        {
            return Self::point(value);
        }
        Some(Self {
            value,
            interval: self.interval.div(b.interval)?,
        })
    }
    fn exact_pair(self, b: Self) -> bool {
        self.interval.lo == self.interval.hi && b.interval.lo == b.interval.hi
    }
    fn neg(self) -> Self {
        Self {
            value: -self.value,
            interval: I {
                lo: -self.interval.hi,
                hi: -self.interval.lo,
            },
        }
    }
    fn product_sum(a: Self, b: Self, c: Self, d: Self) -> Option<Self> {
        // Keep the two product residuals until after their large leading
        // terms cancel. Every step still carries an interval certificate.
        if a.exact_pair(b) && c.exact_pair(d) {
            let tighter = || {
                let (p, ep) = product_parts(a.value, b.value)?;
                let (q, eq) = product_parts(c.value, d.value)?;
                let (sum, error) = sum_parts(p, q)?;
                let tail = Self::point(ep)?
                    .add(Self::point(eq)?)?
                    .add(Self::point(error)?)?;
                Self::point(sum)?.add(tail)
            };
            if let Some(result) = tighter() {
                return Some(result);
            }
        }
        a.mul(b)?.add(c.mul(d)?)
    }
    fn nonnegative(self) -> Option<bool> {
        if self.interval.lo >= 0. {
            Some(true)
        } else if self.interval.hi < 0. {
            Some(false)
        } else {
            None
        }
    }
    fn nonpositive(self) -> Option<bool> {
        if self.interval.hi <= 0. {
            Some(true)
        } else if self.interval.lo > 0. {
            Some(false)
        } else {
            None
        }
    }
    fn coordinate(self) -> Option<f64> {
        if !self.value.is_finite() {
            return None;
        }
        let ulp = [self.value.next_up(), self.value.next_down()]
            .into_iter()
            .filter(|x| x.is_finite())
            .map(|x| (x - self.value).abs())
            .fold(0., f64::max);
        let error = (self.value - self.interval.lo)
            .abs()
            .max((self.interval.hi - self.value).abs());
        let error = if error == 0. { 0. } else { error.next_up() };
        (error <= 32. * ulp).then_some(self.value)
    }
}

fn exact_sum(a: f64, b: f64) -> bool {
    sum_parts(a, b).is_some_and(|(_, error)| error == 0.)
}
fn sum_parts(a: f64, b: f64) -> Option<(f64, f64)> {
    let sum = a + b;
    if !sum.is_finite() {
        return None;
    }
    let virtual_b = sum - a;
    // TwoSum's residual is exact with gradual underflow.
    let error = (a - (sum - virtual_b)) + (b - virtual_b);
    error.is_finite().then_some((sum, error))
}
fn product_parts(a: f64, b: f64) -> Option<(f64, f64)> {
    let product = a * b;
    if !product.is_finite() || a != 0. && b != 0. && product.abs() < RESIDUAL_FLOOR {
        return None;
    }
    Some((product, a.mul_add(b, -product)))
}

type V = [F; 3];
fn difference(a: Point3, b: Point3) -> Option<V> {
    let a = a.to_array();
    let b = b.to_array();
    Some([
        F::point(a[0])?.sub(F::point(b[0])?)?,
        F::point(a[1])?.sub(F::point(b[1])?)?,
        F::point(a[2])?.sub(F::point(b[2])?)?,
    ])
}
fn cross(a: V, b: V) -> Option<V> {
    Some([
        F::product_sum(a[1], b[2], a[2].neg(), b[1])?,
        F::product_sum(a[2], b[0], a[0].neg(), b[2])?,
        F::product_sum(a[0], b[1], a[1].neg(), b[0])?,
    ])
}
fn dot(a: V, b: V) -> Option<F> {
    F::product_sum(a[0], b[0], a[1], b[1])?.add(a[2].mul(b[2])?)
}
fn all<const N: usize>(predicates: [Option<bool>; N]) -> Option<bool> {
    if predicates.contains(&Some(false)) {
        Some(false)
    } else if predicates.iter().all(|p| *p == Some(true)) {
        Some(true)
    } else {
        None
    }
}
fn interpolate(origin: Point3, ab: V, ac: V, u: F, v: F) -> Option<Point3> {
    let a = origin.to_array();
    let mut coordinates = [0.; 3];
    for i in 0..3 {
        coordinates[i] = F::point(a[i])?
            .add(ab[i].mul(u)?)?
            .add(ac[i].mul(v)?)?
            .coordinate()?;
    }
    Point3::try_from(coordinates).ok()
}

pub(super) fn triangle(target: Point3, a: Point3, b: Point3, c: Point3) -> Option<Point3> {
    let (ab, ac, ap) = (difference(b, a)?, difference(c, a)?, difference(target, a)?);
    let (d1, d2) = (dot(ab, ap)?, dot(ac, ap)?);
    if all([d1.nonpositive(), d2.nonpositive()])? {
        return Some(a);
    }
    let (bp, bc) = (difference(target, b)?, difference(c, b)?);
    let (d3, bc_bp) = (dot(ab, bp)?, dot(bc, bp)?);
    // Computing (c-b).(p-b) directly preserves the short edge at B;
    // subtracting the nearly equal AC/AP and AB/AP products does not.
    if all([d3.nonnegative(), bc_bp.nonpositive()])? {
        return Some(b);
    }
    let normal = cross(ab, ac)?;
    let vc = dot(normal, cross(ab, ap)?)?;
    let zero = F::point(0.)?;
    if all([vc.nonpositive(), d1.nonnegative(), d3.nonpositive()])? {
        return interpolate(a, ab, ac, d1.div(d1.sub(d3)?)?, zero);
    }
    let cp = difference(target, c)?;
    let (d6, bc_cp) = (dot(ac, cp)?, dot(bc, cp)?);
    if all([d6.nonnegative(), bc_cp.nonnegative()])? {
        return Some(c);
    }
    let vb = dot(normal, cross(ap, ac)?)?;
    if all([vb.nonpositive(), d2.nonnegative(), d6.nonpositive()])? {
        return interpolate(a, ab, ac, zero, d2.div(d2.sub(d6)?)?);
    }
    let va = dot(normal, cross(bp, cp)?)?;
    if all([va.nonpositive(), bc_bp.nonnegative(), bc_cp.nonpositive()])? {
        let v = bc_bp.div(bc_bp.sub(bc_cp)?)?;
        return interpolate(b, bc, bc, v, zero);
    }
    // The interval predicates have excluded every exterior Voronoi region.
    // An axis-aligned plane provides an exact foot without interpolation.
    let [aa, bb, cc, pp] = [a, b, c, target].map(Point3::to_array);
    if let Some(axis) = (0..3).find(|&i| aa[i] == bb[i] && aa[i] == cc[i]) {
        let mut foot = pp;
        foot[axis] = aa[axis];
        return Point3::try_from(foot).ok();
    }
    let denominator = dot(normal, normal)?;
    interpolate(a, ab, ac, vb.div(denominator)?, vc.div(denominator)?)
}
