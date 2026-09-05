//! Parameterized model-space image of an exact UV trim.

use super::*;
use crate::ParameterSide;

pub(super) struct LiftedTrim<'a> {
    pub(super) curve: NurbsCurve,
    surface: &'a NurbsSurface,
}

impl<'a> LiftedTrim<'a> {
    pub(super) fn new(trim: &BrepTrim, surface: &'a NurbsSurface) -> Result<Self, GeometryError> {
        let curve = NurbsCurve::try_new_rational(
            trim.curve.degree(),
            trim.curve
                .control_points()
                .iter()
                .map(|cp| {
                    WeightedPoint3::try_new(
                        Point3::try_new(cp.point().x(), cp.point().y(), 0.0)?,
                        cp.weight(),
                    )
                })
                .collect::<Result<Vec<_>, GeometryError>>()?,
            trim.curve.knots().to_vec(),
        )?;
        Ok(Self { curve, surface })
    }

    pub(super) fn point(
        &self,
        parameter: Real,
        side: ParameterSide,
    ) -> Result<Point3, GeometryError> {
        let uv = self.curve.evaluate_on_side(parameter, side)?;
        self.surface.evaluate(uv.x(), uv.y())
    }

    fn jet(&self, parameter: Real) -> Result<(Point3, Vector3), GeometryError> {
        let (uv, derivative) = self.curve.evaluate_with_derivative(parameter)?;
        let (point, du, dv) = self.surface.evaluate_with_derivatives(uv.x(), uv.y())?;
        let tangent = Vector3::try_from(std::array::from_fn(|axis| {
            du.to_array()[axis].mul_add(derivative.x(), dv.to_array()[axis] * derivative.y())
        }))?;
        Ok((point, tangent))
    }

    pub(super) fn closest_point(
        &self,
        target: Point3,
        samples: &[(Real, Point3)],
        epsilon: Real,
    ) -> Result<(Real, Real), GeometryError> {
        let mut candidates = samples
            .iter()
            .map(|&(t, point)| Ok((point.distance_to(target)?, t)))
            .collect::<Result<Vec<_>, GeometryError>>()?;
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
        candidates.truncate(16);
        let mut best = *candidates
            .first()
            .ok_or(GeometryError::InvalidBrepTopology {
                context: "trim closest point needs search samples",
            })?;
        let domain = self.curve.domain();
        for (mut distance, mut parameter) in candidates {
            for _ in 0..64 {
                if distance <= epsilon {
                    return Ok((distance, parameter));
                }
                let (point, tangent) = self.jet(parameter)?;
                let speed = tangent.length()?;
                if speed == 0.0 {
                    break;
                }
                let projection = point.vector_to(target)?.dot(tangent)? / speed;
                if projection.abs() <= epsilon {
                    break;
                }
                let delta = projection / speed;
                if !delta.is_finite() {
                    break;
                }
                let mut accepted = None;
                let mut step: Real = 1.0;
                for _ in 0..24 {
                    let next = step
                        .mul_add(delta, parameter)
                        .clamp(*domain.start(), *domain.end());
                    if next == parameter {
                        break;
                    }
                    let next_distance = self
                        .point(next, ParameterSide::Right)?
                        .distance_to(target)?;
                    if next_distance < distance {
                        accepted = Some((next, next_distance));
                        break;
                    }
                    step *= 0.5;
                }
                let Some((next, next_distance)) = accepted else {
                    break;
                };
                parameter = next;
                distance = next_distance;
            }
            if distance < best.0 {
                best = (distance, parameter);
            }
        }
        Ok(best)
    }
}
