//! Preserve a representable source weight function during rational fitting.

use super::interpolation::DEGREE;
use super::*;

pub(super) fn surface_weights(source: &NurbsSurface) -> Option<NurbsSurface> {
    if source.degree_u() > DEGREE || source.degree_v() > DEGREE {
        return None;
    }
    let controls = normalized_controls(source.control_points())?;
    // W is a scalar polynomial spline, represented in X with unit weights.
    NurbsSurface::try_new_rational(
        source.degree_u(),
        source.degree_v(),
        source.control_point_count_u(),
        source.control_point_count_v(),
        controls,
        source.knots_u().to_vec(),
        source.knots_v().to_vec(),
    )
    .ok()
}

pub(super) fn curve_weights(source: &NurbsCurve) -> Option<NurbsCurve> {
    if source.degree() > DEGREE {
        return None;
    }
    NurbsCurve::try_new_rational(
        source.degree(),
        normalized_controls(source.control_points())?,
        source.knots().to_vec(),
    )
    .ok()
}

fn normalized_controls(controls: &[WeightedPoint3]) -> Option<Vec<WeightedPoint3>> {
    let first = controls[0].weight();
    if controls.iter().all(|c| c.weight() == first)
        || controls
            .iter()
            .any(|c| c.weight().is_sign_positive() != first.is_sign_positive())
    {
        return None;
    }
    let scale = controls
        .iter()
        .map(|c| c.weight().abs())
        .fold(0.0, Real::max);
    controls
        .iter()
        .map(|c| {
            let weight = c.weight().abs() / scale;
            if weight == 0.0 {
                return None;
            }
            WeightedPoint3::try_new(Point3::try_new(weight, 0.0, 0.0).ok()?, 1.0).ok()
        })
        .collect()
}
