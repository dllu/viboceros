//! Exclude reversed normal-locus visits using derivative control hulls.
use super::*;

pub(super) fn backward(piece: &NurbsCurve, source: &NurbsCurve) -> Result<bool, GeometryError> {
    let spans = source.try_trimmed(piece.domain())?.try_bezier_spans()?;
    if spans.len() != 1 {
        return Ok(false);
    }
    let c = spans[0].control_points();
    let p = spans[0].degree();
    // This certificate is optional. Bound its quadratic coefficient work.
    if p > 64 {
        return Ok(false);
    }
    let origin = c[0].point();
    let scale = c.iter().map(|v| v.weight().abs()).fold(0., Real::max);
    let w = c.iter().map(|v| v.weight() / scale).collect::<Vec<_>>();
    let x = c
        .iter()
        .zip(&w)
        .map(|(v, w)| origin.vector_to(v.point())?.scaled(*w))
        .collect::<Result<Vec<_>, _>>()?;
    let a = crate::bezier::polynomial::binomial(p - 1);
    let b = crate::bezier::polynomial::binomial(p);
    let denominator = crate::bezier::polynomial::binomial(2 * p - 1);
    let mut tangent = vec![Vector3::try_new(0., 0., 0.)?; 2 * p];
    // The rational source derivative is (X' W - X W') / W².
    // Its numerator has these Bernstein coefficients, up to positive p.
    for i in 0..p {
        let dx = combine(x[i + 1], x[i], -1.)?;
        let dw = w[i + 1] - w[i];
        for j in 0..=p {
            let term = combine(dx.scaled(w[j])?, x[j], -dw)?;
            tangent[i + j] = combine(tangent[i + j], term, a[i] * b[j] / denominator[i + j])?;
        }
    }
    // Every derivative is a convex combination of its coefficients. If all
    // pairwise dots are negative, both curves travel in opposite directions
    // throughout the interval. Those visits cannot survive a region inset.
    for pair in piece.control_points().windows(2) {
        let derivative = pair[0].point().vector_to(pair[1].point())?;
        for t in &tangent {
            if derivative.dot(*t)? >= 0. {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
fn combine(a: Vector3, b: Vector3, scale: Real) -> Result<Vector3, GeometryError> {
    let a = a.to_array();
    let b = b.to_array();
    Vector3::try_from(std::array::from_fn(|i| scale.mul_add(b[i], a[i])))
}
