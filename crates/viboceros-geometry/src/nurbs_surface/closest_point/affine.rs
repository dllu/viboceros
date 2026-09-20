//! Direct closest points on exactly affine, uniformly weighted bilinear patches.
use super::*;
use crate::FiniteSum;

#[cfg(test)]
mod tests;

impl NurbsSurface {
    /// An exact representation guard, followed by a floating-point convex solve.
    /// Unsupported representations or unrepresentable intermediates fall back.
    pub(super) fn closest_affine_parameters(&self, target: Point3) -> Option<(Real, Real)> {
        if self.degree_u != 1
            || self.degree_v != 1
            || self.control_point_count_u != 2
            || self.control_point_count_v != 2
        {
            return None;
        }
        let weight = self.control_points[0].weight();
        if weight == 0. || self.control_points.iter().any(|p| p.weight() != weight) {
            return None;
        }
        let [origin, east, north, opposite] =
            std::array::from_fn(|i| self.control_points[i].point());
        // Do not use model tolerance or equality of rounded diagonal sums here:
        // even a sub-ulp warp must not silently become an affine patch.
        for axis in 0..3 {
            let mut sum = FiniteSum::default();
            for value in [
                origin.to_array()[axis],
                opposite.to_array()[axis],
                -east.to_array()[axis],
                -north.to_array()[axis],
            ] {
                sum.add(value).ok()?;
            }
            if sum.total().ok()? != 0. {
                return None;
            }
        }

        let u_axis = origin.vector_to(east).ok()?;
        let v_axis = origin.vector_to(north).ok()?;
        let u_length = u_axis.length().ok()?;
        let v_length = v_axis.length().ok()?;
        let x = u_axis.normalized_nonzero().ok()?.as_vector();
        let v_direction = v_axis.normalized_nonzero().ok()?.as_vector();
        let normal = x.cross(v_direction).ok()?.normalized_nonzero().ok()?;
        let y = normal
            .as_vector()
            .cross(x)
            .ok()?
            .normalized_nonzero()
            .ok()?
            .as_vector();
        let sine = v_direction.dot(y).ok()?;
        if sine <= 0. {
            return None;
        }
        let tangent_x = x.dot_point_difference(target, origin);
        let tangent_y = y.dot_point_difference(target, origin);
        if !tangent_x.is_finite() || !tangent_y.is_finite() {
            return None;
        }

        let mut best: Option<(Point3, (Real, Real))> = None;
        let mut consider = |u: Real, v: Real| -> Option<()> {
            let u = self.parameter_at_u(u).ok()?;
            let v = self.parameter_at_v(v).ok()?;
            // Evaluate the original representation, including native-domain
            // rounding, and retain the same exact distance ordering as fallback.
            let point = self.evaluate(u, v).ok()?;
            if best.is_none_or(|(previous, _)| target.compare_distances(point, previous).is_lt()) {
                best = Some((point, (u, v)));
            }
            Some(())
        };

        // Every constrained minimum is either the unconstrained plane projection
        // or a minimum on one of the four finite edges (including their corners).
        let v_motion = tangent_y / sine;
        let v = v_motion / v_length;
        if (0.0..=1.0).contains(&v) {
            let u = (-v_direction.dot(x).ok()?).mul_add(v_motion, tangent_x) / u_length;
            if (0.0..=1.0).contains(&u) {
                consider(u, v)?;
            }
        }
        for (start, fixed, along_u) in [
            (origin, 0., true),
            (north, 1., true),
            (origin, 0., false),
            (east, 1., false),
        ] {
            let (direction, length) = if along_u {
                (x, u_length)
            } else {
                (v_direction, v_length)
            };
            let t = (direction.dot_point_difference(target, start) / length).clamp(0., 1.);
            if along_u {
                consider(t, fixed)?;
            } else {
                consider(fixed, t)?;
            }
        }
        best.map(|(_, parameters)| parameters)
    }
}
