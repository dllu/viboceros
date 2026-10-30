//! Exact signs of chart orientation and world-distance circumcircle tests.
use crate::{
    Point3,
    exact_scalar::{Rational, rational},
    interval::I,
};
use num_traits::Zero;
use std::cmp::Ordering;

fn sign(value: I) -> Option<Ordering> {
    if value.lo > 0. {
        Some(Ordering::Greater)
    } else if value.hi < 0. {
        Some(Ordering::Less)
    } else if value.lo == 0. && value.hi == 0. {
        Some(Ordering::Equal)
    } else {
        None
    }
}
fn difference(a: Point3, b: Point3) -> Option<[I; 3]> {
    let (a, b) = (a.to_array(), b.to_array());
    let mut result = [I::point(0.)?; 3];
    for (k, x) in result.iter_mut().enumerate() {
        *x = I::point(a[k])?.sub(I::point(b[k])?)?;
    }
    Some(result)
}
fn exact_difference(a: Point3, b: Point3) -> [Rational; 3] {
    let (a, b) = (a.to_array(), b.to_array());
    std::array::from_fn(|k| rational(a[k]) - rational(b[k]))
}
fn cross(a: [I; 3], b: [I; 3], [x, y]: [usize; 2]) -> Option<I> {
    a[x].mul(b[y])?.sub(a[y].mul(b[x])?)
}
fn exact_cross(a: &[Rational; 3], b: &[Rational; 3], [x, y]: [usize; 2]) -> Rational {
    &a[x] * &b[y] - &a[y] * &b[x]
}
fn norm(a: [I; 3]) -> Option<I> {
    a[0].mul(a[0])?.add(a[1].mul(a[1])?)?.add(a[2].mul(a[2])?)
}
fn exact_norm(a: &[Rational; 3]) -> Rational {
    a.iter().map(|x| x * x).sum()
}

pub(super) fn orient([a, b, c]: [Point3; 3], axes: [usize; 2]) -> Ordering {
    let filtered = (|| sign(cross(difference(a, c)?, difference(b, c)?, axes)?))();
    filtered.unwrap_or_else(|| {
        exact_cross(&exact_difference(a, c), &exact_difference(b, c), axes).cmp(&Rational::zero())
    })
}

/// Positive for an inside point when the first three points are chart-CCW.
/// The chart contributes only oriented area. Squared distances retain all
/// three world coordinates, so an oblique chart cannot distort this metric.
pub(super) fn circle([a, b, c, d]: [Point3; 4], axes: [usize; 2]) -> Ordering {
    let filtered = (|| {
        let (u, v, w) = (difference(a, d)?, difference(b, d)?, difference(c, d)?);
        let det = norm(u)?
            .mul(cross(v, w, axes)?)?
            .sub(norm(v)?.mul(cross(u, w, axes)?)?)?
            .add(norm(w)?.mul(cross(u, v, axes)?)?)?;
        sign(det)
    })();
    filtered.unwrap_or_else(|| {
        let (u, v, w) = (
            exact_difference(a, d),
            exact_difference(b, d),
            exact_difference(c, d),
        );
        (exact_norm(&u) * exact_cross(&v, &w, axes) - exact_norm(&v) * exact_cross(&u, &w, axes)
            + exact_norm(&w) * exact_cross(&u, &v, axes))
        .cmp(&Rational::zero())
    })
}
