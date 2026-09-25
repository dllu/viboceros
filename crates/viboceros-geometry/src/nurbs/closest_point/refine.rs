//! Monotone evaluated-point descent with range-safe tangent projections.
use super::*;

impl CurveQuery<'_> {
    pub(super) fn refine_closest_parameter_only(
        &mut self,
        target: Point3,
        mut parameter: Real,
        domain: [Real; 2],
        tolerance: Tolerance,
    ) -> Result<Real, GeometryError> {
        let mut current = PointDistance::new(target, self.evaluate(parameter)?);
        for _ in 0..64 {
            if current.point() == target {
                break;
            }
            let (derivative, second) = match self.evaluate_with_second_derivative(parameter) {
                Ok((_, first, second)) => (first, Some(second)),
                Err(_) => (self.evaluate_with_derivative(parameter)?.1, None),
            };
            let (derivative, second) = if derivative.normalized_nonzero().is_ok() {
                (derivative, second)
            } else {
                // At a multiple knot the derivative belongs to the span on
                // its right. If that span is stationary, inspect the adjacent
                // one-sided tangents before declaring this a local minimum.
                let adjacent = [parameter.next_down(), parameter.next_up()]
                    .into_iter()
                    .filter(|candidate| domain[0] <= *candidate && *candidate <= domain[1])
                    .find_map(|candidate| {
                        let tangent = self.evaluate_with_derivative(candidate).ok()?.1;
                        tangent.normalized_nonzero().ok().map(|_| tangent)
                    });
                let Some(tangent) = adjacent else {
                    break;
                };
                (tangent, None)
            };
            let Ok(unit) = derivative.normalized_nonzero() else {
                break;
            };
            // Project the exact point difference along a unit tangent. Neither
            // the displacement nor a product with the unnormalized speed must
            // fit in binary64 for this model-space projection to be usable.
            let projection = unit
                .as_vector()
                .dot_point_difference(target, current.point());
            // Model tolerance describes acceptable distances, not the
            // precision of the nearest parameter. A coarser stop here can
            // leave a visible control-point error after trimming to the hit.
            let numerical_projection_tolerance = derivative
                .length()
                .ok()
                .map(|speed| (8.0 * Real::EPSILON * speed).min(tolerance.absolute()))
                .unwrap_or(tolerance.absolute());
            if projection.abs() <= numerical_projection_tolerance {
                break;
            }
            let tangent = derivative.parameter_step(projection);
            // Hessian / speed = speed - residual.(C''/speed). Inconclusive
            // curvature or an unrepresentable speed retains the tangent step.
            let newton = derivative
                .length()
                .ok()
                .and_then(|speed| {
                    let scaled = Vector3::try_from(second?.to_array().map(|x| x / speed)).ok()?;
                    let denominator = speed - scaled.dot_point_difference(target, current.point());
                    (denominator.is_finite() && denominator > 0.)
                        .then_some(projection / denominator)
                })
                .filter(|step| *step != tangent);
            let mut accepted = None;
            for direction in newton.into_iter().chain([tangent]) {
                if !direction.is_finite() {
                    continue;
                }
                let mut step: Real = 1.;
                for _ in 0..24 {
                    let candidate = step
                        .mul_add(direction, parameter)
                        .clamp(domain[0], domain[1]);
                    if candidate == parameter {
                        break;
                    }
                    if let Ok(point) = self.evaluate(candidate) {
                        let distance = PointDistance::new(target, point);
                        // A rounded distance tie is not evidence of descent.
                        if !distance.compare(&current, target).is_gt() {
                            accepted = Some((candidate, distance));
                            break;
                        }
                    }
                    step *= 0.5;
                }
                if accepted.is_some() {
                    break;
                }
            }
            let Some((next_parameter, next_distance)) = accepted else {
                break;
            };
            parameter = next_parameter;
            current = next_distance;
        }
        Ok(parameter)
    }
}
