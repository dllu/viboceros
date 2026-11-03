//! Separating-plane certificate for neighboring rational Bezier spans.
use super::*;
use crate::exact_scalar::{Rational, rational};

pub(super) fn separated(a: &NurbsCurve, b: &NurbsCurve) -> Result<bool, GeometryError> {
    for curve in [a, b] {
        let p = curve.degree();
        let knots = curve.knots();
        let domain = curve.domain();
        if curve.control_points().len() != p + 1
            || knots[..=p].iter().any(|k| k != domain.start())
            || knots[p + 1..].iter().any(|k| k != domain.end())
        {
            return Ok(false);
        }
    }
    let ac = a.control_points();
    let bc = b.control_points();
    let point = ac.last().unwrap().point();
    if point != bc[0].point() {
        return Ok(false);
    }
    for c in [ac, bc] {
        if c.iter().any(|p| {
            p.weight() == 0. || p.weight().is_sign_positive() != c[0].weight().is_sign_positive()
        }) {
            return Ok(false);
        }
    }
    let incoming = ac[0].point().vector_to(point)?;
    let outgoing = point.vector_to(bc.last().unwrap().point())?;
    if incoming.length()? == 0. || outgoing.length()? == 0. {
        return Ok(false);
    }
    let x = incoming.normalized_nonzero()?.as_vector().to_array();
    let y = outgoing.normalized_nonzero()?.as_vector().to_array();
    let axis = Vector3::try_from(std::array::from_fn(|i| x[i] + y[i]))?;
    let axis = axis.to_array().map(rational);
    let origin = point.to_array().map(rational);
    let projection = |p: Point3| -> Rational {
        p.to_array()
            .into_iter()
            .enumerate()
            .map(|(i, x)| &axis[i] * (rational(x) - &origin[i]))
            .sum()
    };
    let zero = rational(0.);
    if projection(ac[0].point()) >= zero || projection(bc.last().unwrap().point()) <= zero {
        return Ok(false);
    }
    // Every interior rational Bezier basis value has positive weight. With
    // one strict coefficient, the two interiors stay in opposite open half
    // spaces. This certifies their sole shared endpoint even at a sharp join
    // or with very different span lengths; no metric tangent search is needed.
    if ac.iter().any(|p| projection(p.point()) > zero)
        || bc.iter().any(|p| projection(p.point()) < zero)
    {
        return Ok(false);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn curve(points: &[[Real; 2]]) -> NurbsCurve {
        let p = points.len() - 1;
        NurbsCurve::try_new(
            p,
            points
                .iter()
                .map(|p| Point3::try_new(p[0], p[1], 0.).unwrap())
                .collect(),
            [vec![0.; p + 1], vec![1.; p + 1]].concat(),
        )
        .unwrap()
    }
    #[test]
    fn separated_junction_handles_unequal_spans_and_rejects_retracing_or_crossing_hulls() {
        let a = curve(&[[-0.01, 0.], [-0.005, 0.], [0., 0.]]);
        let b = curve(&[[0., 0.], [-50., 50.], [-100., 100.]]);
        assert!(separated(&a, &b).unwrap());
        assert!(!separated(&a, &curve(&[[0., 0.], [-1., 0.]])).unwrap());
        assert!(!separated(&curve(&[[-1., 0.], [20., 20.], [0., 0.]]), &b).unwrap());
        assert!(!separated(&a, &curve(&[[0., 1e-12], [1., 1.]])).unwrap());
    }
}
