//! Recognition of the exact nine-by-nine rational ring torus control net.

use super::analytic_sphere::same_knot_pattern;
use super::*;

impl NurbsSurface {
    /// Returns the construction frame and radii for a complete canonical
    /// ring torus, allowing affine knot domains and uniformly scaled weights.
    pub fn canonical_torus(
        &self,
        tolerance: Tolerance,
    ) -> Result<Option<(Frame3, Real, Real)>, GeometryError> {
        if (self.degree_u, self.degree_v) != (2, 2)
            || (self.control_point_count_u, self.control_point_count_v) != (9, 9)
        {
            return Ok(None);
        }
        let major_x = midpoint(
            self.control_points[0].point(),
            self.control_points[4 * 9].point(),
        )?;
        let major_minus_x = midpoint(
            self.control_points[4].point(),
            self.control_points[4 * 9 + 4].point(),
        )?;
        let major_y = midpoint(
            self.control_points[2].point(),
            self.control_points[4 * 9 + 2].point(),
        )?;
        let center = midpoint(major_x, major_minus_x)?;
        let x_direction = center.vector_to(major_x)?;
        let y_direction = center.vector_to(major_y)?;
        let major_radius = x_direction.length()?;
        let minor_radius = self.control_points[0]
            .point()
            .distance_to(self.control_points[4 * 9].point())?
            * 0.5;
        if minor_radius <= tolerance.absolute() || major_radius <= minor_radius {
            return Ok(None);
        }
        let Ok(frame) = Frame3::try_from_directions(center, x_direction, y_direction, tolerance)
        else {
            return Ok(None);
        };
        let candidate = Self::try_torus(frame, major_radius, minor_radius)?;
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
        let allowed_position = (tolerance
            .absolute()
            .max(tolerance.relative() * (major_radius + minor_radius))
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
        Ok(Some((frame, major_radius, minor_radius)))
    }
}

fn midpoint(a: Point3, b: Point3) -> Result<Point3, GeometryError> {
    let a = a.to_array();
    let b = b.to_array();
    Point3::try_from(std::array::from_fn(|axis| {
        0.5_f64.mul_add(a[axis], 0.5 * b[axis])
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_rotated_torus_and_rejects_sphere() {
        let frame = Frame3::try_from_normal(
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let torus = NurbsSurface::try_torus(frame, 4.0, 1.0).unwrap();
        let (recovered, major, minor) = torus.canonical_torus(Tolerance::DEFAULT).unwrap().unwrap();
        assert!(recovered.origin().distance_to(frame.origin()).unwrap() < 1e-12);
        assert!((major - 4.0).abs() < 1e-12);
        assert!((minor - 1.0).abs() < 1e-12);

        let sphere = NurbsSurface::try_sphere(frame, 4.0).unwrap();
        assert!(
            sphere
                .canonical_torus(Tolerance::DEFAULT)
                .unwrap()
                .is_none()
        );
    }
}
