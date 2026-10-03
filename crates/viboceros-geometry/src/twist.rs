//! Axis twist maps with finite cubic blending or unbounded angular rate.
use crate::{
    AffineTransform3, Brep, Frame3, GeometryError, NurbsCurve, NurbsSurface, Point3, PointMorph,
    Real, Tolerance, UnitVector3, Vector3, WeightedPoint3, require_finite,
};

#[derive(Clone, Copy, Debug)]
pub struct TwistPointMorph {
    start: Point3,
    axis: UnitVector3,
    length: Real,
    angle: Real,
    infinite: bool,
    preserve_structure: bool,
}

impl TwistPointMorph {
    pub fn try_new(
        start: Point3,
        end: Point3,
        angle_radians: Real,
        infinite: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite([angle_radians], "twist angle")?;
        let direction = start.vector_to(end)?;
        Ok(Self {
            start,
            axis: direction.normalized(tolerance)?,
            length: direction.length()?,
            angle: angle_radians,
            infinite,
            preserve_structure: false,
        })
    }

    /// Move Euclidean controls, retaining degrees, knots, weights and domains.
    /// Multi-face B-reps still use tolerance-driven fitting to keep shared edges.
    pub fn with_preserve_structure(mut self, preserve: bool) -> Self {
        self.preserve_structure = preserve;
        self
    }

    /// Rigid placement around an object's or selected group's bounds center.
    /// Independently inferred from public command outputs: forward differences
    /// of the world axes, averaged with their dual normals, then orthonormalized.
    /// The coordinate-scaled step reproduces the measured numerical pose policy.
    pub fn rigid_transform(self, center: Point3) -> Result<AffineTransform3, GeometryError> {
        if self.angle == 0. {
            return Ok(AffineTransform3::identity());
        }
        let scale = center
            .to_array()
            .into_iter()
            .map(Real::abs)
            .fold(0., Real::max);
        let step = (scale * 1.490116119385e-8 + 2.3283064365386963e-10).sqrt();
        let mapped = self.morph_point(center)?;
        let mut directions = [Vector3::try_new(0., 0., 0.)?; 3];
        for (i, direction) in directions.iter_mut().enumerate() {
            let mut offset = [0.; 3];
            offset[i] = step;
            *direction = mapped
                .vector_to(self.morph_point(center.translated(Vector3::try_from(offset)?)?)?)?
                .normalized_nonzero()?
                .as_vector();
        }
        let [x, y, z] = directions;
        let normal_x = y.cross(z)?.normalized_nonzero()?.as_vector();
        let normal_y = z.cross(x)?.normalized_nonzero()?.as_vector();
        let sum = |a: Vector3, b: Vector3| {
            let a = a.to_array();
            let b = b.to_array();
            Vector3::try_from(std::array::from_fn(|i| a[i] + b[i]))
        };
        let frame = Frame3::try_from_directions(
            mapped,
            sum(x, normal_x)?,
            sum(y, normal_y)?,
            Tolerance::try_new(1e-14, 1e-14, 1e-14)?,
        )?;
        let [x, y, z] = [
            frame.x_axis().as_vector().to_array(),
            frame.y_axis().as_vector().to_array(),
            frame.z_axis().as_vector().to_array(),
        ];
        AffineTransform3::try_mapping_origins(
            std::array::from_fn(|i| [x[i], y[i], z[i]]),
            center,
            mapped,
        )
    }

    /// Rotation about the axis at this point's axial coordinate. Finite twists
    /// use clamped cubic smoothstep, with zero slope at both axis endpoints.
    pub fn rotation_at(self, point: Point3) -> Result<AffineTransform3, GeometryError> {
        if self.angle == 0. {
            return Ok(AffineTransform3::identity());
        }
        let axial = self.start.vector_to(point)?.dot(self.axis.as_vector())?;
        let angle = if self.infinite {
            let fraction = axial / self.length;
            let product = fraction * self.angle;
            if (fraction.is_normal() && product.is_normal()) || axial == 0. {
                product
            } else {
                crate::exact_scalar::scaled_quotient(axial, self.angle, self.length)?
            }
        } else if axial <= 0. {
            0.
        } else if axial >= self.length {
            self.angle
        } else {
            let t = axial / self.length;
            self.angle * (t * t * (3. - 2. * t))
        };
        if angle == 0. {
            Ok(AffineTransform3::identity())
        } else {
            AffineTransform3::try_rotation_with_cardinal_cleanup(self.start, self.axis, angle)
        }
    }
}

impl PointMorph for TwistPointMorph {
    fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
        self.rotation_at(point)?.transform_point(point)
    }

    fn morph_nurbs_curve(
        &self,
        curve: &NurbsCurve,
        tolerance: Tolerance,
    ) -> Result<NurbsCurve, GeometryError> {
        if !self.preserve_structure {
            return crate::morph::fit_curve(self, curve, tolerance);
        }
        NurbsCurve::try_new_rational(
            curve.degree(),
            self.map_controls(curve.control_points())?,
            curve.knots().to_vec(),
        )
    }

    fn morph_nurbs_surface(
        &self,
        surface: &NurbsSurface,
        tolerance: Tolerance,
    ) -> Result<NurbsSurface, GeometryError> {
        if !self.preserve_structure {
            return crate::morph::fit_surface(self, surface, tolerance);
        }
        let (u, v) = (
            surface.control_point_count_u(),
            surface.control_point_count_v(),
        );
        NurbsSurface::try_new_rational(
            surface.degree_u(),
            surface.degree_v(),
            u,
            v,
            self.map_controls(surface.control_points())?,
            surface.knots_u().to_vec(),
            surface.knots_v().to_vec(),
        )
    }

    fn morph_brep(&self, brep: &Brep, tolerance: Tolerance) -> Result<Brep, GeometryError> {
        brep.morphed(
            &self.with_preserve_structure(self.preserve_structure && brep.faces().len() == 1),
            tolerance,
        )
    }
}

impl TwistPointMorph {
    fn map_controls(
        self,
        controls: &[WeightedPoint3],
    ) -> Result<Vec<WeightedPoint3>, GeometryError> {
        controls
            .iter()
            .map(|c| WeightedPoint3::try_new(self.morph_point(c.point())?, c.weight()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    fn point(values: [Real; 3]) -> Point3 {
        Point3::try_from(values).unwrap()
    }
    #[test]
    fn direct_twist_points_match_fourteen_public_native_maps() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/twist_points.json"
        ))
        .unwrap();
        let observations: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/twist_points.json"
        ))
        .unwrap();
        assert_eq!(fixture["operations"].as_array().unwrap().len(), 14);
        assert_eq!(observations["results"].as_array().unwrap().len(), 14);
        for (op, row) in fixture["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(observations["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            let array = |v: &Value| serde_json::from_value::<[Real; 3]>(v.clone()).unwrap();
            let morph = TwistPointMorph::try_new(
                point(array(&op["axis_start"])),
                point(array(&op["axis_end"])),
                op["angle"].as_f64().unwrap(),
                op["infinite"].as_bool().unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap();
            for (source, expected) in op["points"]
                .as_array()
                .unwrap()
                .iter()
                .zip(row["value"]["points"].as_array().unwrap())
            {
                let actual = morph.morph_point(point(array(source))).unwrap();
                for (a, b) in actual.to_array().into_iter().zip(array(expected)) {
                    assert!(
                        (a - b).abs() < 1e-11,
                        "{}: {actual:?} != {expected}",
                        op["id"]
                    );
                }
            }
        }
    }
    #[test]
    fn twist_identity_axis_and_extreme_rate_arithmetic_are_checked() {
        let origin = point([0.; 3]);
        let end = point([0., 0., 1e308]);
        let morph = TwistPointMorph::try_new(origin, end, 1e308, true, Tolerance::DEFAULT).unwrap();
        let p = point([1., 0., 1.]);
        let actual = morph.morph_point(p).unwrap();
        assert!((actual.x() - 1_f64.cos()).abs() < 1e-14);
        assert!((actual.y() - 1_f64.sin()).abs() < 1e-14);
        assert_eq!(actual.z(), 1.);
        assert!(TwistPointMorph::try_new(origin, origin, 0., false, Tolerance::DEFAULT).is_err());
        assert!(
            TwistPointMorph::try_new(origin, end, Real::NAN, false, Tolerance::DEFAULT).is_err()
        );
        let identity = TwistPointMorph::try_new(
            point([-1e308, 0., 0.]),
            point([-1e308, 1., 0.]),
            0.,
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            identity.morph_point(point([1e308, 0., 0.])).unwrap(),
            point([1e308, 0., 0.])
        );
    }
    #[test]
    fn twist_mesh_keeps_colors_ngons_and_faces_while_rebuilding_geometry() {
        use crate::{MeshFace, MeshNgon, TriangleMesh};
        let vertices = [[1., -1., 0.], [3., -1., 0.], [3., 1., 10.], [1., 1., 10.]]
            .into_iter()
            .map(point)
            .collect();
        let colors = vec![
            [10, 20, 30, 40],
            [20, 30, 40, 50],
            [30, 40, 50, 60],
            [40, 50, 60, 70],
        ];
        let mesh = TriangleMesh::try_new_faces(
            vertices,
            vec![MeshFace::Quad([0, 1, 2, 3])],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .try_with_vertex_colors(Some(colors.clone()))
        .unwrap()
        .try_with_ngons(vec![MeshNgon::from_parts(vec![0, 1, 2, 3], vec![0])])
        .unwrap();
        let morph = TwistPointMorph::try_new(
            point([0.; 3]),
            point([0., 0., 10.]),
            std::f64::consts::FRAC_PI_2,
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let result = morph.morph_mesh(&mesh, Tolerance::DEFAULT).unwrap();
        assert_eq!(result.faces(), mesh.faces());
        assert_eq!(result.ngons(), mesh.ngons());
        assert_eq!(result.vertex_colors(), Some(colors.as_slice()));
        assert_ne!(result.bounds(), mesh.bounds());
        for (a, b) in result.vertices().iter().zip(mesh.vertices()) {
            assert_eq!(*a, morph.morph_point(*b).unwrap());
        }
    }
}
