use crate::{
    GeometryError, MAX_CURVE_DIVISION_POINTS, NurbsCurve, NurbsCurve2, NurbsSurface, Point2,
    Point3, Real, Tolerance, Vector3, require_finite,
};

const PULLBACK_DEGREE: usize = 3;
const PULLBACK_SAMPLES_PER_SPAN: usize = 16;
const MAX_PULLBACK_SUBDIVISION_DEPTH: usize = 20;
const MAX_PULLBACK_SEGMENTS: usize = (MAX_CURVE_DIVISION_POINTS - 1) / PULLBACK_DEGREE;

#[derive(Clone, Copy, Debug)]
struct PullbackNode {
    parameter: Real,
    point: Point2,
    derivative: [Real; 2],
}

#[derive(Clone, Copy, Debug)]
struct PullbackSegment {
    start: Real,
    end: Real,
    controls: [Point2; 4],
}

struct PullbackFitter<'a> {
    surface: &'a NurbsSurface,
    curve: &'a NurbsCurve,
    tolerance: Tolerance,
    numerical_tolerance: Tolerance,
    segments: Vec<PullbackSegment>,
}

fn model_points_near(
    first: Point3,
    second: Point3,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    let scale = first
        .to_array()
        .into_iter()
        .chain(second.to_array())
        .map(Real::abs)
        .fold(1.0_f64, Real::max);
    let allowed = tolerance.absolute().max(tolerance.relative() * scale);
    Ok(first.distance_to(second)? <= allowed)
}

fn snap_domain_roundoff(parameter: Real, domain: [Real; 2]) -> Real {
    let scale = domain[0].abs().max(domain[1].abs()).max(1.0);
    let epsilon = 4096.0 * Real::EPSILON * scale;
    if (parameter - domain[0]).abs() <= epsilon {
        domain[0]
    } else if (parameter - domain[1]).abs() <= epsilon {
        domain[1]
    } else {
        parameter
    }
}

fn numerical_pullback_tolerance(tolerance: Tolerance) -> Result<Tolerance, GeometryError> {
    // Keep closest-point iteration noise well below the model-space budget
    // used to accept a fitted span. Angular degeneracy remains the caller's
    // policy because tightening it could invert a nearly singular surface.
    let tighten = |value: Real| {
        let tightened = value * 1.0e-4;
        if tightened > 0.0 { tightened } else { value }
    };
    Tolerance::try_new(
        tighten(tolerance.absolute()),
        tighten(tolerance.relative()),
        tolerance.angular(),
    )
}

fn subtract_scaled_vector(
    vector: Vector3,
    direction: Vector3,
    scale: Real,
) -> Result<Vector3, GeometryError> {
    let vector = vector.to_array();
    let direction = direction.to_array();
    Vector3::try_new(
        (-scale).mul_add(direction[0], vector[0]),
        (-scale).mul_add(direction[1], vector[1]),
        (-scale).mul_add(direction[2], vector[2]),
    )
}

fn pullback_derivative(
    surface: &NurbsSurface,
    point: Point2,
    model_derivative: Vector3,
    tolerance: Tolerance,
) -> Result<[Real; 2], GeometryError> {
    let (_, derivative_u, derivative_v) =
        surface.evaluate_with_derivatives(point.x(), point.y())?;
    let u_speed = derivative_u.length()?;
    let u_axis = derivative_u.normalized(tolerance)?.as_vector();
    let v_along_u = derivative_v.dot(u_axis)?;
    let v_perpendicular = subtract_scaled_vector(derivative_v, u_axis, v_along_u)?;
    let v_speed = v_perpendicular.length()?;
    let v_axis = v_perpendicular.normalized(tolerance)?.as_vector();
    let derivative_v_parameter = model_derivative.dot(v_axis)? / v_speed;
    let derivative_u_parameter =
        (model_derivative.dot(u_axis)? - v_along_u * derivative_v_parameter) / u_speed;
    let derivative = [derivative_u_parameter, derivative_v_parameter];
    require_finite(derivative, "surface pullback derivative")?;
    Ok(derivative)
}

fn pullback_node(
    surface: &NurbsSurface,
    curve: &NurbsCurve,
    parameter: Real,
    derivative_parameter: Real,
    tolerance: Tolerance,
    numerical_tolerance: Tolerance,
) -> Result<PullbackNode, GeometryError> {
    let model_point = curve.evaluate(parameter)?;
    let (u, v) = surface.closest_parameters(model_point, numerical_tolerance)?;
    let domain_u = [*surface.domain_u().start(), *surface.domain_u().end()];
    let domain_v = [*surface.domain_v().start(), *surface.domain_v().end()];
    let point = Point2::try_new(
        snap_domain_roundoff(u, domain_u),
        snap_domain_roundoff(v, domain_v),
    )?;
    if !model_points_near(
        surface.evaluate(point.x(), point.y())?,
        model_point,
        tolerance,
    )? {
        return Err(GeometryError::InvalidControlNet {
            context: "curve must lie on the surface for parameter-space pullback",
        });
    }
    let (_, model_derivative) = curve.evaluate_with_derivative(derivative_parameter)?;
    Ok(PullbackNode {
        parameter,
        point,
        derivative: pullback_derivative(surface, point, model_derivative, numerical_tolerance)?,
    })
}

fn hermite_segment(
    start: PullbackNode,
    end: PullbackNode,
) -> Result<PullbackSegment, GeometryError> {
    let extent = end.parameter - start.parameter;
    require_finite([extent], "surface pullback span extent")?;
    if extent <= 0.0 {
        return Err(GeometryError::InvalidKnotVector {
            context: "surface pullback span must advance",
        });
    }
    let handle_scale = extent / PULLBACK_DEGREE as Real;
    let first_handle = Point2::try_new(
        start.derivative[0].mul_add(handle_scale, start.point.x()),
        start.derivative[1].mul_add(handle_scale, start.point.y()),
    )?;
    let second_handle = Point2::try_new(
        (-end.derivative[0]).mul_add(handle_scale, end.point.x()),
        (-end.derivative[1]).mul_add(handle_scale, end.point.y()),
    )?;
    Ok(PullbackSegment {
        start: start.parameter,
        end: end.parameter,
        controls: [start.point, first_handle, second_handle, end.point],
    })
}

fn evaluate_cubic(controls: [Point2; 4], parameter: Real) -> Result<Point2, GeometryError> {
    let complement = 1.0 - parameter;
    let start_weight = complement * complement * complement;
    let first_weight = 3.0 * parameter * complement * complement;
    let second_weight = 3.0 * parameter * parameter * complement;
    let end_weight = parameter * parameter * parameter;
    let coordinate = |value: fn(Point2) -> Real| {
        value(controls[0]).mul_add(
            start_weight,
            value(controls[1]).mul_add(
                first_weight,
                value(controls[2]).mul_add(second_weight, value(controls[3]) * end_weight),
            ),
        )
    };
    Point2::try_new(coordinate(Point2::x), coordinate(Point2::y))
}

fn segment_matches_curve(
    surface: &NurbsSurface,
    curve: &NurbsCurve,
    segment: PullbackSegment,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    for sample in 1..PULLBACK_SAMPLES_PER_SPAN {
        let normalized = sample as Real / PULLBACK_SAMPLES_PER_SPAN as Real;
        let parameter = segment
            .start
            .mul_add(1.0 - normalized, segment.end * normalized);
        let uv = evaluate_cubic(segment.controls, normalized)?;
        let Ok(surface_point) = surface.evaluate(uv.x(), uv.y()) else {
            return Ok(false);
        };
        if !model_points_near(surface_point, curve.evaluate(parameter)?, tolerance)? {
            return Ok(false);
        }
    }
    Ok(true)
}

impl PullbackFitter<'_> {
    fn append_span(
        &mut self,
        start: PullbackNode,
        end: PullbackNode,
        depth: usize,
    ) -> Result<(), GeometryError> {
        let candidate = hermite_segment(start, end)?;
        if segment_matches_curve(self.surface, self.curve, candidate, self.tolerance)? {
            if self.segments.len() == MAX_PULLBACK_SEGMENTS {
                return Err(GeometryError::TooManySurfacePullbackControlPoints {
                    maximum: MAX_CURVE_DIVISION_POINTS,
                });
            }
            self.segments.push(candidate);
            return Ok(());
        }
        if depth == MAX_PULLBACK_SUBDIVISION_DEPTH {
            return Err(GeometryError::SurfacePullbackDidNotConverge {
                tolerance: self.tolerance.absolute(),
            });
        }
        let middle_parameter = start.parameter.mul_add(0.5, end.parameter * 0.5);
        if middle_parameter == start.parameter || middle_parameter == end.parameter {
            return Err(GeometryError::SurfacePullbackDidNotConverge {
                tolerance: self.tolerance.absolute(),
            });
        }
        let middle = pullback_node(
            self.surface,
            self.curve,
            middle_parameter,
            middle_parameter,
            self.tolerance,
            self.numerical_tolerance,
        )?;
        self.append_span(start, middle, depth + 1)?;
        self.append_span(middle, end, depth + 1)
    }
}

fn piecewise_cubic(segments: &[PullbackSegment]) -> Result<NurbsCurve2, GeometryError> {
    let first = segments.first().ok_or(GeometryError::InvalidControlNet {
        context: "surface pullback requires at least one nonempty span",
    })?;
    let last = segments
        .last()
        .expect("a first segment implies a last segment");
    let control_count = segments
        .len()
        .checked_mul(PULLBACK_DEGREE)
        .and_then(|count| count.checked_add(1))
        .filter(|count| *count <= MAX_CURVE_DIVISION_POINTS)
        .ok_or(GeometryError::TooManySurfacePullbackControlPoints {
            maximum: MAX_CURVE_DIVISION_POINTS,
        })?;
    let knot_count = control_count.checked_add(PULLBACK_DEGREE + 1).ok_or(
        GeometryError::TooManySurfacePullbackControlPoints {
            maximum: MAX_CURVE_DIVISION_POINTS,
        },
    )?;
    let mut controls = Vec::with_capacity(control_count);
    let mut knots = Vec::with_capacity(knot_count);
    knots.extend(std::iter::repeat_n(first.start, PULLBACK_DEGREE + 1));
    controls.extend(first.controls);
    for segment in &segments[1..] {
        knots.extend(std::iter::repeat_n(segment.start, PULLBACK_DEGREE));
        controls.extend_from_slice(&segment.controls[1..]);
    }
    knots.extend(std::iter::repeat_n(last.end, PULLBACK_DEGREE + 1));
    NurbsCurve2::try_new(PULLBACK_DEGREE, controls, knots)
}

fn parameter_curve_matches(
    surface: &NurbsSurface,
    curve: &NurbsCurve,
    parameter_curve: &NurbsCurve2,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    for (span_start, span_end) in parameter_curve.spans() {
        for sample in 0..=PULLBACK_SAMPLES_PER_SPAN {
            let normalized = sample as Real / PULLBACK_SAMPLES_PER_SPAN as Real;
            let parameter = span_start.mul_add(1.0 - normalized, span_end * normalized);
            let uv = parameter_curve.evaluate(parameter)?;
            if !model_points_near(
                surface.evaluate(uv.x(), uv.y())?,
                curve.evaluate(parameter)?,
                tolerance,
            )? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

impl NurbsSurface {
    /// Pulls a model-space NURBS curve into this surface's parameter space.
    ///
    /// Exact affine, projective, and eligible bilinear inverses retain the
    /// source NURBS structure. Other regular parameterizations use adaptive
    /// piecewise-cubic Hermite pullback. Every fitted span is verified in
    /// model space at the supplied tolerance, and the source parameter domain
    /// is retained.
    pub fn try_pullback_curve(
        &self,
        curve: &NurbsCurve,
        tolerance: Tolerance,
    ) -> Result<NurbsCurve2, GeometryError> {
        if let Ok(exact) = self.try_pullback_exact_curve(curve, tolerance) {
            return Ok(exact);
        }

        let curve_domain = curve.domain();
        let domain_start = *curve_domain.start();
        let domain_end = *curve_domain.end();
        let mut fitter = PullbackFitter {
            surface: self,
            curve,
            tolerance,
            numerical_tolerance: numerical_pullback_tolerance(tolerance)?,
            segments: Vec::new(),
        };
        for (span_start, span_end) in curve.spans() {
            let derivative_start = if span_start == domain_start {
                span_start
            } else {
                span_start.next_up().min(span_end)
            };
            let derivative_end = if span_end == domain_end {
                span_end
            } else {
                span_end.next_down().max(span_start)
            };
            let start = pullback_node(
                self,
                curve,
                span_start,
                derivative_start,
                tolerance,
                fitter.numerical_tolerance,
            )?;
            let end = pullback_node(
                self,
                curve,
                span_end,
                derivative_end,
                tolerance,
                fitter.numerical_tolerance,
            )?;
            fitter.append_span(start, end, 0)?;
        }
        let parameter_curve = piecewise_cubic(&fitter.segments)?;
        if !parameter_curve_matches(self, curve, &parameter_curve, tolerance)? {
            return Err(GeometryError::SurfacePullbackDidNotConverge {
                tolerance: tolerance.absolute(),
            });
        }
        Ok(parameter_curve)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AffineTransform3, WeightedPoint3};

    fn point(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.0).unwrap()
    }

    fn trapezoid_surface() -> NurbsSurface {
        NurbsSurface::try_bilinear([
            point(0.0, 0.0),
            point(10.0, 0.0),
            point(8.0, 10.0),
            point(0.0, 10.0),
        ])
        .unwrap()
    }

    #[test]
    fn fitted_pullback_recovers_curved_parameters_on_a_nonaffine_plane() {
        let surface = trapezoid_surface();
        let curve = NurbsCurve::try_new(
            3,
            vec![
                point(0.0, 2.0),
                point(3.2, 20.0 / 3.0),
                point(82.0 / 15.0, 8.0),
                point(8.8, 6.0),
            ],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        assert!(
            surface
                .try_pullback_exact_curve(&curve, Tolerance::DEFAULT)
                .is_err()
        );

        let parameter_curve = surface
            .try_pullback_curve(&curve, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(parameter_curve.degree(), 3);
        assert_eq!(parameter_curve.control_points().len(), 4);
        assert_eq!(parameter_curve.domain(), curve.domain());
        for (actual, expected) in parameter_curve.control_points().iter().zip([
            [0.0, 0.2],
            [1.0 / 3.0, 2.0 / 3.0],
            [2.0 / 3.0, 0.8],
            [1.0, 0.6],
        ]) {
            assert!(Tolerance::DEFAULT.approx_eq(actual.point().x(), expected[0]));
            assert!(Tolerance::DEFAULT.approx_eq(actual.point().y(), expected[1]));
            assert_eq!(actual.weight(), 1.0);
        }
        for sample in 0..=64 {
            let parameter = sample as Real / 64.0;
            let uv = parameter_curve.evaluate(parameter).unwrap();
            assert!(
                surface
                    .evaluate(uv.x(), uv.y())
                    .unwrap()
                    .is_near(curve.evaluate(parameter).unwrap(), Tolerance::DEFAULT)
            );
        }

        let reparameterized = surface
            .try_reparameterized(-2.0..=4.0, 10.0..=20.0)
            .unwrap();
        let reparameterized_curve = reparameterized
            .try_pullback_curve(&curve, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(reparameterized_curve.control_points().len(), 4);
        for (actual, expected) in reparameterized_curve.control_points().iter().zip([
            [0.0, 0.2],
            [1.0 / 3.0, 2.0 / 3.0],
            [2.0 / 3.0, 0.8],
            [1.0, 0.6],
        ]) {
            assert!(Tolerance::DEFAULT.approx_eq(actual.point().x(), 6.0 * expected[0] - 2.0));
            assert!(Tolerance::DEFAULT.approx_eq(actual.point().y(), 10.0 * expected[1] + 10.0));
        }

        let transform = AffineTransform3::try_new(
            [[2.0, -1.0, 0.5], [0.25, 3.0, -0.5], [0.5, 0.75, 4.0]],
            Vector3::try_new(4.0, -2.0, 7.0).unwrap(),
        )
        .unwrap();
        let transformed_surface = reparameterized.transformed(transform).unwrap();
        let transformed_curve = curve.transformed(transform).unwrap();
        let transformed_pullback = transformed_surface
            .try_pullback_curve(&transformed_curve, Tolerance::DEFAULT)
            .unwrap();
        for (actual, expected) in transformed_pullback
            .control_points()
            .iter()
            .zip(reparameterized_curve.control_points())
        {
            assert!(actual.point().is_near(expected.point(), Tolerance::DEFAULT));
        }
    }

    #[test]
    fn fitted_pullback_subdivides_and_rejects_off_surface_curves() {
        let surface = trapezoid_surface();
        let curve = NurbsCurve::try_new(
            2,
            vec![point(0.0, 2.0), point(5.0, 9.0), point(8.8, 6.0)],
            vec![2.0, 2.0, 2.0, 5.0, 5.0, 5.0],
        )
        .unwrap();
        let parameter_curve = surface
            .try_pullback_curve(&curve, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(parameter_curve.degree(), 3);
        assert!(parameter_curve.control_points().len() > 4);
        assert_eq!(parameter_curve.domain(), curve.domain());
        for sample in 0..=128 {
            let parameter = 2.0 + 3.0 * sample as Real / 128.0;
            let uv = parameter_curve.evaluate(parameter).unwrap();
            assert!(
                surface
                    .evaluate(uv.x(), uv.y())
                    .unwrap()
                    .is_near(curve.evaluate(parameter).unwrap(), Tolerance::DEFAULT)
            );
        }

        let off_surface = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 2.0),
                Point3::try_new(5.0, 9.0, 0.01).unwrap(),
                point(8.8, 6.0),
            ],
            vec![2.0, 2.0, 2.0, 5.0, 5.0, 5.0],
        )
        .unwrap();
        assert!(matches!(
            surface.try_pullback_curve(&off_surface, Tolerance::DEFAULT),
            Err(GeometryError::InvalidControlNet { .. })
        ));
    }

    #[test]
    fn fitted_pullback_handles_a_rational_nonprojective_surface() {
        let surface_controls = [
            WeightedPoint3::try_new(point(0.0, 0.0), 1.0).unwrap(),
            WeightedPoint3::try_new(point(10.0, 0.0), 2.0).unwrap(),
            WeightedPoint3::try_new(point(0.0, 10.0), 1.5).unwrap(),
            WeightedPoint3::try_new(point(8.0, 10.0), 3.0).unwrap(),
        ];
        let surface = NurbsSurface::try_new_rational(
            1,
            1,
            2,
            2,
            surface_controls.to_vec(),
            vec![0.0, 0.0, 1.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();

        // Compose the homogeneous bilinear surface with
        // u=s, v=0.2+1.4s-s^2, then convert the resulting cubic power basis
        // to Bezier controls. This gives an exact rational spatial cutter
        // whose known UV preimage is polynomial even though the surface does
        // not have a projective inverse.
        let homogeneous = surface_controls.map(|control| {
            let point = control.point();
            let weight = control.weight();
            [point.x() * weight, point.y() * weight, 0.0, weight]
        });
        let origin = homogeneous[0];
        let axis_u: [Real; 4] =
            std::array::from_fn(|coordinate| homogeneous[1][coordinate] - origin[coordinate]);
        let axis_v: [Real; 4] =
            std::array::from_fn(|coordinate| homogeneous[2][coordinate] - origin[coordinate]);
        let twist: [Real; 4] = std::array::from_fn(|coordinate| {
            homogeneous[3][coordinate] - homogeneous[1][coordinate] - homogeneous[2][coordinate]
                + origin[coordinate]
        });
        let power = [
            std::array::from_fn(|coordinate| axis_v[coordinate].mul_add(0.2, origin[coordinate])),
            std::array::from_fn(|coordinate| {
                twist[coordinate].mul_add(0.2, axis_v[coordinate].mul_add(1.4, axis_u[coordinate]))
            }),
            std::array::from_fn(|coordinate| twist[coordinate].mul_add(1.4, -axis_v[coordinate])),
            twist.map(|coordinate| -coordinate),
        ];
        let curve_homogeneous = [
            power[0],
            std::array::from_fn(|coordinate| {
                power[1][coordinate].mul_add(1.0 / 3.0, power[0][coordinate])
            }),
            std::array::from_fn(|coordinate| {
                power[2][coordinate].mul_add(
                    1.0 / 3.0,
                    power[1][coordinate].mul_add(2.0 / 3.0, power[0][coordinate]),
                )
            }),
            std::array::from_fn(|coordinate| {
                power[3][coordinate]
                    + power[2][coordinate]
                    + power[1][coordinate]
                    + power[0][coordinate]
            }),
        ];
        let curve = NurbsCurve::try_new_rational(
            3,
            curve_homogeneous
                .map(|control| {
                    WeightedPoint3::try_new(
                        Point3::try_new(
                            control[0] / control[3],
                            control[1] / control[3],
                            control[2] / control[3],
                        )
                        .unwrap(),
                        control[3],
                    )
                    .unwrap()
                })
                .to_vec(),
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        assert!(
            surface
                .try_pullback_exact_curve(&curve, Tolerance::DEFAULT)
                .is_err()
        );

        let parameter_curve = surface
            .try_pullback_curve(&curve, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(parameter_curve.control_points().len(), 4);
        for (actual, expected) in parameter_curve.control_points().iter().zip([
            [0.0, 0.2],
            [1.0 / 3.0, 2.0 / 3.0],
            [2.0 / 3.0, 0.8],
            [1.0, 0.6],
        ]) {
            assert!(Tolerance::DEFAULT.approx_eq(actual.point().x(), expected[0]));
            assert!(Tolerance::DEFAULT.approx_eq(actual.point().y(), expected[1]));
        }
        for sample in 0..=64 {
            let parameter = sample as Real / 64.0;
            let uv = parameter_curve.evaluate(parameter).unwrap();
            assert!(
                surface
                    .evaluate(uv.x(), uv.y())
                    .unwrap()
                    .is_near(curve.evaluate(parameter).unwrap(), Tolerance::DEFAULT)
            );
        }
    }
}
