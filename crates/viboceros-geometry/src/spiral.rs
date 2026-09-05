use std::f64::consts::TAU;

use crate::{
    CurveRef, CurveSample, Frame3, GeometryError, NurbsCurve, Point3, Real, Tolerance, UnitVector3,
    Vector3, require_finite,
};

const CUBIC_DEGREE: usize = 3;
const SHORT_SPIRAL_SPANS_PER_TURN: Real = 36.0;
const LONG_SPIRAL_SPANS_PER_TURN: Real = 24.0;
const MIN_SPIRAL_SPANS: usize = 4;

/// Resource ceiling for one spiral or helix construction.
pub const MAX_SPIRAL_CONTROL_POINTS: usize = 1_000_000;

/// RhinoCommon's recommended interpolation density for a swept spiral.
pub const DEFAULT_SWEPT_SPIRAL_POINTS_PER_TURN: usize = 12;

/// Smallest interpolation density accepted by RhinoCommon's swept overload.
pub const MIN_SWEPT_SPIRAL_POINTS_PER_TURN: usize = 5;

impl NurbsCurve {
    /// Constructs a C2 uniform cubic approximation of an axial spiral.
    ///
    /// Radius changes linearly from `radii[0]` to `radii[1]`, while `height`
    /// advances along the frame Z axis. Positive turns rotate from frame X
    /// toward frame Y; negative turns reverse that twist. The knot domain is
    /// `[0, abs(turns)]`. As in Rhino's straight-axis spiral constructor,
    /// varying-radius spirals use 36 spans per turn. Constant-radius positive
    /// helices of at least one turn use 24; shorter or reverse helices retain
    /// the 36-span density. Fractional span counts are rounded upward.
    ///
    /// Every span-boundary sample lies on the analytic spiral. Analytic end
    /// tangents close the interpolation system, and its strictly diagonally
    /// dominant tridiagonal form is solved in linear time.
    pub fn try_spiral(
        frame: Frame3,
        height: Real,
        turns: Real,
        radii: [Real; 2],
    ) -> Result<Self, GeometryError> {
        require_finite([height, turns, radii[0], radii[1]], "spiral dimensions")?;
        if turns == 0.0 || (radii[0] == 0.0 && radii[1] == 0.0) {
            return Err(GeometryError::InvalidSpiralDimensions);
        }

        let turn_count = turns.abs();
        let spans_per_turn = if radii[0] == radii[1] && turns >= 1.0 {
            LONG_SPIRAL_SPANS_PER_TURN
        } else {
            SHORT_SPIRAL_SPANS_PER_TURN
        };
        let requested_spans = spans_per_turn * turn_count;
        let maximum_spans = MAX_SPIRAL_CONTROL_POINTS - CUBIC_DEGREE;
        if !requested_spans.is_finite() || requested_spans.ceil() > maximum_spans as Real {
            return Err(GeometryError::TooManySpiralControlPoints {
                maximum: MAX_SPIRAL_CONTROL_POINTS,
            });
        }
        let span_count = (requested_spans.ceil() as usize).max(MIN_SPIRAL_SPANS);
        let parameter_step = turn_count / span_count as Real;
        let angle_step = turns.signum() * TAU / span_count as Real * turn_count;

        let samples = (0..=span_count)
            .map(|index| {
                let fraction = index as Real / span_count as Real;
                spiral_point(frame, height, turns, radii, fraction)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let radial_rate = (radii[1] - radii[0]) / turn_count;
        let axial_rate = height / turn_count;
        require_finite(
            [parameter_step, angle_step, radial_rate, axial_rate],
            "spiral parameterization",
        )?;
        let direction = turns.signum();
        let start_derivative =
            frame_vector(frame, radial_rate, direction * TAU * radii[0], axial_rate)?;
        let end_angle = turns * TAU;
        let (end_sine, end_cosine) = end_angle.sin_cos();
        let end_derivative = frame_vector(
            frame,
            radial_rate.mul_add(end_cosine, -direction * TAU * radii[1] * end_sine),
            radial_rate.mul_add(end_sine, direction * TAU * radii[1] * end_cosine),
            axial_rate,
        )?;
        let handle_scale = parameter_step / CUBIC_DEGREE as Real;
        let start_handle = samples[0].translated(start_derivative.scaled(handle_scale)?)?;
        let end_handle = samples[span_count].translated(end_derivative.scaled(-handle_scale)?)?;

        let interior =
            solve_uniform_cubic_controls(&samples, start_handle, end_handle, span_count)?;
        let mut controls = Vec::with_capacity(span_count + CUBIC_DEGREE);
        controls.push(samples[0]);
        controls.push(start_handle);
        controls.extend(interior);
        controls.push(end_handle);
        controls.push(samples[span_count]);

        let mut knots = Vec::with_capacity(controls.len() + CUBIC_DEGREE + 1);
        knots.extend([0.0; CUBIC_DEGREE + 1]);
        knots.extend((1..span_count).map(|index| parameter_step * index as Real));
        knots.extend([turn_count; CUBIC_DEGREE + 1]);
        NurbsCurve::try_new(CUBIC_DEGREE, controls, knots)
    }

    /// Constructs a constant-radius axial helix.
    pub fn try_helix(
        frame: Frame3,
        radius: Real,
        height: Real,
        turns: Real,
    ) -> Result<Self, GeometryError> {
        require_finite([radius, height], "helix dimensions")?;
        if radius <= 0.0 || height <= 0.0 {
            return Err(GeometryError::InvalidSpiralDimensions);
        }
        Self::try_spiral(frame, height, turns, [radius, radius])
    }

    /// Constructs a C2 uniform cubic spiral swept around a rail curve.
    ///
    /// Stations divide the complete rail into equal arc-length intervals. A
    /// rotation-minimizing frame is seeded by the perpendicular component of
    /// `radius_point - rail.start`, transported along the rail, and twisted by
    /// `turns * 2π`. Radius varies linearly between the two endpoints.
    /// Negative turns and radii are supported, matching RhinoCommon's swept
    /// spiral constructor. `points_per_turn` must be at least five; 12 is the
    /// recommended default.
    pub fn try_swept_spiral(
        rail: CurveRef<'_>,
        radius_point: Point3,
        turns: Real,
        radii: [Real; 2],
        points_per_turn: usize,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite([turns, radii[0], radii[1]], "swept spiral dimensions")?;
        if turns == 0.0 || (radii[0] == 0.0 && radii[1] == 0.0) {
            return Err(GeometryError::InvalidSpiralDimensions);
        }
        if points_per_turn < MIN_SWEPT_SPIRAL_POINTS_PER_TURN {
            return Err(GeometryError::InvalidSweptSpiralPointsPerTurn {
                actual: points_per_turn,
            });
        }

        let turn_count = turns.abs();
        let requested_spans = turn_count * points_per_turn as Real;
        let maximum_spans = MAX_SPIRAL_CONTROL_POINTS - CUBIC_DEGREE;
        if !requested_spans.is_finite() || requested_spans.ceil() > maximum_spans as Real {
            return Err(GeometryError::TooManySpiralControlPoints {
                maximum: MAX_SPIRAL_CONTROL_POINTS,
            });
        }
        let span_count = (requested_spans.ceil() as usize).max(MIN_SPIRAL_SPANS);
        let rail_length = rail.length(tolerance)?;
        if rail_length <= tolerance.absolute() {
            return Err(GeometryError::Degenerate {
                context: "swept spiral rail",
            });
        }
        let rail_samples = rail.divide_by_count_samples(span_count, true, tolerance)?;
        let frames = swept_spiral_frames(rail, &rail_samples, radius_point, tolerance)?;

        let samples = rail_samples
            .iter()
            .zip(&frames)
            .enumerate()
            .map(|(index, (rail_sample, frame))| {
                let fraction = index as Real / span_count as Real;
                let angle = turns * TAU * fraction;
                let radius = (radii[1] - radii[0]).mul_add(fraction, radii[0]);
                let radial = rotated_swept_axis(*frame, angle, false)?;
                rail_sample.point().translated(radial.scaled(radius)?)
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;

        let start_curvature = rail.curvature_vector(rail_samples[0].parameter())?;
        let end_curvature = rail.curvature_vector(rail_samples[span_count].parameter())?;
        let start_derivative = swept_spiral_endpoint_derivative(
            frames[0],
            start_curvature,
            rail_length,
            turns,
            radii,
            0.0,
        )?;
        let end_derivative = swept_spiral_endpoint_derivative(
            frames[span_count],
            end_curvature,
            rail_length,
            turns,
            radii,
            1.0,
        )?;
        let handle_scale = 1.0 / (CUBIC_DEGREE * span_count) as Real;
        let start_handle = samples[0].translated(start_derivative.scaled(handle_scale)?)?;
        let end_handle = samples[span_count].translated(end_derivative.scaled(-handle_scale)?)?;

        let interior =
            solve_uniform_cubic_controls(&samples, start_handle, end_handle, span_count)?;
        let mut controls = Vec::with_capacity(span_count + CUBIC_DEGREE);
        controls.push(samples[0]);
        controls.push(start_handle);
        controls.extend(interior);
        controls.push(end_handle);
        controls.push(samples[span_count]);

        // Rhino's swept overload uses this scale for its otherwise-uniform
        // knot vector. It does not affect the locus, but retaining it makes
        // edited control data and downstream parameter values interoperable.
        let mean_radius = radii[0].abs() * 0.5 + radii[1].abs() * 0.5;
        let domain_end = mean_radius * (rail_length + TAU * turn_count);
        require_finite([domain_end], "swept spiral parameterization")?;
        if domain_end <= 0.0 {
            return Err(GeometryError::InvalidSpiralDimensions);
        }
        let parameter_step = domain_end / span_count as Real;
        let mut knots = Vec::with_capacity(controls.len() + CUBIC_DEGREE + 1);
        knots.extend([0.0; CUBIC_DEGREE + 1]);
        knots.extend((1..span_count).map(|index| parameter_step * index as Real));
        knots.extend([domain_end; CUBIC_DEGREE + 1]);
        Self::try_new(CUBIC_DEGREE, controls, knots)
    }
}

#[derive(Clone, Copy, Debug)]
struct SweptSpiralFrame {
    radial: UnitVector3,
    angular: UnitVector3,
    tangent: UnitVector3,
}

fn swept_spiral_frames(
    rail: CurveRef<'_>,
    samples: &[CurveSample],
    radius_point: Point3,
    tolerance: Tolerance,
) -> Result<Vec<SweptSpiralFrame>, GeometryError> {
    let tangent = samples[0].tangent();
    let seed = samples[0].point().vector_to(radius_point)?;
    let tangent_projection = seed.dot(tangent.as_vector())?;
    let tangent_component = tangent.as_vector().scaled(tangent_projection)?;
    let radial = subtract_vectors(seed, tangent_component)?.normalized(tolerance)?;
    let parameters = samples.iter().map(|s| s.parameter()).collect::<Vec<_>>();
    Ok(rail
        .rotation_minimizing_frames(
            &parameters,
            Some(radial),
            crate::FrameTransportOptions::default(),
        )?
        .into_iter()
        .map(|f| SweptSpiralFrame {
            radial: f.x_axis(),
            angular: f.y_axis(),
            tangent: f.z_axis(),
        })
        .collect())
}

fn subtract_vectors(left: Vector3, right: Vector3) -> Result<Vector3, GeometryError> {
    Vector3::try_new(
        left.x() - right.x(),
        left.y() - right.y(),
        left.z() - right.z(),
    )
}

fn rotated_swept_axis(
    frame: SweptSpiralFrame,
    angle: Real,
    derivative: bool,
) -> Result<Vector3, GeometryError> {
    let (sine, cosine) = angle.sin_cos();
    let (radial_scale, angular_scale) = if derivative {
        (-sine, cosine)
    } else {
        (cosine, sine)
    };
    let radial = frame.radial.as_vector().to_array();
    let angular = frame.angular.as_vector().to_array();
    Vector3::try_new(
        radial_scale.mul_add(radial[0], angular_scale * angular[0]),
        radial_scale.mul_add(radial[1], angular_scale * angular[1]),
        radial_scale.mul_add(radial[2], angular_scale * angular[2]),
    )
}

fn swept_spiral_endpoint_derivative(
    frame: SweptSpiralFrame,
    curvature: Vector3,
    rail_length: Real,
    turns: Real,
    radii: [Real; 2],
    fraction: Real,
) -> Result<Vector3, GeometryError> {
    let angle = turns * TAU * fraction;
    let radius = (radii[1] - radii[0]).mul_add(fraction, radii[0]);
    let radial = rotated_swept_axis(frame, angle, false)?;
    let angular = rotated_swept_axis(frame, angle, true)?;
    let tangent_scale = (-radius).mul_add(curvature.dot(radial)?, rail_length);
    let radial_scale = radii[1] - radii[0];
    let angular_scale = turns * TAU * radius;
    let tangent = frame.tangent.as_vector().to_array();
    let radial = radial.to_array();
    let angular = angular.to_array();
    Vector3::try_new(
        tangent_scale.mul_add(
            tangent[0],
            radial_scale.mul_add(radial[0], angular_scale * angular[0]),
        ),
        tangent_scale.mul_add(
            tangent[1],
            radial_scale.mul_add(radial[1], angular_scale * angular[1]),
        ),
        tangent_scale.mul_add(
            tangent[2],
            radial_scale.mul_add(radial[2], angular_scale * angular[2]),
        ),
    )
}

fn spiral_point(
    frame: Frame3,
    height: Real,
    turns: Real,
    radii: [Real; 2],
    fraction: Real,
) -> Result<Point3, GeometryError> {
    let angle = turns * TAU * fraction;
    let (sine, cosine) = angle.sin_cos();
    let radius = (radii[1] - radii[0]).mul_add(fraction, radii[0]);
    frame_point(frame, radius * cosine, radius * sine, height * fraction)
}

fn frame_point(frame: Frame3, x: Real, y: Real, z: Real) -> Result<Point3, GeometryError> {
    frame
        .origin()
        .translated(frame.x_axis().as_vector().scaled(x)?)?
        .translated(frame.y_axis().as_vector().scaled(y)?)?
        .translated(frame.z_axis().as_vector().scaled(z)?)
}

fn frame_vector(frame: Frame3, x: Real, y: Real, z: Real) -> Result<crate::Vector3, GeometryError> {
    let x_axis = frame.x_axis().as_vector().to_array();
    let y_axis = frame.y_axis().as_vector().to_array();
    let z_axis = frame.z_axis().as_vector().to_array();
    crate::Vector3::try_new(
        x.mul_add(x_axis[0], y.mul_add(y_axis[0], z * z_axis[0])),
        x.mul_add(x_axis[1], y.mul_add(y_axis[1], z * z_axis[1])),
        x.mul_add(x_axis[2], y.mul_add(y_axis[2], z * z_axis[2])),
    )
}

fn solve_uniform_cubic_controls(
    samples: &[Point3],
    start_handle: Point3,
    end_handle: Point3,
    span_count: usize,
) -> Result<Vec<Point3>, GeometryError> {
    debug_assert_eq!(samples.len(), span_count + 1);
    debug_assert!(span_count >= MIN_SPIRAL_SPANS);
    let unknown_count = span_count - 1;
    let mut diagonal = vec![4.0; unknown_count];
    diagonal[0] = 7.0;
    diagonal[unknown_count - 1] = 7.0;
    let mut upper = vec![1.0; unknown_count - 1];
    upper[0] = 2.0;
    let mut rhs = Vec::with_capacity(unknown_count);
    for (sample_index, sample_point) in samples.iter().enumerate().take(span_count).skip(1) {
        let sample = sample_point.to_array();
        let mut value = sample.map(|coordinate| 6.0 * coordinate);
        if sample_index == 1 {
            let handle = start_handle.to_array();
            value = std::array::from_fn(|axis| 2.0 * value[axis] - 3.0 * handle[axis]);
        } else if sample_index == span_count - 1 {
            let handle = end_handle.to_array();
            value = std::array::from_fn(|axis| 2.0 * value[axis] - 3.0 * handle[axis]);
        }
        rhs.push(value);
    }

    for row in 1..unknown_count {
        let lower = if row == unknown_count - 1 { 2.0 } else { 1.0 };
        let factor: Real = lower / diagonal[row - 1];
        diagonal[row] -= factor * upper[row - 1];
        let previous = rhs[row - 1];
        for (coordinate, previous) in rhs[row].iter_mut().zip(previous) {
            *coordinate = (-factor).mul_add(previous, *coordinate);
        }
    }
    for coordinate in &mut rhs[unknown_count - 1] {
        *coordinate /= diagonal[unknown_count - 1];
    }
    for row in (0..unknown_count - 1).rev() {
        let next = rhs[row + 1];
        for (coordinate, next) in rhs[row].iter_mut().zip(next) {
            *coordinate = (-upper[row]).mul_add(next, *coordinate) / diagonal[row];
        }
    }
    rhs.into_iter().map(Point3::try_from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LineSegment, Tolerance, Vector3};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn world_frame() -> Frame3 {
        Frame3::try_from_normal(
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    fn assert_point_near(actual: Point3, expected: Point3, tolerance: Real) {
        assert!(
            actual.distance_to(expected).unwrap() <= tolerance,
            "actual {actual:?}, expected {expected:?}"
        );
    }

    #[test]
    fn helix_matches_uniform_rhino_sampling_and_analytic_end_tangents() {
        let curve = NurbsCurve::try_helix(world_frame(), 2.0, 10.0, 3.0).unwrap();
        assert_eq!(curve.degree(), 3);
        assert!(!curve.is_rational());
        assert_eq!(curve.control_points().len(), 75);
        assert_eq!(curve.domain(), 0.0..=3.0);
        assert_eq!(curve.knots().len(), 79);
        assert_eq!(&curve.knots()[..4], &[0.0; 4]);
        assert_eq!(&curve.knots()[75..], &[3.0; 4]);

        for index in 0..=72 {
            let parameter = index as Real / 24.0;
            let angle = TAU * parameter;
            assert_point_near(
                curve.evaluate(parameter).unwrap(),
                Point3::try_new(
                    2.0 * angle.cos(),
                    2.0 * angle.sin(),
                    10.0 * index as Real / 72.0,
                )
                .unwrap(),
                2.0e-12,
            );
        }

        let (_, derivative) = curve.evaluate_with_derivative(0.0).unwrap();
        let expected = [0.0, 4.0 * std::f64::consts::PI, 10.0 / 3.0];
        for (actual, expected) in derivative.to_array().into_iter().zip(expected) {
            assert!((actual - expected).abs() <= 2.0e-12);
        }
    }

    #[test]
    fn fractional_and_reverse_spirals_use_the_documented_span_policy() {
        let reverse = NurbsCurve::try_spiral(world_frame(), 3.3, -1.1, [2.0, 4.0]).unwrap();
        assert_eq!(reverse.control_points().len(), 43);
        assert_eq!(reverse.domain(), 0.0..=1.1);
        assert_point_near(
            reverse.evaluate(0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            1.0e-14,
        );
        assert_point_near(
            reverse.evaluate(1.1).unwrap(),
            Point3::try_new(4.0 * (TAU * 1.1).cos(), -4.0 * (TAU * 1.1).sin(), 3.3).unwrap(),
            2.0e-12,
        );

        let short = NurbsCurve::try_helix(world_frame(), 1.0, 0.3, 0.1).unwrap();
        assert_eq!(short.control_points().len(), 7);
        assert_eq!(short.knots().len(), 11);

        let expanding = NurbsCurve::try_spiral(world_frame(), 6.0, 2.0, [1.0, 4.0]).unwrap();
        assert_eq!(expanding.control_points().len(), 75);
        assert_eq!(expanding.knots().len(), 79);
    }

    #[test]
    fn spiral_supports_zero_and_negative_endpoint_radii() {
        let curve = NurbsCurve::try_spiral(world_frame(), 0.0, 2.0, [0.0, -3.0]).unwrap();
        assert_point_near(
            curve.evaluate(0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            1.0e-14,
        );
        assert_point_near(
            curve.evaluate(2.0).unwrap(),
            Point3::try_new(-3.0, 0.0, 0.0).unwrap(),
            2.0e-12,
        );
    }

    #[test]
    fn spiral_rejects_degenerate_non_finite_and_unbounded_input() {
        let frame = world_frame();
        for result in [
            NurbsCurve::try_spiral(frame, 1.0, 0.0, [1.0, 1.0]),
            NurbsCurve::try_spiral(frame, 1.0, 1.0, [0.0, 0.0]),
            NurbsCurve::try_spiral(frame, Real::NAN, 1.0, [1.0, 1.0]),
            NurbsCurve::try_helix(frame, 0.0, 1.0, 1.0),
            NurbsCurve::try_helix(frame, 1.0, 0.0, 1.0),
        ] {
            assert!(result.is_err());
        }
        assert_eq!(
            NurbsCurve::try_spiral(frame, 1.0, 1.0e100, [1.0, 1.0]),
            Err(GeometryError::TooManySpiralControlPoints {
                maximum: MAX_SPIRAL_CONTROL_POINTS,
            })
        );
    }

    #[test]
    fn swept_spiral_matches_rhino_line_rail_layout() {
        let rail = LineSegment::try_new(
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 10.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let curve = NurbsCurve::try_swept_spiral(
            CurveRef::Line(&rail),
            point(1.0, 0.0, 0.0),
            1.0,
            [1.0, 1.0],
            DEFAULT_SWEPT_SPIRAL_POINTS_PER_TURN,
            Tolerance::DEFAULT,
        )
        .unwrap();

        assert_eq!(curve.degree(), 3);
        assert!(!curve.is_rational());
        assert_eq!(curve.control_points().len(), 15);
        assert_eq!(curve.domain(), 0.0..=10.0 + TAU);
        assert_point_near(
            curve.control_points()[0].point(),
            point(1.0, 0.0, 0.0),
            1.0e-14,
        );
        assert_point_near(
            curve.control_points()[1].point(),
            point(1.0, std::f64::consts::PI / 18.0, 10.0 / 36.0),
            2.0e-15,
        );
        for index in 0..=12 {
            let fraction = index as Real / 12.0;
            let parameter = (10.0 + TAU) * fraction;
            assert_point_near(
                curve.evaluate(parameter).unwrap(),
                point(
                    (TAU * fraction).cos(),
                    (TAU * fraction).sin(),
                    10.0 * fraction,
                ),
                3.0e-12,
            );
        }
    }

    #[test]
    fn swept_spiral_uses_equal_arc_rail_stations_and_rhino_endpoint_tangents() {
        let rail = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 0.0, 0.0),
                point(5.0, 0.0, 0.0),
                point(5.0, 5.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let curve = NurbsCurve::try_swept_spiral(
            CurveRef::NurbsCurve(&rail),
            point(0.0, 0.0, 2.0),
            1.0,
            [1.0, 2.0],
            12,
            Tolerance::DEFAULT,
        )
        .unwrap();

        assert_eq!(curve.control_points().len(), 15);
        assert!((*curve.domain().end() - 21.598967232867093).abs() < 5.0e-8);
        assert_point_near(
            curve.control_points()[1].point(),
            point(0.22544794948329108, -0.174532925199433, 1.0277777777777777),
            2.0e-9,
        );
        assert_point_near(
            curve.control_points()[13].point(),
            point(4.650934149601133, 4.774552050516709, 1.972222222222222),
            2.0e-9,
        );
        assert_point_near(
            curve.evaluate(*curve.domain().end() * 0.5).unwrap(),
            point(3.749999992296474, 1.2499999922964744, -1.5),
            3.0e-8,
        );
    }

    #[test]
    fn swept_spiral_transports_its_seed_frame_on_a_spatial_rail() {
        let rail = NurbsCurve::try_new(
            3,
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 2.0),
                point(4.0, 4.0, 4.0),
                point(0.0, 5.0, 6.0),
            ],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let curve = NurbsCurve::try_swept_spiral(
            CurveRef::NurbsCurve(&rail),
            point(0.0, 1.0, 0.0),
            1.0,
            [1.0, 1.0],
            12,
            Tolerance::DEFAULT,
        )
        .unwrap();

        assert_point_near(
            curve.evaluate(*curve.domain().end()).unwrap(),
            point(-0.3817921072075612, 5.251548256397855, 5.11064165738595),
            3.0e-8,
        );
        assert_point_near(
            curve.control_points()[13].point(),
            point(-0.07472151473754365, 5.352610108097121, 5.007404085332098),
            3.0e-8,
        );
    }

    #[test]
    fn swept_spiral_rejects_bad_density_and_axial_radius_seed() {
        let rail = LineSegment::try_new(
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 10.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            NurbsCurve::try_swept_spiral(
                CurveRef::Line(&rail),
                point(1.0, 0.0, 0.0),
                1.0,
                [1.0, 1.0],
                4,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidSweptSpiralPointsPerTurn { actual: 4 })
        );
        assert!(
            NurbsCurve::try_swept_spiral(
                CurveRef::Line(&rail),
                point(0.0, 0.0, 2.0),
                1.0,
                [1.0, 1.0],
                12,
                Tolerance::DEFAULT,
            )
            .is_err()
        );
    }
}
