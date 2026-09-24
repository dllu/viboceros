//! Recognition of the exact rational cylinder control layout used by Rhino.

use super::analytic_sphere::same_knot_pattern;
use super::*;

impl NurbsSurface {
    /// Returns a frame at the first rim, radius, and positive height for a
    /// complete nine-by-two rational circular cylinder wall. Reparameterized
    /// knot domains and uniformly scaled weights retain the same recognition.
    pub fn canonical_cylinder(
        &self,
        tolerance: Tolerance,
    ) -> Result<Option<(Frame3, Real, Real)>, GeometryError> {
        if (self.degree_u, self.degree_v) != (2, 1)
            || (self.control_point_count_u, self.control_point_count_v) != (9, 2)
        {
            return Ok(None);
        }
        let first = self.control_points[0].point().to_array();
        let opposite = self.control_points[4].point().to_array();
        let center = Point3::try_from(std::array::from_fn(|axis| {
            0.5_f64.mul_add(first[axis], 0.5 * opposite[axis])
        }))?;
        let far_first = self.control_points[9].point().to_array();
        let far_opposite = self.control_points[13].point().to_array();
        let far_center = Point3::try_from(std::array::from_fn(|axis| {
            0.5_f64.mul_add(far_first[axis], 0.5 * far_opposite[axis])
        }))?;
        let radius_direction = center.vector_to(self.control_points[0].point())?;
        let tangent_direction = center.vector_to(self.control_points[2].point())?;
        let radius = radius_direction.length()?;
        let height = center.distance_to(far_center)?;
        if radius <= tolerance.absolute() || height <= tolerance.absolute() {
            return Ok(None);
        }
        let Ok(frame) =
            Frame3::try_from_directions(center, radius_direction, tangent_direction, tolerance)
        else {
            return Ok(None);
        };
        let candidate = Self::try_cylinder(frame, radius, 0.0, height)?;
        if !same_knot_pattern(&self.knots_u, &candidate.knots_u)
            || !same_knot_pattern(&self.knots_v, &candidate.knots_v)
        {
            return Ok(None);
        }
        let base_weight = self.control_points[0].weight();
        if base_weight == 0.0 {
            return Ok(None);
        }
        let expected_base_weight = candidate.control_points[0].weight();
        let coordinate_scale = center
            .to_array()
            .into_iter()
            .map(Real::abs)
            .fold(0.0, Real::max);
        // Reconstructing a local frame from distant controls loses several
        // ulps of their world coordinates even for an exact source cylinder.
        let allowed_position = (tolerance
            .absolute()
            .max(tolerance.relative() * radius.max(height))
            * 4.0)
            .max(8.0 * Real::EPSILON * coordinate_scale);
        for (actual, expected) in self.control_points.iter().zip(&candidate.control_points) {
            if actual.point().distance_to(expected.point())? > allowed_position
                || ((actual.weight() / base_weight) - (expected.weight() / expected_base_weight))
                    .abs()
                    > 1e-10
            {
                return Ok(None);
            }
        }
        Ok(Some((frame, radius, height)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_rotated_cylinder_but_not_cone() {
        let frame = Frame3::try_from_normal(
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cylinder = NurbsSurface::try_cylinder(frame, 2.0, -1.0, 4.0).unwrap();
        let (start, radius, height) = cylinder
            .canonical_cylinder(Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert!((radius - 2.0).abs() < 1e-12);
        assert!((height - 5.0).abs() < 1e-12);
        assert!(
            start
                .origin()
                .distance_to(frame.point_at([0.0, 0.0, -1.0]).unwrap())
                .unwrap()
                < 1e-12
        );

        let cone = NurbsSurface::try_cone(frame, 2.0, 5.0).unwrap();
        assert!(
            cone.canonical_cylinder(Tolerance::DEFAULT)
                .unwrap()
                .is_none()
        );
    }
}
