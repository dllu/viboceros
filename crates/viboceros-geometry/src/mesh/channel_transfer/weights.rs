//! Projected barycentrics with exact fallback for poorly conditioned triangles.
use crate::{
    GeometryError, Point3, Real,
    exact_scalar::{Rational, rational, scalar},
};
use num_traits::{Signed, Zero};

pub(super) fn triangle(p: Point3, corners: [Point3; 3]) -> Result<[Real; 3], GeometryError> {
    let [a, b, c] = corners.map(Point3::to_array);
    let p = p.to_array();
    let ab: [Real; 3] = std::array::from_fn(|i| b[i] - a[i]);
    let ac: [Real; 3] = std::array::from_fn(|i| c[i] - a[i]);
    let ap: [Real; 3] = std::array::from_fn(|i| p[i] - a[i]);
    let scale = ab.into_iter().chain(ac).map(Real::abs).fold(0., Real::max);
    if scale.is_normal() && scale.is_finite() && ap.iter().all(|x| x.is_finite()) {
        let [ab, ac, ap] = [ab, ac, ap].map(|v| v.map(|x| x / scale));
        let det: [Real; 3] = std::array::from_fn(|i| {
            let (j, k) = ((i + 1) % 3, (i + 2) % 3);
            ab[j] * ac[k] - ab[k] * ac[j]
        });
        let axis = (0..3)
            .max_by(|&i, &j| det[i].abs().total_cmp(&det[j].abs()))
            .unwrap();
        let (j, k) = ((axis + 1) % 3, (axis + 2) % 3);
        let bound = (ab[j] * ac[k]).abs() + (ab[k] * ac[j]).abs();
        if det[axis].is_normal() && det[axis].abs() > 128. * Real::EPSILON * bound {
            let u = (ap[j] * ac[k] - ap[k] * ac[j]) / det[axis];
            let v = (ab[j] * ap[k] - ab[k] * ap[j]) / det[axis];
            let weights = [1. - u - v, u, v];
            if weights.iter().all(|w| w.is_finite()) {
                return normalized(weights);
            }
        }
    }
    let [a, b, c, p] = [a, b, c, p].map(|v| v.map(rational));
    let ab: [Rational; 3] = std::array::from_fn(|i| &b[i] - &a[i]);
    let ac: [Rational; 3] = std::array::from_fn(|i| &c[i] - &a[i]);
    let ap: [Rational; 3] = std::array::from_fn(|i| &p[i] - &a[i]);
    let det: [Rational; 3] = std::array::from_fn(|i| {
        let (j, k) = ((i + 1) % 3, (i + 2) % 3);
        &ab[j] * &ac[k] - &ab[k] * &ac[j]
    });
    let axis = (0..3).max_by_key(|&i| det[i].abs()).unwrap();
    if det[axis].is_zero() {
        return Err(GeometryError::Degenerate {
            context: "mesh channel source triangle",
        });
    }
    let (j, k) = ((axis + 1) % 3, (axis + 2) % 3);
    let u = (&ap[j] * &ac[k] - &ap[k] * &ac[j]) / &det[axis];
    let v = (&ab[j] * &ap[k] - &ab[k] * &ap[j]) / &det[axis];
    normalized([scalar(&(rational(1.) - &u - &v))?, scalar(&u)?, scalar(&v)?])
}
fn normalized(weights: [Real; 3]) -> Result<[Real; 3], GeometryError> {
    let weights = weights.map(|w| w.clamp(0., 1.));
    let total = weights.iter().sum::<Real>();
    if !total.is_finite() || total <= 0. {
        return Err(GeometryError::Degenerate {
            context: "mesh channel weights",
        });
    }
    Ok(weights.map(|w| w / total))
}
