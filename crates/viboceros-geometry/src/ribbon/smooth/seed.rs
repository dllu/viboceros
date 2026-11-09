//! A basis-preserving control polygon candidate, checked on the true locus.
use super::*;
use crate::{Vector3, WeightedPoint3, curve_offset::accepts_ribbon_seed};

pub(super) fn try_offset(
    source: &NurbsCurve,
    distance: Real,
    normal: UnitVector3,
    tolerance: Tolerance,
) -> Result<Option<NurbsCurve>, GeometryError> {
    let Ok(candidate) = candidate(source, distance, normal) else {
        return Ok(None);
    };
    Ok(accepts_ribbon_seed(source, &candidate, distance, normal, tolerance)?.then_some(candidate))
}

fn candidate(
    source: &NurbsCurve,
    distance: Real,
    normal: UnitVector3,
) -> Result<NurbsCurve, GeometryError> {
    let points = source.control_points();
    let closed = source.is_closed()?;
    if closed && points[0].point() != points.last().unwrap().point() {
        return Err(GeometryError::NurbsOffsetFitLimit);
    }
    let n = points.len() - usize::from(closed);
    let lefts = points
        .windows(2)
        .map(|p| {
            normal
                .as_vector()
                .cross(p[0].point().vector_to(p[1].point())?)?
                .normalized_nonzero()
                .map(|v| v.as_vector())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut controls = Vec::with_capacity(points.len());
    for (i, p) in points.iter().take(n).enumerate() {
        let shift = if !closed && i == 0 {
            lefts[0].scaled(distance)?
        } else if !closed && i + 1 == n {
            lefts[i - 1].scaled(distance)?
        } else {
            let a = lefts[if i == 0 { n - 1 } else { i - 1 }];
            let b = lefts[i];
            let denominator = 1. + a.dot(b)?;
            if denominator <= 0. {
                return Err(GeometryError::NurbsOffsetFitLimit);
            }
            Vector3::try_new(a.x() + b.x(), a.y() + b.y(), a.z() + b.z())?
                .scaled(distance / denominator)?
        };
        controls.push(WeightedPoint3::try_new(
            p.point().translated(shift)?,
            p.weight(),
        )?);
    }
    if closed {
        controls.push(WeightedPoint3::try_new(
            controls[0].point(),
            points.last().unwrap().weight(),
        )?);
    }
    NurbsCurve::try_new_rational(source.degree(), controls, source.knots().to_vec())
}
