//! Recognition of the exact rational sphere control layout used by Rhino.

use super::*;

impl NurbsSurface {
    /// Returns the center and radius when the full surface has the exact
    /// nine-by-five rational sphere layout, allowing affine knot domains and
    /// uniformly rescaled weights. Other sphere parameterizations are not
    /// classified by this conservative recognizer.
    pub fn canonical_sphere(
        &self,
        tolerance: Tolerance,
    ) -> Result<Option<(Point3, Real)>, GeometryError> {
        if (self.degree_u, self.degree_v) != (2, 2)
            || (self.control_point_count_u, self.control_point_count_v) != (9, 5)
        {
            return Ok(None);
        }
        let south = self.control_points[0].point().to_array();
        let north = self.control_points[4 * 9].point().to_array();
        let center = Point3::try_from(std::array::from_fn(|axis| {
            0.5_f64.mul_add(south[axis], 0.5 * north[axis])
        }))?;
        let radial_x = center.vector_to(self.control_points[2 * 9].point())?;
        let radial_y = center.vector_to(self.control_points[2 * 9 + 2].point())?;
        let radius = radial_x.length()?;
        if radius <= tolerance.absolute() {
            return Ok(None);
        }
        let Ok(frame) = Frame3::try_from_directions(center, radial_x, radial_y, tolerance) else {
            return Ok(None);
        };
        let candidate = Self::try_sphere(frame, radius)?;
        if !same_knot_pattern(&self.knots_u, &candidate.knots_u)
            || !same_knot_pattern(&self.knots_v, &candidate.knots_v)
        {
            return Ok(None);
        }
        let base_weight = self.control_points[2 * 9].weight();
        if base_weight == 0.0 {
            return Ok(None);
        }
        let expected_base_weight = candidate.control_points[2 * 9].weight();
        let allowed_position = tolerance.absolute().max(tolerance.relative() * radius) * 4.0;
        for (actual, expected) in self.control_points.iter().zip(&candidate.control_points) {
            if actual.point().distance_to(expected.point())? > allowed_position
                || ((actual.weight() / base_weight) - (expected.weight() / expected_base_weight))
                    .abs()
                    > 1e-10
            {
                return Ok(None);
            }
        }
        Ok(Some((center, radius)))
    }
}

fn same_knot_pattern(actual: &[Real], expected: &[Real]) -> bool {
    if actual.len() != expected.len() {
        return false;
    }
    let actual_span = actual[actual.len() - 1] - actual[0];
    let expected_span = expected[expected.len() - 1] - expected[0];
    actual_span > 0.0
        && expected_span > 0.0
        && actual.iter().zip(expected).all(|(a, b)| {
            ((a - actual[0]) / actual_span - (b - expected[0]) / expected_span).abs() <= 1e-10
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_sphere_and_rejects_ellipsoid() {
        let center = Point3::try_new(1.0, 2.0, 3.0).unwrap();
        let frame = Frame3::try_from_normal(
            center,
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let sphere = NurbsSurface::try_sphere(frame, 2.0).unwrap();
        let (actual_center, radius) = sphere
            .canonical_sphere(Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert!(actual_center.distance_to(center).unwrap() < 1e-12);
        assert!((radius - 2.0).abs() < 1e-12);

        let ellipsoid = NurbsSurface::try_ellipsoid(frame, [2.0, 2.0, 3.0]).unwrap();
        assert!(
            ellipsoid
                .canonical_sphere(Tolerance::DEFAULT)
                .unwrap()
                .is_none()
        );
    }
}
