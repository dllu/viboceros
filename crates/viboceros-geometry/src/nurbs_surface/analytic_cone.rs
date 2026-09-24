//! Recognition of the exact singular rational cone control layout.

use super::analytic_sphere::same_knot_pattern;
use super::*;

impl NurbsSurface {
    /// Returns the apex frame, base radius, and signed apex-to-base height
    /// for a complete nine-by-two canonical circular cone wall.
    pub fn canonical_cone(
        &self,
        tolerance: Tolerance,
    ) -> Result<Option<(Frame3, Real, Real)>, GeometryError> {
        if (self.degree_u, self.degree_v) != (2, 1)
            || (self.control_point_count_u, self.control_point_count_v) != (9, 2)
        {
            return Ok(None);
        }
        let collapsed_first = self.control_points[0]
            .point()
            .distance_to(self.control_points[4].point())?
            <= tolerance.absolute();
        let collapsed_last = self.control_points[9]
            .point()
            .distance_to(self.control_points[13].point())?
            <= tolerance.absolute();
        if collapsed_first == collapsed_last {
            return Ok(None);
        }
        let (apex_index, base_index, height_sign) = if collapsed_first {
            (0, 9, 1.0)
        } else {
            (9, 0, -1.0)
        };
        let apex = self.control_points[apex_index].point();
        let first = self.control_points[base_index].point().to_array();
        let opposite = self.control_points[base_index + 4].point().to_array();
        let base_center = Point3::try_from(std::array::from_fn(|axis| {
            0.5_f64.mul_add(first[axis], 0.5 * opposite[axis])
        }))?;
        let radius_direction = base_center.vector_to(self.control_points[base_index].point())?;
        let tangent_direction =
            base_center.vector_to(self.control_points[base_index + 2].point())?;
        let radius = radius_direction.length()?;
        let height = apex.distance_to(base_center)?;
        if radius <= tolerance.absolute() || height <= tolerance.absolute() {
            return Ok(None);
        }
        let Ok(frame) =
            Frame3::try_from_directions(apex, radius_direction, tangent_direction, tolerance)
        else {
            return Ok(None);
        };
        let signed_height = height_sign * height;
        let candidate = Self::try_cone(frame, radius, signed_height)?;
        if !same_knot_pattern(&self.knots_u, &candidate.knots_u)
            || !same_knot_pattern(&self.knots_v, &candidate.knots_v)
        {
            return Ok(None);
        }
        let base_weight = self.control_points[base_index].weight();
        if base_weight == 0.0 {
            return Ok(None);
        }
        let expected_base_weight = candidate.control_points[base_index].weight();
        let coordinate_scale = apex
            .to_array()
            .into_iter()
            .chain(base_center.to_array())
            .map(Real::abs)
            .fold(0.0, Real::max);
        // Rebuilding an analytic cone from distant controls loses several
        // ulps of the stored world coordinates.
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
        Ok(Some((frame, radius, signed_height)))
    }

    /// Offsets a canonical cone along its surface normal. The singular apex
    /// becomes a circular rim; retaining the original rational control weights
    /// represents the resulting ruled frustum exactly.
    pub fn try_offset_canonical_cone(
        &self,
        distance: Real,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        let Some((frame, radius, height)) = self.canonical_cone(tolerance)? else {
            return Ok(None);
        };
        require_finite([distance], "cone offset distance")?;
        let u = self.parameter_at_u(0.125)?;
        let v = self.parameter_at_v(0.5)?;
        let point = self.evaluate(u, v)?;
        let local = frame.coordinates_of(point)?;
        let axis_point = frame.point_at([0.0, 0.0, local[2]])?;
        let radial = axis_point.vector_to(point)?.normalized(tolerance)?;
        let normal = self.normal_at(u, v)?.as_vector();
        let radial_distance = distance * normal.dot(radial.as_vector())?;
        let axial_distance = distance * normal.dot(frame.z_axis().as_vector())?;
        let base_center = frame.point_at([0.0, 0.0, height])?;
        let base_row = usize::from(height > 0.0);
        let mut controls = Vec::with_capacity(self.control_points.len());
        for v_index in 0..2 {
            for u_index in 0..9 {
                let control = self.control_points[v_index * 9 + u_index];
                let base_control = self.control_points[base_row * 9 + u_index];
                let radial_control = base_center.vector_to(base_control.point())?;
                let point = control
                    .point()
                    .translated(radial_control.scaled(radial_distance / radius)?)?
                    .translated(frame.z_axis().as_vector().scaled(axial_distance)?)?;
                controls.push(WeightedPoint3::try_new(point, control.weight())?);
            }
        }
        Ok(Some(Self::try_new_rational(
            self.degree_u,
            self.degree_v,
            self.control_point_count_u,
            self.control_point_count_v,
            controls,
            self.knots_u.clone(),
            self.knots_v.clone(),
        )?))
    }

    /// Joins corresponding circular control rows with an exact ruled patch.
    /// `row` selects the apex-side or base-side row in V order.
    pub fn try_cone_offset_cap(
        &self,
        other: &Self,
        row: usize,
    ) -> Result<Option<Self>, GeometryError> {
        if row >= 2
            || self.degree_u != 2
            || self.degree_v != 1
            || self.control_point_count_u != 9
            || self.control_point_count_v != 2
            || self.control_point_count_u != other.control_point_count_u
            || self.control_point_count_v != other.control_point_count_v
            || self.knots_u != other.knots_u
        {
            return Ok(None);
        }
        let mut controls = Vec::with_capacity(18);
        controls.extend_from_slice(&self.control_points[row * 9..row * 9 + 9]);
        controls.extend_from_slice(&other.control_points[row * 9..row * 9 + 9]);
        Ok(Some(Self::try_new_rational(
            2,
            1,
            9,
            2,
            controls,
            self.knots_u.clone(),
            vec![0.0, 0.0, 1.0, 1.0],
        )?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_both_cone_directions_and_rejects_cylinder() {
        let frame = Frame3::try_from_normal(
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        for height in [3.0, -3.0] {
            let cone = NurbsSurface::try_cone(frame, 2.0, height).unwrap();
            let (recovered, radius, recovered_height) =
                cone.canonical_cone(Tolerance::DEFAULT).unwrap().unwrap();
            assert!(recovered.origin().distance_to(frame.origin()).unwrap() < 1e-12);
            assert!((radius - 2.0).abs() < 1e-12);
            assert!((recovered_height - height).abs() < 1e-12);
        }
        let cylinder = NurbsSurface::try_cylinder(frame, 2.0, 0.0, 3.0).unwrap();
        assert!(
            cylinder
                .canonical_cone(Tolerance::DEFAULT)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn cone_offset_opens_apex_and_preserves_exact_normal_distance() {
        let frame = Frame3::try_from_normal(
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cone = NurbsSurface::try_cone(frame, 2.0, 3.0).unwrap();
        let radial_shift = 0.25 * 3.0 / 13.0_f64.sqrt();
        let axial_shift = -0.25 * 2.0 / 13.0_f64.sqrt();
        for signed_distance in [0.25, -0.25] {
            let offset = cone
                .try_offset_canonical_cone(signed_distance, Tolerance::DEFAULT)
                .unwrap()
                .unwrap();
            assert_eq!(offset.domain_u(), cone.domain_u());
            assert_eq!(offset.domain_v(), cone.domain_v());
            let sign = signed_distance.signum();
            let apex = offset.control_points()[0].point().to_array();
            let base = offset.control_points()[9].point().to_array();
            assert!((apex[0] - (1.0 + sign * radial_shift)).abs() < 1e-12);
            assert!((apex[2] - (3.0 + sign * axial_shift)).abs() < 1e-12);
            assert!((base[0] - (3.0 + sign * radial_shift)).abs() < 1e-12);
            assert!((base[2] - (6.0 + sign * axial_shift)).abs() < 1e-12);
            for (u_fraction, v_fraction) in [(0.1, 0.2), (0.25, 0.8), (0.9, 0.5)] {
                let u = cone.parameter_at_u(u_fraction).unwrap();
                let v = cone.parameter_at_v(v_fraction).unwrap();
                let original = cone.evaluate(u, v).unwrap();
                let expected = original
                    .translated(
                        cone.normal_at(u, v)
                            .unwrap()
                            .as_vector()
                            .scaled(signed_distance)
                            .unwrap(),
                    )
                    .unwrap();
                assert!(
                    offset
                        .evaluate(u, v)
                        .unwrap()
                        .distance_to(expected)
                        .unwrap()
                        < 1e-11
                );
            }
        }
    }
}
