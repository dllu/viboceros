//! Local stationarity recovery when positional roundoff obscures distance descent.
use super::*;

impl NurbsSurface {
    pub(in crate::nurbs_surface::closest_point) fn polish_closest_parameters(
        &self,
        target: Point3,
        parameters: (Real, Real),
        tolerance: Tolerance,
    ) -> (Real, Real) {
        if let Some(result) = self.polish_in_domain(target, parameters, tolerance) {
            return result;
        }
        // A tiny domain may have usable first but overflowing second partials.
        // One normalized copy recovers local curvature without restarting the
        // search or repeatedly cloning the net for every refinement seed.
        if (self.domain_u() != (0.0..=1.0) || self.domain_v() != (0.0..=1.0))
            && let Ok(surface) = self.try_reparameterized(0.0..=1.0, 0.0..=1.0)
            && let Ok([u, v]) = self.normalized_parameters(parameters.0, parameters.1)
            && let Some((u, v)) = surface.polish_in_domain(target, (u, v), tolerance)
            && let (Ok(u), Ok(v)) = (self.parameter_at_u(u), self.parameter_at_v(v))
            && self.evaluate(u, v).is_ok()
        {
            return (u, v);
        }
        parameters
    }

    fn polish_in_domain(
        &self,
        target: Point3,
        mut parameters: (Real, Real),
        tolerance: Tolerance,
    ) -> Option<(Real, Real)> {
        let domains = [self.domain_u(), self.domain_v()].map(|d| [*d.start(), *d.end()]);
        let scale = self
            .control_points
            .iter()
            .flat_map(|c| c.point().to_array())
            .map(Real::abs)
            .fold(0.0, Real::max);
        let mut jet = self
            .evaluate_with_second_derivatives(parameters.0, parameters.1)
            .ok()?;
        let initial = jet.point;
        let initial_distance = initial.distance_to(target).ok()?;
        let scale = scale.max(initial_distance).max(
            initial
                .to_array()
                .into_iter()
                .map(Real::abs)
                .fold(0., Real::max),
        );
        // The squared objective has quadratic sensitivity near a minimum, while
        // evaluated positions have first-order rounding error. These are local
        // safeguards, not a certified error bound for arbitrary rational nets.
        let radius = (64. * Real::EPSILON).sqrt() * scale;
        let allowance = 64. * Real::EPSILON * scale;
        for _ in 0..8 {
            let (residual, directions, gradient) = gradient_at(jet, target)?;
            let fixed = active_constraints([parameters.0, parameters.1], domains, gradient);
            let norm = projected_gradient_norm(gradient, fixed);
            if norm <= tolerance.absolute() {
                break;
            }
            let Some(step) = curvature_step(jet, residual, directions, fixed) else {
                break;
            };
            let candidate = (
                (parameters.0 + step[0]).clamp(domains[0][0], domains[0][1]),
                (parameters.1 + step[1]).clamp(domains[1][0], domains[1][1]),
            );
            if candidate == parameters {
                break;
            }
            let next = self
                .evaluate_with_second_derivatives(candidate.0, candidate.1)
                .ok()?;
            if initial.distance_to(next.point).ok()? > radius
                || next.point.distance_to(target).ok()? - initial_distance > allowance
            {
                break;
            }
            let (_, _, gradient) = gradient_at(next, target)?;
            let fixed = active_constraints([candidate.0, candidate.1], domains, gradient);
            if projected_gradient_norm(gradient, fixed) >= norm {
                break;
            }
            parameters = candidate;
            jet = next;
        }
        Some(parameters)
    }
}

fn gradient_at(jet: SurfaceJet2, target: Point3) -> Option<(Vector3, [Vector3; 2], [Real; 2])> {
    let residual = jet.point.vector_to(target).ok()?;
    let directions = [
        jet.derivative_u.normalized_nonzero().ok()?.as_vector(),
        jet.derivative_v.normalized_nonzero().ok()?.as_vector(),
    ];
    let gradient = [
        residual.dot(directions[0]).ok()?,
        residual.dot(directions[1]).ok()?,
    ];
    Some((residual, directions, gradient))
}
