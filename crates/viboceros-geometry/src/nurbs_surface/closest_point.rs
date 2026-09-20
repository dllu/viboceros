//! Bounded multi-start surface closest-point search and scaled tangent refinement.
use super::*;
mod affine;

impl NurbsSurface {
    /// Finds natural surface parameters nearest to a finite model-space
    /// point. Exactly affine bilinear patches use direct constrained projection;
    /// other surfaces use bounded multi-start tangent-plane Newton refinement.
    /// Neither path assumes normalized parameter domains.
    pub fn closest_parameters(
        &self,
        target: Point3,
        tolerance: Tolerance,
    ) -> Result<(Real, Real), GeometryError> {
        if let Some(parameters) = self.closest_affine_parameters(target) {
            return Ok(parameters);
        }
        self.closest_parameters_general(target, tolerance)
    }

    fn closest_parameters_general(
        &self,
        target: Point3,
        tolerance: Tolerance,
    ) -> Result<(Real, Real), GeometryError> {
        let u_domain = self.domain_u();
        let v_domain = self.domain_v();
        let u_start = *u_domain.start();
        let u_end = *u_domain.end();
        let v_start = *v_domain.start();
        let v_end = *v_domain.end();
        let u_seeds = closest_parameter_seeds(self.spans_u(), u_start, u_end);
        let v_seeds = closest_parameter_seeds(self.spans_v(), v_start, v_end);
        let mut seeds = Vec::with_capacity(u_seeds.len() * v_seeds.len());
        for &v in &v_seeds {
            for &u in &u_seeds {
                if let Ok(point) = self.evaluate(u, v)
                    && let Ok(distance) = point.distance_to(target)
                {
                    seeds.push((distance, u, v));
                }
            }
        }
        seeds.sort_by(|left, right| left.0.total_cmp(&right.0));
        seeds.truncate(16);
        let mut best = seeds.first().copied().ok_or(GeometryError::Degenerate {
            context: "NURBS surface closest-point search",
        })?;
        let mut best_point = self.evaluate(best.1, best.2)?;
        // Clamping a coupled two-parameter Newton step can stall before the
        // minimum along an active boundary. Solve all four natural boundary
        // curves independently, including their endpoints. Singular constant
        // edges remain represented by the boundary seeds above.
        for (curve, fixed_u, fixed) in [
            (self.isocurve_u(v_start), false, v_start),
            (self.isocurve_u(v_end), false, v_end),
            (self.isocurve_v(u_start), true, u_start),
            (self.isocurve_v(u_end), true, u_end),
        ] {
            if let Ok(curve) = curve
                && let Ok(t) = curve.closest_parameter(target, tolerance)
                && let Ok(point) = curve.evaluate(t)
                && let Ok(distance) = point.distance_to(target)
                && target.compare_distances(point, best_point).is_lt()
            {
                best_point = point;
                best = if fixed_u {
                    (distance, fixed, t)
                } else {
                    (distance, t, fixed)
                };
            }
        }
        let mut refined = false;
        let mut nonfinite_step = false;
        for (_, seed_u, seed_v) in seeds {
            match self.refine_closest_parameters(
                target,
                seed_u,
                seed_v,
                [u_start, u_end],
                [v_start, v_end],
                tolerance,
            ) {
                Ok((u, v, distance)) => {
                    refined = true;
                    let point = self.evaluate(u, v)?;
                    if target.compare_distances(point, best_point).is_lt() {
                        best = (distance, u, v);
                        best_point = point;
                    }
                }
                Err(GeometryError::NonFinite { .. }) => nonfinite_step = true,
                Err(_) => {}
            }
        }
        // Native derivatives/steps can be unrepresentable even though the
        // surface and its closest point are finite. Retry in unit domains only
        // on numerical failure, avoiding a control-net copy on ordinary queries.
        // The unit-domain guard makes this fallback non-recursive in practice.
        if (!refined || nonfinite_step)
            && (u_domain != (0.0..=1.0) || v_domain != (0.0..=1.0))
            && let Ok(normalized) = self.try_reparameterized(0.0..=1.0, 0.0..=1.0)
            && let Ok((u, v)) = normalized.closest_parameters(target, tolerance)
        {
            let u = self.parameter_at_u(u)?;
            let v = self.parameter_at_v(v)?;
            let point = self.evaluate(u, v)?;
            let distance = point.distance_to(target)?;
            if target.compare_distances(point, best_point).is_lt() {
                best = (distance, u, v);
            }
        }
        Ok((best.1, best.2))
    }

    fn refine_closest_parameters(
        &self,
        target: Point3,
        mut u: Real,
        mut v: Real,
        u_domain: [Real; 2],
        v_domain: [Real; 2],
        tolerance: Tolerance,
    ) -> Result<(Real, Real, Real), GeometryError> {
        let mut distance = self.evaluate(u, v)?.distance_to(target)?;
        for _ in 0..64 {
            let (point, derivative_u, derivative_v) = self.evaluate_with_derivatives(u, v)?;
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
            let mut step = 1.0;
            let mut accepted = None;
            for _ in 0..24 {
                let candidate_u = (u + step * delta_u).clamp(u_domain[0], u_domain[1]);
                let candidate_v = (v + step * delta_v).clamp(v_domain[0], v_domain[1]);
                if candidate_u == u && candidate_v == v {
                    break;
                }
                let candidate = self.evaluate(candidate_u, candidate_v)?;
                let candidate_distance = candidate.distance_to(target)?;
                if candidate_distance < distance
                    || (candidate_distance == distance
                        && !target.compare_distances(candidate, point).is_gt())
                {
                    accepted = Some((candidate_u, candidate_v, candidate_distance));
                    break;
                }
                step *= 0.5;
            }
            let Some((next_u, next_v, next_distance)) = accepted else {
                break;
            };
            u = next_u;
            v = next_v;
            distance = next_distance;
        }
        Ok((u, v, distance))
    }
}

/// Divide by a nonzero derivative's norm without forming an overflowing norm.
fn parameter_step(derivative: Vector3, motion: Real) -> Real {
    let values = derivative.to_array();
    let scale = values.into_iter().map(Real::abs).fold(0.0, Real::max);
    let [x, y, z] = values.map(|v| v / scale);
    (motion / x.hypot(y).hypot(z)) / scale
}

fn closest_parameter_seeds(
    spans: impl Iterator<Item = (Real, Real)>,
    domain_start: Real,
    domain_end: Real,
) -> Vec<Real> {
    const MAX_SEEDS: usize = 33;
    let spans = spans.collect::<Vec<_>>();
    let mut seeds = Vec::new();
    if spans.len() <= 10 {
        for (start, end) in spans {
            seeds.extend([start, start * 0.5 + end * 0.5, end]);
        }
    }
    let remaining = MAX_SEEDS.saturating_sub(seeds.len()).max(2);
    for index in 0..remaining {
        let fraction = index as Real / (remaining - 1) as Real;
        seeds.push(domain_start.mul_add(1.0 - fraction, domain_end * fraction));
    }
    seeds.sort_by(Real::total_cmp);
    seeds.dedup();
    seeds
}
