//! Smooth native pieces retain their chart in the original parent.
use super::*;
use viboceros_geometry::CurveContinuity;

pub(super) fn all(source: &Curve3, tolerance: Tolerance) -> Result<Vec<Curve3>, CommandError> {
    match source {
        Curve3::Polyline(c) => Ok(c.segments().map(Curve3::Line).collect()),
        Curve3::PolyCurve(c) => {
            let mut result = Vec::new();
            for (index, segment) in c.segments().iter().enumerate() {
                for local in all(&segment.clone().into_curve(), tolerance)? {
                    let domain = local.as_ref().domain();
                    result.push(local.try_reparameterized(
                        c.polycurve_parameter(index, *domain.start())?
                            ..=c.polycurve_parameter(index, *domain.end())?,
                    )?);
                }
            }
            Ok(result)
        }
        Curve3::NurbsCurve(c) => {
            let mut bounds =
                c.discontinuity_parameters(CurveContinuity::G1, tolerance.angular())?;
            let domain = c.domain();
            if !bounds.is_empty()
                && let Some(pieces) = c.try_split_at_c0_parameters(&bounds)?
            {
                return Ok(pieces.into_iter().map(Curve3::NurbsCurve).collect());
            }
            if bounds.is_empty() && c.is_closed()? {
                let a = source
                    .as_ref()
                    .evaluate_with_tangent(*domain.start())?
                    .tangent()
                    .as_vector();
                let b = source
                    .as_ref()
                    .evaluate_with_tangent(*domain.end())?
                    .tangent()
                    .as_vector();
                if a.dot(b)? < tolerance.angular().cos() {
                    bounds.push((*domain.start() + *domain.end()) * 0.5);
                }
            }
            bounds.insert(0, *domain.start());
            bounds.push(*domain.end());
            bounds
                .windows(2)
                .map(|p| super::super::super::curve_segments::trimmed(source, p[0]..=p[1]))
                .collect()
        }
        _ => Ok(vec![source.clone()]),
    }
}
