mod curve_fit;

use crate::{
    Frame3, GeometryError, LineSegment, NurbsCurve, NurbsSurface, Point3, PointCloud3, Polyline3,
    Real, Tolerance, TriangleMesh, UnitVector3, Vector3, WeightedPoint3, require_finite,
};

/// Resource ceiling for adaptive cubic fitting and point-map sampling.
pub const MAX_MORPH_CURVE_CONTROL_POINTS: usize = 512;

/// A deterministic non-affine mapping of finite Euclidean points.
///
/// Default helpers preserve mesh and surface control-net topology. Curves are
/// made deformable as cubic NURBS curves, following Rhino's space-morph
/// behavior rather than reducing a nonlinear result to mapped controls.
pub trait PointMorph {
    fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError>;

    fn morph_point_cloud(&self, cloud: &PointCloud3) -> Result<PointCloud3, GeometryError> {
        PointCloud3::try_new(
            cloud
                .points()
                .iter()
                .map(|point| self.morph_point(*point))
                .collect::<Result<Vec<_>, _>>()?,
        )
    }

    /// Fits a nonlinear line image to the document's absolute tolerance.
    fn morph_line(
        &self,
        line: LineSegment,
        tolerance: Tolerance,
    ) -> Result<NurbsCurve, GeometryError> {
        self.morph_nurbs_curve(&line.to_nurbs()?, tolerance)
    }

    /// Preserves native vertex parameters and C0 joins while refining interiors.
    fn morph_polyline(
        &self,
        polyline: &Polyline3,
        tolerance: Tolerance,
    ) -> Result<NurbsCurve, GeometryError> {
        let parameters = polyline.parameters();
        let mut knots = Vec::with_capacity(parameters.len() + 2);
        knots.push(parameters[0]);
        knots.extend_from_slice(parameters);
        knots.push(parameters[parameters.len() - 1]);
        let source = NurbsCurve::try_new(1, polyline.vertices().to_vec(), knots)?;
        self.morph_nurbs_curve(&source, tolerance)
    }

    /// Adaptive native-parameter cubic fit, retaining source knot continuity
    /// and exact one-sided endpoint values. Error checks are sampled, not a
    /// continuous certificate for an arbitrary black-box morph. Exhausted work
    /// or representable parameter resolution returns an error, never a knowingly
    /// out-of-tolerance approximation.
    fn morph_nurbs_curve(
        &self,
        curve: &NurbsCurve,
        tolerance: Tolerance,
    ) -> Result<NurbsCurve, GeometryError> {
        curve_fit::fit(self, curve, tolerance, MAX_MORPH_CURVE_CONTROL_POINTS)
    }

    fn morph_nurbs_surface(&self, surface: &NurbsSurface) -> Result<NurbsSurface, GeometryError> {
        let controls = surface
            .control_points()
            .iter()
            .map(|control| {
                WeightedPoint3::try_new(self.morph_point(control.point())?, control.weight())
            })
            .collect::<Result<Vec<_>, _>>()?;
        NurbsSurface::try_new_rational(
            surface.degree_u(),
            surface.degree_v(),
            surface.control_point_count_u(),
            surface.control_point_count_v(),
            controls,
            surface.knots_u().to_vec(),
            surface.knots_v().to_vec(),
        )
    }

    fn morph_mesh(
        &self,
        mesh: &TriangleMesh,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        TriangleMesh::try_new_faces(
            mesh.vertices()
                .iter()
                .map(|point| self.morph_point(*point))
                .collect::<Result<Vec<_>, _>>()?,
            mesh.faces().to_vec(),
            tolerance,
        )
    }
}

/// Rhino-compatible plane-to-surface ("splop") point morph.
///
/// Plane x/y distances are converted into target surface parameter offsets by
/// inverting the first surface differential at the picked UV. The target
/// surface and its normal are then evaluated at the offset parameters.
#[derive(Clone, Debug)]
pub struct SurfacePointMorph<'a> {
    source: Frame3,
    surface: &'a NurbsSurface,
    target_u: Real,
    target_v: Real,
    u_per_x: Real,
    u_per_y: Real,
    v_per_y: Real,
    scale: Real,
    sine: Real,
    cosine: Real,
    flip: bool,
    constrained_normal: Option<UnitVector3>,
    tolerance: Tolerance,
}

impl<'a> SurfacePointMorph<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        source: Frame3,
        surface: &'a NurbsSurface,
        target_u: Real,
        target_v: Real,
        scale: Real,
        angle_radians: Real,
        flip: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite(
            [target_u, target_v, scale, angle_radians],
            "surface point morph definition",
        )?;
        if scale <= 0.0 {
            return Err(GeometryError::Degenerate {
                context: "surface point morph scale",
            });
        }
        let (_, derivative_u, derivative_v) =
            surface.evaluate_with_derivatives(target_u, target_v)?;
        let x_axis = derivative_u.normalized(tolerance)?;
        let u_speed = derivative_u.length()?;
        let v_along_x = derivative_v.dot(x_axis.as_vector())?;
        let derivative_v_values = derivative_v.to_array();
        let x_values = x_axis.as_vector().to_array();
        let v_perpendicular = Vector3::try_new(
            (-v_along_x).mul_add(x_values[0], derivative_v_values[0]),
            (-v_along_x).mul_add(x_values[1], derivative_v_values[1]),
            (-v_along_x).mul_add(x_values[2], derivative_v_values[2]),
        )?;
        let v_speed = v_perpendicular.length()?;
        if v_speed <= tolerance.absolute() {
            return Err(GeometryError::Degenerate {
                context: "surface point morph target frame",
            });
        }
        let (sine, cosine) = angle_radians.sin_cos();
        let u_per_x = 1.0 / u_speed;
        let u_per_y = -v_along_x / (u_speed * v_speed);
        let v_per_y = 1.0 / v_speed;
        require_finite(
            [u_per_x, u_per_y, v_per_y, sine, cosine],
            "surface point morph differential",
        )?;
        Ok(Self {
            source,
            surface,
            target_u,
            target_v,
            u_per_x,
            u_per_y,
            v_per_y,
            scale,
            sine,
            cosine,
            flip,
            constrained_normal: None,
            tolerance,
        })
    }

    /// Uses a fixed placement construction-plane normal instead of the
    /// varying target surface normal.
    pub fn with_constrained_normal(mut self, normal: Vector3) -> Result<Self, GeometryError> {
        self.constrained_normal = Some(normal.normalized(self.tolerance)?);
        Ok(self)
    }

    #[inline]
    pub const fn target_parameters(&self) -> (Real, Real) {
        (self.target_u, self.target_v)
    }
}

impl PointMorph for SurfacePointMorph<'_> {
    fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
        let offset = self.source.origin().vector_to(point)?;
        let source_x = offset.dot(self.source.x_axis().as_vector())?;
        let source_y = offset.dot(self.source.y_axis().as_vector())?;
        let source_z = offset.dot(self.source.z_axis().as_vector())?;
        let target_x = self.scale * self.cosine.mul_add(source_x, -self.sine * source_y);
        let mut target_y = self.scale * self.sine.mul_add(source_x, self.cosine * source_y);
        let mut target_z = self.scale * source_z;
        if self.flip {
            target_y = -target_y;
            target_z = -target_z;
        }
        require_finite(
            [target_x, target_y, target_z],
            "surface point morph coordinate",
        )?;
        let u = self
            .u_per_x
            .mul_add(target_x, self.u_per_y.mul_add(target_y, self.target_u));
        let v = self.v_per_y.mul_add(target_y, self.target_v);
        let (surface_point, derivative_u, derivative_v) =
            self.surface.evaluate_extended_with_derivatives(u, v)?;
        let normal = if let Some(normal) = self.constrained_normal {
            normal
        } else {
            derivative_u
                .cross(derivative_v)?
                .normalized(self.tolerance)?
        };
        surface_point.translated(normal.as_vector().scaled(target_z)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Circle3;

    struct IdentityMorph;

    impl PointMorph for IdentityMorph {
        fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
            Ok(point)
        }
    }

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn quarter_cylinder() -> NurbsSurface {
        let middle_weight = 0.5_f64.sqrt();
        let mut controls = Vec::new();
        for z in [0.0, 10.0] {
            controls.extend([
                WeightedPoint3::try_new(point(10.0, 0.0, z), 1.0).unwrap(),
                WeightedPoint3::try_new(point(10.0, 10.0, z), middle_weight).unwrap(),
                WeightedPoint3::try_new(point(0.0, 10.0, z), 1.0).unwrap(),
            ]);
        }
        NurbsSurface::try_new_rational(
            2,
            1,
            3,
            2,
            controls,
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap()
    }

    fn assert_point_near(actual: Point3, expected: [Real; 3]) {
        for (actual, expected) in actual.to_array().into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() <= 2.0e-12,
                "{actual} != {expected}"
            );
        }
    }

    #[test]
    fn surface_point_morph_matches_rhino_splop_on_a_rational_cylinder() {
        let surface = quarter_cylinder();
        let origin = point(1.0, 2.0, 3.0);
        let source = Frame3::try_from_directions(
            origin,
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let morph = SurfacePointMorph::try_new(
            source,
            &surface,
            0.3,
            0.4,
            1.0,
            0.0,
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        for (offset, expected) in [
            ([0.0, 0.0, 0.0], [8.973756499953726, 4.412674277525846, 4.0]),
            ([1.0, 0.0, 0.0], [8.484425274005353, 5.292875189329445, 4.0]),
            ([0.0, 1.0, 0.0], [8.973756499953724, 4.412674277525845, 5.0]),
            ([0.0, 0.0, 1.0], [9.8711321499491, 4.85394170527843, 4.0]),
            (
                [2.0, -1.5, 0.75],
                [8.494502276420908, 6.5883177728370095, 2.5],
            ),
            (
                [-4.0, 3.0, -2.0],
                [7.977767111967475, 0.5960133448255271, 7.0],
            ),
        ] {
            let point = origin
                .translated(Vector3::try_from(offset).unwrap())
                .unwrap();
            assert_point_near(morph.morph_point(point).unwrap(), expected);
        }

        let constrained = morph
            .with_constrained_normal(Vector3::try_new(0.0, 0.0, 1.0).unwrap())
            .unwrap();
        let source_normal_end = origin
            .translated(Vector3::try_new(0.0, 0.0, 1.0).unwrap())
            .unwrap();
        assert_point_near(
            constrained.morph_point(source_normal_end).unwrap(),
            [8.973756499953726, 4.412674277525846, 5.0],
        );
    }

    #[test]
    fn line_morph_refines_the_cylinder_image_to_document_tolerance() {
        let surface = quarter_cylinder();
        let origin = point(1.0, 2.0, 3.0);
        let source = Frame3::try_from_directions(
            origin,
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let morph = SurfacePointMorph::try_new(
            source,
            &surface,
            0.3,
            0.4,
            1.0,
            0.0,
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let line = LineSegment::try_new(
            point(-1.0, 2.0, 3.0),
            point(3.0, 2.0, 3.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let curve = morph.morph_line(line, Tolerance::DEFAULT).unwrap();
        assert_eq!(curve.degree(), 3);
        assert!(curve.control_points().len() > 4);
        // A single cubic interpolates four stations but misses the nonlinear
        // image between them. Control equality with a loose Rhino fit cannot
        // establish the requested geometric tolerance.
        for i in 0..=2048 {
            let s = i as Real / 2048.0;
            let actual = curve.evaluate(curve.parameter_at(s).unwrap()).unwrap();
            let expected = morph.morph_point(line.point_at(s).unwrap()).unwrap();
            assert!(actual.distance_to(expected).unwrap() <= Tolerance::DEFAULT.absolute());
        }
    }

    #[test]
    fn nurbs_curve_morph_matches_rhino_adaptive_cubic_fit() {
        let tolerance = Tolerance::try_new(1.0e-3, 1.0e-12, 1.0e-10).unwrap();
        let surface = quarter_cylinder();
        let origin = point(1.0, 2.0, 3.0);
        let source_frame = Frame3::try_from_directions(
            origin,
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            tolerance,
        )
        .unwrap();
        let morph = SurfacePointMorph::try_new(
            source_frame,
            &surface,
            0.3,
            0.4,
            1.0,
            0.0,
            false,
            tolerance,
        )
        .unwrap();
        let source = NurbsCurve::try_new(
            3,
            vec![
                point(-1.0, 1.0, 3.25),
                point(0.5, 4.0, 2.5),
                point(1.75, 0.0, 4.0),
                point(3.0, 3.0, 3.25),
            ],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();

        let fitted = morph.morph_nurbs_curve(&source, tolerance).unwrap();
        assert_eq!(fitted.degree(), 3);
        assert!(!fitted.is_rational());
        assert_eq!(
            fitted.knots(),
            &[0.0, 0.0, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0, 1.0, 1.0]
        );
        for (actual, expected) in fitted.control_points().iter().zip([
            [9.903118539226027, 2.6440013612019504, 3.0],
            [9.62694536417517, 2.9507091324819466, 3.75],
            [9.323490683416267, 3.576627826611515, 4.375],
            [9.172855614886387, 4.610892997949948, 4.0],
            [8.916649937470847, 5.634336473858159, 3.625],
            [8.448980236942445, 6.137312585428537, 4.25],
            [8.09940914728505, 6.281884388053891, 5.0],
        ]) {
            assert_point_near(actual.point(), expected);
        }
    }

    #[test]
    fn rational_circle_morph_refines_source_spans_to_tolerance() {
        let tolerance = Tolerance::try_new(1.0e-3, 1.0e-12, 1.0e-10).unwrap();
        let surface = quarter_cylinder();
        let origin = point(1.0, 2.0, 3.0);
        let source_frame = Frame3::try_from_directions(
            origin,
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            tolerance,
        )
        .unwrap();
        let morph = SurfacePointMorph::try_new(
            source_frame,
            &surface,
            0.3,
            0.4,
            1.0,
            0.0,
            false,
            tolerance,
        )
        .unwrap();
        let normal = UnitVector3::try_new(0.0, 0.0, 1.0, tolerance).unwrap();
        let source = Circle3::try_new(origin, 1.0, normal, tolerance)
            .unwrap()
            .to_nurbs()
            .unwrap();

        let fitted = morph.morph_nurbs_curve(&source, tolerance).unwrap();
        assert_eq!(fitted.degree(), 3);
        assert!(!fitted.is_rational());
        assert_eq!(fitted.domain(), source.domain());
        for (start, _) in source.spans() {
            assert!(fitted.knots().contains(&start));
        }
        let maximum_error = (0..=1024)
            .map(|sample| {
                let parameter = source.parameter_at(sample as Real / 1024.0).unwrap();
                let exact = morph
                    .morph_point(source.evaluate(parameter).unwrap())
                    .unwrap();
                exact
                    .distance_to(fitted.evaluate(parameter).unwrap())
                    .unwrap()
            })
            .fold(0.0_f64, Real::max);
        assert!(maximum_error <= tolerance.absolute(), "{maximum_error}");
    }

    #[test]
    fn curve_morph_handles_a_parameter_domain_whose_width_overflows() {
        let source = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0, 0.0), point(1.0, 2.0, 3.0)],
            vec![-Real::MAX, -Real::MAX, Real::MAX, Real::MAX],
        )
        .unwrap();
        let fitted = IdentityMorph
            .morph_nurbs_curve(&source, Tolerance::DEFAULT)
            .unwrap();

        assert_eq!(fitted.domain(), -Real::MAX..=Real::MAX);
        assert_eq!(fitted.control_points().len(), 4);
        assert_point_near(fitted.evaluate(-Real::MAX).unwrap(), [0.0, 0.0, 0.0]);
        assert_point_near(fitted.evaluate(Real::MAX).unwrap(), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn curve_morph_rejects_an_initial_fit_over_the_control_budget() {
        let controls = (0..MAX_MORPH_CURVE_CONTROL_POINTS)
            .map(|index| point(index as Real, 0.0, 0.0))
            .collect::<Vec<_>>();
        let mut knots = vec![0.0, 0.0];
        knots.extend((1..MAX_MORPH_CURVE_CONTROL_POINTS - 1).map(|index| index as Real));
        knots.extend([(MAX_MORPH_CURVE_CONTROL_POINTS - 1) as Real; 2]);
        let source = NurbsCurve::try_new(1, controls, knots).unwrap();

        assert_eq!(
            IdentityMorph.morph_nurbs_curve(&source, Tolerance::DEFAULT),
            Err(GeometryError::TooManyMorphCurveControlPoints {
                maximum: MAX_MORPH_CURVE_CONTROL_POINTS,
            })
        );
    }
}
