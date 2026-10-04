//! Exact fallback for collapsed, extremely thin or overflowing triangles.

use crate::exact_scalar::{Rational, rational, scalar};
use crate::{GeometryError, Point3};
use num_traits::{One, Zero};

type ExactPoint = [Rational; 3];

fn difference(a: &ExactPoint, b: &ExactPoint) -> ExactPoint {
    std::array::from_fn(|i| &a[i] - &b[i])
}

fn dot(a: &ExactPoint, b: &ExactPoint) -> Rational {
    &a[0] * &b[0] + &a[1] * &b[1] + &a[2] * &b[2]
}

fn barycentric(
    a: &ExactPoint,
    ab: &ExactPoint,
    ac: &ExactPoint,
    u: Rational,
    v: Rational,
) -> ExactPoint {
    std::array::from_fn(|i| &a[i] + &ab[i] * &u + &ac[i] * &v)
}

fn segment(target: &ExactPoint, a: &ExactPoint, b: &ExactPoint) -> ExactPoint {
    let delta = difference(b, a);
    let squared = dot(&delta, &delta);
    if squared.is_zero() {
        return a.clone();
    }
    let along = dot(&difference(target, a), &delta);
    if along <= Rational::zero() {
        a.clone()
    } else if along >= squared {
        b.clone()
    } else {
        let t = along / squared;
        std::array::from_fn(|i| &a[i] + &delta[i] * &t)
    }
}

pub(super) fn exact_triangle(
    target: Point3,
    a: Point3,
    b: Point3,
    c: Point3,
) -> Result<Point3, GeometryError> {
    let [target, a, b, c] = [target, a, b, c].map(|p| p.to_array().map(rational));
    let ab = difference(&b, &a);
    let ac = difference(&c, &a);
    let cross: ExactPoint = std::array::from_fn(|i| {
        let (j, k) = ((i + 1) % 3, (i + 2) % 3);
        &ab[j] * &ac[k] - &ab[k] * &ac[j]
    });
    let point = if cross.iter().all(Zero::is_zero) {
        let candidates = [
            segment(&target, &a, &b),
            segment(&target, &b, &c),
            segment(&target, &c, &a),
        ];
        candidates
            .into_iter()
            .min_by_key(|p| {
                let delta = difference(&target, p);
                dot(&delta, &delta)
            })
            .expect("three segment candidates")
    } else {
        regular(&target, &a, &b, &c, &ab, &ac)
    };
    Point3::try_new(scalar(&point[0])?, scalar(&point[1])?, scalar(&point[2])?)
}

fn regular(
    target: &ExactPoint,
    a: &ExactPoint,
    b: &ExactPoint,
    c: &ExactPoint,
    ab: &ExactPoint,
    ac: &ExactPoint,
) -> ExactPoint {
    let zero = Rational::zero();
    let ap = difference(target, a);
    let (d1, d2) = (dot(ab, &ap), dot(ac, &ap));
    if d1 <= zero && d2 <= zero {
        return a.clone();
    }
    let bp = difference(target, b);
    let (d3, d4) = (dot(ab, &bp), dot(ac, &bp));
    if d3 >= zero && d4 <= d3 {
        return b.clone();
    }
    let vc = &d1 * &d4 - &d3 * &d2;
    if vc <= zero && d1 >= zero && d3 <= zero {
        return barycentric(a, ab, ac, &d1 / (&d1 - &d3), zero);
    }
    let cp = difference(target, c);
    let (d5, d6) = (dot(ab, &cp), dot(ac, &cp));
    if d6 >= zero && d5 <= d6 {
        return c.clone();
    }
    let vb = &d5 * &d2 - &d1 * &d6;
    if vb <= zero && d2 >= zero && d6 <= zero {
        return barycentric(a, ab, ac, zero, &d2 / (&d2 - &d6));
    }
    let va = &d3 * &d6 - &d5 * &d4;
    let (d43, d56) = (&d4 - &d3, &d5 - &d6);
    if va <= zero && d43 >= zero && d56 >= zero {
        let v = &d43 / (&d43 + &d56);
        return barycentric(a, ab, ac, Rational::one() - &v, v);
    }
    let sum = va + &vb + &vc;
    barycentric(a, ab, ac, vb / &sum, vc / sum)
}
