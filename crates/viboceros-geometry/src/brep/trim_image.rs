//! Parameterized model-space image of an exact UV trim.

use super::*;
use crate::ParameterSide;

#[cfg(test)]
mod tests;

pub(super) const MAX_REFINEMENT_STEPS: usize = 64;

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

    fn jet(
        &self,
        parameter: Real,
        side: ParameterSide,
    ) -> Result<(Point3, Vector3), GeometryError> {
        let (uv, derivative) = self
            .curve
            .evaluate_with_derivative_on_side(parameter, side)?;
        let (point, du, dv) = self.surface.evaluate_with_derivatives(uv.x(), uv.y())?;
        let tangent = Vector3::try_from(std::array::from_fn(|axis| {
            du.to_array()[axis].mul_add(derivative.x(), dv.to_array()[axis] * derivative.y())
        }))?;
        Ok((point, tangent))
    }

    /// Finds an evaluated-point distance witness, returning early within epsilon.
    /// If no start reaches epsilon, returns the best point encountered, not a
    /// certified global minimum. Work scales with the supplied finite seed set;
    /// each start has bounded refinement and backtracking iterations.
    pub(super) fn distance_witness(
        &self,
        target: Point3,
        samples: &[(Real, ParameterSide, Point3)],
        epsilon: Real,
    ) -> Result<(Real, Real), GeometryError> {
        let mut candidates = samples
            .iter()
            .map(|&(t, side, point)| Ok((point.distance_to(target)?, t, side)))
            .collect::<Result<Vec<_>, GeometryError>>()?;
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
        // A fixed nearest-N prefix can consist entirely of stationary samples,
        // excluding every start on an active span whose image contains target.
        // Distance orders work; it must not discard any supplied start. Stop
        // only after finding an actual evaluated point within epsilon.
        let first = *candidates
            .first()
            .ok_or(GeometryError::InvalidBrepTopology {
                context: "trim distance witness needs search samples",
            })?;
        let mut best = (first.0, first.1);
        let domain = self.curve.domain();
        let knots = self.curve.knots();
        for (mut distance, mut parameter, side) in candidates {
            let span = if side == ParameterSide::Left && parameter > *domain.start() {
                knots.partition_point(|k| *k < parameter) - 1
            } else {
                find_span_in_knots(
                    knots,
                    self.curve.degree(),
                    self.curve.control_points().len(),
                    parameter,
                )
            };
            let bounds = [
                knots[span].max(*domain.start()),
                knots[span + 1].min(*domain.end()),
            ];
            // Keep the start on its own polynomial piece. An improving step
            // onto a neighboring stationary span can have zero tangent even
            // though the original span contains an exact solution. At its upper
            // endpoint use the left jet, not the next piece's derivative.
            let side_at = |t| {
                if t == bounds[1] {
                    ParameterSide::Left
                } else {
                    ParameterSide::Right
                }
            };
            for _ in 0..MAX_REFINEMENT_STEPS {
                if distance <= epsilon {
                    return Ok((distance, parameter));
                }
                let (point, tangent) = self.jet(parameter, side_at(parameter))?;
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
                    let next = step.mul_add(delta, parameter).clamp(bounds[0], bounds[1]);
                    if next == parameter {
                        break;
                    }
                    let next_distance = self.point(next, side_at(next))?.distance_to(target)?;
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
