//! Curvature-aware surface closest-point refinement with a tangent fallback.
use super::*;
use nalgebra::{Matrix2, Vector2};
mod polish;

#[cfg(test)]
mod tests;

impl SurfaceQuery<'_> {
    pub(super) fn refine_closest_parameters(
        &mut self,
        target: Point3,
        mut u: Real,
        mut v: Real,
        u_domain: [Real; 2],
        v_domain: [Real; 2],
        tolerance: Tolerance,
    ) -> Result<(Real, Real), GeometryError> {
        let initial = self.evaluate(u, v)?;
        if initial == target {
            return Ok((u, v));
        }
        let mut distance = initial.distance_to(target)?;
        for _ in 0..64 {
            let (point, derivative_u, derivative_v, second) =
                match self.evaluate_with_second_derivatives(u, v) {
                    Ok(jet) => (jet.point, jet.derivative_u, jet.derivative_v, Some(jet)),
                    Err(_) => {
                        // Second derivatives may overflow on a tiny UV domain
                        // while its point and first derivatives remain usable.
                        let (point, du, dv) = self.evaluate_with_derivatives(u, v)?;
                        (point, du, dv, None)
                    }
                };
            let residual = point.vector_to(target)?;
            // Normalize columns before QR: parameter speeds are not model-space
            // feature sizes, and must not be compared to a modelling tolerance.
            let x_axis = derivative_u.normalized_nonzero()?;
            let v_direction = derivative_v.normalized_nonzero()?.as_vector();
            let v_along_x = v_direction.dot(x_axis.as_vector())?;
            // Compensated cross products avoid a spurious parallel component
            // from subtracting nearly parallel unit directions in Gram-Schmidt.
            let normal = x_axis
                .as_vector()
                .cross(v_direction)?
                .normalized_nonzero()?;
            let y_axis = normal
                .as_vector()
                .cross(x_axis.as_vector())?
                .normalized_nonzero()?;
            let tangent_x = residual.dot(x_axis.as_vector())?;
            let tangent_y = residual.dot(y_axis.as_vector())?;
            if tangent_x.hypot(tangent_y) <= tolerance.absolute() {
                break;
            }
            let v_motion = tangent_y / v_direction.dot(y_axis.as_vector())?;
            let u_motion = (-v_along_x).mul_add(v_motion, tangent_x);
            let delta_v = parameter_step(derivative_v, v_motion);
            let delta_u = parameter_step(derivative_u, u_motion);
            require_finite([delta_u, delta_v], "surface closest-point step")?;
            let mut accepted = None;
            let newton = second.and_then(|jet| {
                let directions = [x_axis.as_vector(), v_direction];
                let gradient = [
                    residual.dot(directions[0]).ok()?,
                    residual.dot(directions[1]).ok()?,
                ];
                let fixed = active_constraints([u, v], [u_domain, v_domain], gradient);
                curvature_step(jet, residual, directions, fixed)
            });
            // A positive-definite curvature correction is tried first. If its
            // clamped/backtracked direction fails, retain the tangent-plane step.
            for [delta_u, delta_v] in newton.into_iter().chain([[delta_u, delta_v]]) {
                let mut step = 1.0;
                for _ in 0..24 {
                    let candidate_u = (u + step * delta_u).clamp(u_domain[0], u_domain[1]);
                    let candidate_v = (v + step * delta_v).clamp(v_domain[0], v_domain[1]);
                    if candidate_u == u && candidate_v == v {
                        break;
                    }
                    if let Ok(candidate) = self.evaluate(candidate_u, candidate_v)
                        && let Ok(candidate_distance) = candidate.distance_to(target)
                        && (candidate_distance < distance
                            || (candidate_distance == distance
                                && !target.compare_distances(candidate, point).is_gt()))
                    {
                        accepted = Some((
                            candidate_u,
                            candidate_v,
                            candidate_distance,
                            candidate == target,
                        ));
                        break;
                    }
                    step *= 0.5;
                }
                if accepted.is_some() {
                    break;
                }
            }
            let Some((next_u, next_v, next_distance, exact_hit)) = accepted else {
                break;
            };
            u = next_u;
            v = next_v;
            distance = next_distance;
            if exact_hit {
                return Ok((u, v));
            }
        }
        Ok((u, v))
    }
}

/// Hessian of |S-target|²/2, expressed in independently speed-scaled U/V
/// coordinates: H_ij = unit_i.unit_j - residual.S_ij/(speed_i*speed_j).
/// Positive definiteness makes this a descent direction before constraints.
/// This is an optional acceleration: inconclusive arithmetic retains QR above.
fn curvature_step(
    jet: SurfaceJet2,
    residual: Vector3,
    directions: [Vector3; 2],
    fixed: [bool; 2],
) -> Option<[Real; 2]> {
    let speeds = [
        jet.derivative_u.length().ok()?,
        jet.derivative_v.length().ok()?,
    ];
    let coefficient = |partial: Vector3, a: Real, b: Real| -> Option<Real> {
        let product = a * b;
        // Do not amplify rounding of a subnormal metric or divide by an
        // overflowing one. The existing tangent solve handles those scales.
        if !product.is_normal() {
            return None;
        }
        let partial = Vector3::try_from(partial.to_array().map(|x| x / product)).ok()?;
        residual.dot(partial).ok()
    };
    let a = 1. - coefficient(jet.derivative_uu, speeds[0], speeds[0])?;
    let b = directions[0].dot(directions[1]).ok()?
        - coefficient(jet.derivative_uv, speeds[0], speeds[1])?;
    let c = 1. - coefficient(jet.derivative_vv, speeds[1], speeds[1])?;
    require_finite([a, b, c], "surface closest-point Hessian").ok()?;
    let scale = a.abs().max(b.abs()).max(c.abs());
    if scale == 0. {
        return None;
    }
    let mut hessian = Matrix2::new(a / scale, b / scale, b / scale, c / scale);
    let mut gradient = Vector2::new(
        residual.dot(directions[0]).ok()? / scale,
        residual.dot(directions[1]).ok()? / scale,
    );
    for i in 0..2 {
        if fixed[i] {
            hessian[(i, i)] = 1.;
            hessian[(0, 1)] = 0.;
            hessian[(1, 0)] = 0.;
            gradient[i] = 0.;
        }
    }
    let motion = hessian.cholesky()?.solve(&gradient);
    let step = [
        parameter_step(jet.derivative_u, motion[0]),
        parameter_step(jet.derivative_v, motion[1]),
    ];
    require_finite(step, "surface closest-point Newton step").ok()?;
    Some(step)
}

fn active_constraints(
    parameters: [Real; 2],
    domains: [[Real; 2]; 2],
    gradient: [Real; 2],
) -> [bool; 2] {
    std::array::from_fn(|i| {
        (parameters[i] <= domains[i][0] && gradient[i] <= 0.)
            || (parameters[i] >= domains[i][1] && gradient[i] >= 0.)
    })
}

fn projected_gradient_norm(gradient: [Real; 2], fixed: [bool; 2]) -> Real {
    let free: [Real; 2] = std::array::from_fn(|i| if fixed[i] { 0. } else { gradient[i] });
    free[0].hypot(free[1])
}

/// Divide by a nonzero derivative's norm without forming an overflowing norm.
fn parameter_step(derivative: Vector3, motion: Real) -> Real {
    derivative.parameter_step(motion)
}
