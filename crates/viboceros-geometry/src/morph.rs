use faer::{Mat, prelude::*};

use crate::{
    Frame3, GeometryError, LineSegment, NurbsCurve, NurbsSurface, Point3, PointCloud3, Polyline3,
    Real, Tolerance, TriangleMesh, UnitVector3, Vector3, WeightedPoint3,
    nurbs::bspline_basis_values, require_finite,
};

const MORPH_CURVE_DEGREE: usize = 3;
const MORPH_CURVE_ERROR_SAMPLES: usize = 16;
const MAX_MORPH_CURVE_CONTROL_POINTS: usize = 100;

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

    fn morph_line(&self, line: LineSegment) -> Result<NurbsCurve, GeometryError> {
        let samples = [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0]
            .into_iter()
            .map(|parameter| {
                line.point_at(parameter)
                    .and_then(|point| self.morph_point(point))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let first = samples[0].to_array();
        let one_third = samples[1].to_array();
        let two_thirds = samples[2].to_array();
        let last = samples[3].to_array();
        let interpolate = |coordinate: usize| {
            let first_equation = 27.0_f64.mul_add(
                one_third[coordinate],
                (-8.0_f64).mul_add(first[coordinate], -last[coordinate]),
            );
            let second_equation = 27.0_f64.mul_add(
                two_thirds[coordinate],
                -first[coordinate] - 8.0 * last[coordinate],
            );
            (
                (2.0 * first_equation - second_equation) / 18.0,
                (2.0 * second_equation - first_equation) / 18.0,
            )
        };
        let first_control =
            Point3::try_from(std::array::from_fn(|coordinate| interpolate(coordinate).0))?;
        let second_control =
            Point3::try_from(std::array::from_fn(|coordinate| interpolate(coordinate).1))?;
        let controls = vec![samples[0], first_control, second_control, samples[3]];
        NurbsCurve::try_new(3, controls, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0])
    }

    fn morph_polyline(&self, polyline: &Polyline3) -> Result<NurbsCurve, GeometryError> {
        let segment_count = polyline.vertices().len() - 1;
        let mut controls = Vec::with_capacity(3 * segment_count + 1);
        for (segment_index, vertices) in polyline.vertices().windows(2).enumerate() {
            let line = LineSegment::from_validated(vertices[0], vertices[1]);
            let segment = self.morph_line(line)?;
            controls.extend(
                segment
                    .control_points()
                    .iter()
                    .skip(usize::from(segment_index > 0))
                    .map(|control| control.point()),
            );
        }
        let mut knots = Vec::with_capacity(controls.len() + 4);
        knots.extend([0.0; 4]);
        for segment in 1..segment_count {
            knots.extend([segment as Real; 3]);
        }
        knots.extend([segment_count as Real; 4]);
        NurbsCurve::try_new(3, controls, knots)
    }

    /// Approximates a deformed curve with Rhino-style adaptive cubic fitting.
    ///
    /// The cubic interpolates the exact morph at its Greville abscissae. Knot
    /// spans whose sampled error exceeds the absolute tolerance are bisected,
    /// retaining the source curve's parameter domain and initial span breaks.
    /// Refinement is bounded at 100 controls to prevent an unbounded dense
    /// linear solve; at that limit the best fitted curve is returned.
    fn morph_nurbs_curve(
        &self,
        curve: &NurbsCurve,
        tolerance: Tolerance,
    ) -> Result<NurbsCurve, GeometryError> {
        let mut breaks = Vec::new();
        for (start, end) in curve.spans() {
            if breaks.last().copied() != Some(start) {
                breaks.push(start);
            }
            breaks.push(end);
        }
        debug_assert!(breaks.len() >= 2);
        if breaks.len() + MORPH_CURVE_DEGREE - 1 > MAX_MORPH_CURVE_CONTROL_POINTS {
            return Err(GeometryError::TooManyMorphCurveControlPoints {
                maximum: MAX_MORPH_CURVE_CONTROL_POINTS,
            });
        }

        loop {
            let approximation = interpolate_morphed_curve(self, curve, &breaks)?;
            let mut refinements = Vec::new();
            for (index, interval) in breaks.windows(2).enumerate() {
                let [start, end] = [interval[0], interval[1]];
                let mut maximum_error = 0.0_f64;
                for sample in 1..MORPH_CURVE_ERROR_SAMPLES {
                    let fraction = sample as Real / MORPH_CURVE_ERROR_SAMPLES as Real;
                    let parameter = stable_lerp(start, end, fraction)?;
                    let exact = self.morph_point(curve.evaluate(parameter)?)?;
                    let fitted = approximation.evaluate(parameter)?;
                    maximum_error = maximum_error.max(exact.distance_to(fitted)?);
                }
                if maximum_error > tolerance.absolute() {
                    refinements.push((index, maximum_error));
                }
            }
            if refinements.is_empty()
                || approximation.control_points().len() >= MAX_MORPH_CURVE_CONTROL_POINTS
            {
                return Ok(approximation);
            }

            let available = MAX_MORPH_CURVE_CONTROL_POINTS - approximation.control_points().len();
            refinements.sort_by(|left, right| right.1.total_cmp(&left.1));
            refinements.truncate(available);
            refinements.sort_unstable_by_key(|refinement| refinement.0);

            let mut refined = Vec::with_capacity(breaks.len() + refinements.len());
            let mut next_refinement = refinements.iter().peekable();
            for (index, interval) in breaks.windows(2).enumerate() {
                refined.push(interval[0]);
                if next_refinement
                    .peek()
                    .is_some_and(|refinement| refinement.0 == index)
                {
                    let midpoint = stable_lerp(interval[0], interval[1], 0.5)?;
                    if midpoint > interval[0] && midpoint < interval[1] {
                        refined.push(midpoint);
                    }
                    next_refinement.next();
                }
            }
            refined.push(*breaks.last().expect("a NURBS curve has a domain"));
            if refined.len() == breaks.len() {
                return Ok(approximation);
            }
            breaks = refined;
        }
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
        TriangleMesh::try_new(
            mesh.vertices()
                .iter()
                .map(|point| self.morph_point(*point))
                .collect::<Result<Vec<_>, _>>()?,
            mesh.triangles().to_vec(),
            tolerance,
        )
    }
}

fn interpolate_morphed_curve(
    morph: &(impl PointMorph + ?Sized),
    curve: &NurbsCurve,
    breaks: &[Real],
) -> Result<NurbsCurve, GeometryError> {
    debug_assert!(breaks.len() >= 2);
    let control_count = breaks.len() + MORPH_CURVE_DEGREE - 1;
    let mut knots = Vec::with_capacity(control_count + MORPH_CURVE_DEGREE + 1);
    knots.extend([breaks[0]; MORPH_CURVE_DEGREE + 1]);
    knots.extend_from_slice(&breaks[1..breaks.len() - 1]);
    knots.extend([breaks[breaks.len() - 1]; MORPH_CURVE_DEGREE + 1]);

    let parameters = (0..control_count)
        .map(|index| stable_mean3(knots[index + 1], knots[index + 2], knots[index + 3]))
        .collect::<Result<Vec<_>, _>>()?;
    let mut basis_rows = Vec::with_capacity(control_count);
    let mut targets = Vec::with_capacity(control_count);
    for parameter in parameters {
        basis_rows.push(bspline_basis_values(
            &knots,
            MORPH_CURVE_DEGREE,
            control_count,
            parameter,
        )?);
        targets.push(morph.morph_point(curve.evaluate(parameter)?)?.to_array());
    }

    let matrix = Mat::from_fn(control_count, control_count, |row, column| {
        basis_rows[row][column]
    });
    let right_hand_side = Mat::from_fn(control_count, 3, |row, column| targets[row][column]);
    let solution = matrix.full_piv_lu().solve(&right_hand_side);
    let controls = (0..control_count)
        .map(|row| Point3::try_new(solution[(row, 0)], solution[(row, 1)], solution[(row, 2)]))
        .collect::<Result<Vec<_>, _>>()?;
    NurbsCurve::try_new(MORPH_CURVE_DEGREE, controls, knots)
}

fn stable_mean3(first: Real, second: Real, third: Real) -> Result<Real, GeometryError> {
    let scale = first.abs().max(second.abs()).max(third.abs());
    if scale == 0.0 {
        return Ok(0.0);
    }
    let normalized_mean = ((first / scale + second / scale) + third / scale) / 3.0;
    let mean = normalized_mean.clamp(-1.0, 1.0) * scale;
    require_finite([mean], "cubic morph Greville parameter")?;
    Ok(mean)
}

fn stable_lerp(start: Real, end: Real, fraction: Real) -> Result<Real, GeometryError> {
    let parameter = start.mul_add(1.0 - fraction, end * fraction);
    require_finite([parameter], "cubic morph sample parameter")?;
    Ok(parameter)
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
    fn line_morph_uses_rhino_style_cubic_deformable_controls() {
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
        let curve = morph.morph_line(line).unwrap();
        assert_eq!(curve.degree(), 3);
        assert_eq!(curve.control_points().len(), 4);
        for (actual, expected) in curve.control_points().iter().zip([
            [9.661579062659536, 2.579513523123856, 4.0],
            [9.33432420167862, 3.8138176137024753, 4.0],
            [8.740602340815528, 5.050606206440483, 4.0],
            [7.901862582717121, 6.128667695662336, 4.0],
        ]) {
            assert_point_near(actual.point(), expected);
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
        assert_eq!(fitted.control_points().len(), 25);
        assert_eq!(
            fitted.knots(),
            &[
                0.0, 0.0, 0.0, 0.0, 0.0625, 0.125, 0.1875, 0.21875, 0.25, 0.28125, 0.3125, 0.375,
                0.4375, 0.46875, 0.5, 0.53125, 0.5625, 0.625, 0.6875, 0.71875, 0.75, 0.78125,
                0.8125, 0.875, 0.9375, 1.0, 1.0, 1.0, 1.0,
            ]
        );
        let maximum_error = (0..=1024)
            .map(|sample| {
                let parameter = sample as Real / 1024.0;
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
        let controls = (0..100)
            .map(|index| point(index as Real, 0.0, 0.0))
            .collect::<Vec<_>>();
        let mut knots = vec![0.0, 0.0];
        knots.extend((1..99).map(|index| index as Real));
        knots.extend([99.0, 99.0]);
        let source = NurbsCurve::try_new(1, controls, knots).unwrap();

        assert_eq!(
            IdentityMorph.morph_nurbs_curve(&source, Tolerance::DEFAULT),
            Err(GeometryError::TooManyMorphCurveControlPoints {
                maximum: MAX_MORPH_CURVE_CONTROL_POINTS,
            })
        );
    }
}
