//! A common-sign rational degree-one span traverses its chord monotonically.
use super::*;

pub(super) fn source(curve: &NurbsCurve) -> Result<Option<Polyline3>, GeometryError> {
    let controls = curve.control_points();
    let sign = controls[0].weight().is_sign_positive();
    if controls
        .iter()
        .any(|c| c.weight().is_sign_positive() != sign)
    {
        return Err(GeometryError::UnsupportedCurveApproximation {
            context: "weights must have a common sign",
        });
    }
    let mut points = Vec::new();
    for span in 1..controls.len() {
        if curve.knots()[span] == curve.knots()[span + 1] {
            continue;
        }
        let a = controls[span - 1].point();
        let b = controls[span].point();
        if points.is_empty() {
            points.push(a);
        } else if points.last() != Some(&a) {
            return Err(GeometryError::UnsupportedCurveApproximation {
                context: "curve is discontinuous at a knot",
            });
        }
        if a != b {
            points.push(b);
        }
        if points.len() > crate::MAX_CURVE_DIVISION_POINTS {
            return Err(GeometryError::MeshPullLimit);
        }
    }
    if points.len() < 2 {
        return Ok(None);
    }
    Ok(Some(Polyline3::try_new(
        points,
        Tolerance::NUMERICAL_VALIDATION,
    )?))
}
