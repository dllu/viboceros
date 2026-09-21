//! Whole-curve certificates: exact binary64 predicates and rational span bounds.
use super::*;
use crate::binary_accumulator::{add_product, decompose};
use std::cmp::Ordering;
mod product;
mod refine;
pub(super) use product::restriction::{
    curve_endpoint_bound, restricted_curve_bound, restricted_endpoint_bound,
};
pub(super) use refine::refined_curve_bound;

/// Fast basis/locus certificates first; otherwise align exact knot spans and
/// bound the rational difference without rounded knot insertion or sampling.
pub(super) fn whole_curve_bound(
    a: &NurbsCurve,
    b: &NurbsCurve,
    reversed: bool,
    limit: Real,
    mut charge: impl FnMut(usize) -> Result<(), GeometryError>,
) -> Result<Option<Real>, GeometryError> {
    charge(
        a.control_points()
            .len()
            .saturating_add(b.control_points().len()),
    )?;
    if let Some(bound) = curve_bound(a, b, reversed, limit) {
        return Ok(Some(bound));
    }
    product::bound(a, b, reversed, limit, false, &mut charge)
}

// At most twenty-four products, including repeated products: 66 limbs at 2^-2148
// cover the entire finite binary64 range and all carries.
struct Products {
    positive: [u64; 66],
    negative: [u64; 66],
}
impl Default for Products {
    fn default() -> Self {
        Self {
            positive: [0; 66],
            negative: [0; 66],
        }
    }
}
impl Products {
    fn add(&mut self, a: Real, b: Real, subtract: bool) {
        let (a_bits, a_shift) = decompose(a);
        let (b_bits, b_shift) = decompose(b);
        let target = if a.is_sign_negative() ^ b.is_sign_negative() ^ subtract {
            &mut self.negative
        } else {
            &mut self.positive
        };
        add_product(
            target,
            u128::from(a_bits) * u128::from(b_bits),
            a_shift + b_shift,
        );
    }
    fn order(&self) -> Ordering {
        self.positive.iter().rev().cmp(self.negative.iter().rev())
    }
    fn compare_square(&self, distance: Real) -> Ordering {
        let mut negative = self.negative;
        let (bits, shift) = decompose(distance);
        add_product(
            &mut negative,
            u128::from(bits) * u128::from(bits),
            2 * shift,
        );
        self.positive.iter().rev().cmp(negative.iter().rev())
    }
}

fn proportional(a: Real, b: Real, a0: Real, b0: Real) -> bool {
    if (a == a0 && b == b0) || (a == b && a0 == b0) {
        return true;
    }
    let mut products = Products::default();
    products.add(a, b0, false);
    products.add(b, a0, true);
    products.order() == Ordering::Equal
}

// (a-a0)/(a1-a0) == (b-b0)/(b1-b0), without rounded subtractions,
// divisions, overflowing products, or underflowed weights/knot spacings.
fn same_fraction(a: Real, a0: Real, a1: Real, b: Real, b0: Real, b1: Real) -> bool {
    if a == b && a0 == b0 && a1 == b1 {
        return true;
    }
    let mut products = Products::default();
    for (x, y, subtract) in [
        (a, b1, false),
        (a, b0, true),
        (a0, b1, true),
        (a0, b0, false),
        (b, a1, true),
        (b, a0, false),
        (b0, a1, false),
        (b0, a0, true),
    ] {
        products.add(x, y, subtract);
    }
    products.order() == Ordering::Equal
}

/// An outward distance bound, or None when the exact distance exceeds limit.
/// The acceptance predicate is independent of libm accuracy and coordinate
/// scale. A rounded hypot is only a candidate upper bound, checked exactly.
pub(super) fn point_bound(a: Point3, b: Point3, limit: Real) -> Option<Real> {
    if a == b {
        return Some(0.);
    }
    let mut squared = Products::default();
    for (a, b) in a.to_array().into_iter().zip(b.to_array()) {
        squared.add(a, a, false);
        squared.add(b, b, false);
        squared.add(a, b, true);
        squared.add(a, b, true);
    }
    if squared.compare_square(limit) == Ordering::Greater {
        return None;
    }
    let mut bound = a.distance_to(b).unwrap_or(limit).min(limit);
    for _ in 0..4 {
        if squared.compare_square(bound) != Ordering::Greater {
            return Some(bound);
        }
        bound = bound.next_up().min(limit);
    }
    // Even an inaccurate platform hypot cannot invalidate the certificate.
    Some(limit)
}

/// Compare two independent point-pair distances, not distances from a shared
/// origin. No rounded subtraction, norm, overflow or underflow breaks ties.
pub(super) fn compare_pair_distances(a: [Point3; 2], b: [Point3; 2]) -> Ordering {
    let mut products = Products::default();
    for ([a, b], subtract) in [(a, false), (b, true)] {
        for (a, b) in a.to_array().into_iter().zip(b.to_array()) {
            products.add(a, a, subtract);
            products.add(b, b, subtract);
            products.add(a, b, !subtract);
            products.add(a, b, !subtract);
        }
    }
    products.order()
}

pub(super) fn curve_bound(
    a: &NurbsCurve,
    b: &NurbsCurve,
    reversed: bool,
    limit: Real,
) -> Option<Real> {
    common_basis_bound(a, b, reversed, limit).or_else(|| {
        let a = linear_endpoints(a)?;
        let mut b = linear_endpoints(b)?;
        if reversed {
            b.reverse();
        }
        Some(point_bound(a[0], b[0], limit)?.max(point_bound(a[1], b[1], limit)?))
    })
}

/// Exact straight, continuous, clamped, monotonically ordered positive-basis curves have
/// the same oriented segment locus regardless of degree, knots or weight speed.
pub(super) fn linear_endpoints(curve: &NurbsCurve) -> Option<[Point3; 2]> {
    let controls = curve.control_points();
    let domain = curve.domain();
    let degree = curve.degree();
    if curve.full_order_knots().next().is_some()
        || !curve.knots()[..=degree].iter().all(|k| k == domain.start())
        || !curve.knots()[curve.knots().len() - degree - 1..]
            .iter()
            .all(|k| k == domain.end())
    {
        return None;
    }
    let endpoints = [controls[0].point(), controls[controls.len() - 1].point()];
    let [a, b] = endpoints.map(Point3::to_array);
    let axis = (0..3).max_by(|&i, &j| (a[i] - b[i]).abs().total_cmp(&(a[j] - b[j]).abs()))?;
    if a[axis] == b[axis] {
        return None;
    }
    let increasing = a[axis] < b[axis];
    let mut previous = a[axis];
    let negative = controls[0].weight().is_sign_negative();
    for control in controls {
        let point = control.point().to_array();
        if control.weight() == 0.
            || control.weight().is_sign_negative() != negative
            || (increasing && point[axis] < previous)
            || (!increasing && point[axis] > previous)
            || (0..3).any(|other| {
                !same_fraction(
                    point[axis],
                    a[axis],
                    b[axis],
                    point[other],
                    a[other],
                    b[other],
                )
            })
        {
            return None;
        }
        previous = point[axis];
    }
    Some(endpoints)
}

fn common_basis_bound(a: &NurbsCurve, b: &NurbsCurve, reversed: bool, limit: Real) -> Option<Real> {
    let ac = a.control_points();
    let bc = b.control_points();
    if a.degree() != b.degree() || ac.len() != bc.len() {
        return None;
    }
    let index = |i: usize, len: usize| if reversed { len - 1 - i } else { i };
    let ad = a.domain();
    let bd = b.domain();
    let (b0, b1) = if reversed {
        (*bd.end(), *bd.start())
    } else {
        (*bd.start(), *bd.end())
    };
    for (i, &knot) in a.knots().iter().enumerate() {
        if !same_fraction(
            knot,
            *ad.start(),
            *ad.end(),
            b.knots()[index(i, b.knots().len())],
            b0,
            b1,
        ) {
            return None;
        }
    }
    let (aw, bw) = (ac[0].weight(), bc[index(0, bc.len())].weight());
    let mut bound: Real = 0.;
    for (i, a) in ac.iter().enumerate() {
        let b = bc[index(i, bc.len())];
        // Convex combination requires sign-coherent nonzero weights. Do not
        // assume this remains a global NurbsCurve invariant forever.
        if a.weight() == 0.
            || b.weight() == 0.
            || a.weight().is_sign_negative() != aw.is_sign_negative()
            || b.weight().is_sign_negative() != bw.is_sign_negative()
            || !proportional(a.weight(), b.weight(), aw, bw)
        {
            return None;
        }
        bound = bound.max(point_bound(a.point(), b.point(), limit)?);
    }
    Some(bound)
}

pub(super) fn add_bound(a: Real, b: Real) -> Result<Real, GeometryError> {
    // Zero is common and should not grow exact, previously joined topology.
    let sum = if a == 0. {
        b
    } else if b == 0. {
        a
    } else {
        (a + b).next_up()
    };
    require_nonnegative_finite(sum, "joined B-rep component tolerance")?;
    Ok(sum)
}

#[cfg(test)]
mod tests;
