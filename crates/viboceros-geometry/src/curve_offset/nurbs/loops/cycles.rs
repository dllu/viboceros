//! Signed polynomial areas and cyclic interval containment in O(n log n).
use super::*;

pub(super) fn area(curve: &NurbsCurve, normal: UnitVector3) -> Result<Real, GeometryError> {
    let origin = curve.evaluate(*curve.domain().start())?;
    let mut sum = crate::FiniteSum::default();
    for span in curve.try_bezier_spans()? {
        let c = span.control_points();
        let p = span.degree();
        if c.iter().any(|v| v.weight() != c[0].weight()) {
            return Err(GeometryError::NurbsOffsetFitLimit);
        }
        let a = crate::bezier::polynomial::binomial(p);
        let b = crate::bezier::polynomial::binomial(p - 1);
        let q = crate::bezier::polynomial::binomial(2 * p - 1);
        for (i, point) in c.iter().enumerate() {
            let x = origin.vector_to(point.point())?;
            for j in 0..p {
                let y = c[j].point().vector_to(c[j + 1].point())?;
                let factor = a[i] * b[j] / (4. * q[i + j]);
                sum.add(x.cross(y)?.dot(normal.as_vector())? * factor)?;
            }
        }
    }
    sum.total()
}

pub(super) fn minimal(mut arcs: Vec<[Real; 2]>) -> Result<Vec<[Real; 2]>, GeometryError> {
    arcs.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    arcs.dedup_by(|a, b| a == b);
    if arcs.iter().any(|[a, b]| {
        !a.is_finite() || !b.is_finite() || *a < 0. || *a >= 1. || *b <= *a || *b - *a >= 1.
    }) {
        return Err(GeometryError::DegenerateOffsetRegion);
    }
    // With ends sorted, any earlier interval starting at or after this start
    // is contained in it. Two copies cover intervals crossing the old seam.
    let mut copies = arcs
        .iter()
        .enumerate()
        .flat_map(|(i, &[a, b])| [(a, b, i, false), (a + 1., b + 1., i, true)])
        .collect::<Vec<_>>();
    copies.sort_by(|a, b| a.1.total_cmp(&b.1).then(b.0.total_cmp(&a.0)));
    let mut maximum = Real::NEG_INFINITY;
    let mut keep = vec![false; arcs.len()];
    for (a, _, i, shifted) in copies {
        if !shifted {
            keep[i] = maximum < a;
        }
        maximum = maximum.max(a);
    }
    Ok(arcs
        .into_iter()
        .zip(keep)
        .filter_map(|(a, k)| k.then_some(a))
        .collect())
}
