use faer::{Mat, prelude::*};

use crate::{
    GeometryError, NurbsCurve, Point3, Real, Tolerance, UnitVector3, Vector3,
    nurbs::bspline_basis_values, require_finite,
};

/// Maximum point count accepted by one interpolation solve.
pub const MAX_CURVE_INTERPOLATION_POINTS: usize = 256;
const CUBIC_DEGREE: usize = 3;

/// Parameter interval policy between consecutive interpolation points.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveKnotSpacing {
    /// Assigns every interval a length of one.
    Uniform,
    /// Uses the Euclidean chord length.
    Chord,
    /// Uses the square root of the Euclidean chord length.
    SquareRootChord,
}

/// Topology requested at the interpolation seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterpolatedCurveClosure {
    /// Leaves the natural endpoints independent.
    Open,
    /// Builds a smooth periodic seam.
    Smooth,
    /// Repeats the first point as a non-periodic, kinked seam.
    Sharp,
}

/// Degree, parameterization, closure, and optional endpoint directions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveInterpolationOptions {
    degree: usize,
    knot_spacing: CurveKnotSpacing,
    closure: InterpolatedCurveClosure,
    start_tangent: Option<Vector3>,
    end_tangent: Option<Vector3>,
}

impl CurveInterpolationOptions {
    pub const fn new(
        degree: usize,
        knot_spacing: CurveKnotSpacing,
        closure: InterpolatedCurveClosure,
    ) -> Self {
        Self {
            degree,
            knot_spacing,
            closure,
            start_tangent: None,
            end_tangent: None,
        }
    }

    pub const fn with_start_tangent(mut self, tangent: Vector3) -> Self {
        self.start_tangent = Some(tangent);
        self
    }

    pub const fn with_end_tangent(mut self, tangent: Vector3) -> Self {
        self.end_tangent = Some(tangent);
        self
    }

    #[inline]
    pub const fn degree(self) -> usize {
        self.degree
    }

    #[inline]
    pub const fn knot_spacing(self) -> CurveKnotSpacing {
        self.knot_spacing
    }

    #[inline]
    pub const fn closure(self) -> InterpolatedCurveClosure {
        self.closure
    }

    #[inline]
    pub const fn start_tangent(self) -> Option<Vector3> {
        self.start_tangent
    }

    #[inline]
    pub const fn end_tangent(self) -> Option<Vector3> {
        self.end_tangent
    }
}

impl Default for CurveInterpolationOptions {
    fn default() -> Self {
        Self::new(
            CUBIC_DEGREE,
            CurveKnotSpacing::Chord,
            InterpolatedCurveClosure::Open,
        )
    }
}

impl NurbsCurve {
    /// Interpolates points with Rhino-style degree-one or cubic construction.
    ///
    /// Cubics use the picked points as simple knot parameters and solve for
    /// two additional endpoint handles. Automatic handles follow the
    /// chord-parameterized parabola through the first or last three points.
    pub fn try_interpolate(
        points: &[Point3],
        options: CurveInterpolationOptions,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if !matches!(options.degree, 1 | CUBIC_DEGREE) {
            return Err(GeometryError::UnsupportedCurveInterpolationDegree {
                actual: options.degree,
            });
        }
        if points.len() < 2 {
            return Err(GeometryError::InsufficientCurveInterpolationPoints {
                actual: points.len(),
            });
        }
        if points.len() > MAX_CURVE_INTERPOLATION_POINTS {
            return Err(GeometryError::TooManyCurveInterpolationPoints {
                maximum: MAX_CURVE_INTERPOLATION_POINTS,
            });
        }
        if (options.start_tangent.is_some() || options.end_tangent.is_some())
            && (options.degree != CUBIC_DEGREE || options.closure != InterpolatedCurveClosure::Open)
        {
            return Err(GeometryError::CurveInterpolationTangentsRequireOpenCubic);
        }

        let mut points = points.to_vec();
        match options.closure {
            InterpolatedCurveClosure::Open => {}
            InterpolatedCurveClosure::Smooth => {
                if points[0].is_near(points[points.len() - 1], tolerance) {
                    points.pop();
                }
            }
            InterpolatedCurveClosure::Sharp => {
                let first = points[0];
                let last = points.len() - 1;
                if first.is_near(points[last], tolerance) {
                    points[last] = first;
                } else {
                    points.push(first);
                }
            }
        }
        if points.len() < 2 {
            return Err(GeometryError::InsufficientCurveInterpolationPoints {
                actual: points.len(),
            });
        }
        if points.len() > MAX_CURVE_INTERPOLATION_POINTS {
            return Err(GeometryError::TooManyCurveInterpolationPoints {
                maximum: MAX_CURVE_INTERPOLATION_POINTS,
            });
        }
        validate_adjacent_points(&points, tolerance)?;
        if options.closure == InterpolatedCurveClosure::Smooth
            && points[points.len() - 1].distance_to(points[0])? <= tolerance.absolute()
        {
            return Err(GeometryError::CoincidentCurveInterpolationPoints { second_index: 0 });
        }

        if options.degree == 1 {
            if options.closure == InterpolatedCurveClosure::Smooth {
                points.push(points[0]);
            }
            return interpolate_degree_one(&points);
        }
        if options.closure == InterpolatedCurveClosure::Smooth {
            return interpolate_periodic_cubic(&points, options.knot_spacing);
        }
        if points.len() == 2 && options.start_tangent.is_none() && options.end_tangent.is_none() {
            return interpolate_two_point_line(points[0], points[1]);
        }
        interpolate_open_cubic(
            &points,
            options.knot_spacing,
            options.start_tangent,
            options.end_tangent,
        )
    }
}

fn validate_adjacent_points(points: &[Point3], tolerance: Tolerance) -> Result<(), GeometryError> {
    for (index, pair) in points.windows(2).enumerate() {
        if pair[0].distance_to(pair[1])? <= tolerance.absolute() {
            return Err(GeometryError::CoincidentCurveInterpolationPoints {
                second_index: index + 1,
            });
        }
    }
    Ok(())
}

fn interpolate_degree_one(points: &[Point3]) -> Result<NurbsCurve, GeometryError> {
    if points.len() == 2 {
        return interpolate_two_point_line(points[0], points[1]);
    }
    let last = (points.len() - 1) as Real;
    let mut knots = Vec::with_capacity(points.len() + 2);
    knots.extend([0.0, 0.0]);
    knots.extend((1..points.len() - 1).map(|index| index as Real));
    knots.extend([last, last]);
    NurbsCurve::try_new(1, points.to_vec(), knots)
}

fn interpolate_two_point_line(start: Point3, end: Point3) -> Result<NurbsCurve, GeometryError> {
    let domain_end = start.distance_to(end)?;
    NurbsCurve::try_new(1, vec![start, end], vec![0.0, 0.0, domain_end, domain_end])
}

fn interpolate_open_cubic(
    points: &[Point3],
    spacing: CurveKnotSpacing,
    start_tangent: Option<Vector3>,
    end_tangent: Option<Vector3>,
) -> Result<NurbsCurve, GeometryError> {
    let intervals = interpolation_intervals(points, spacing, false)?;
    let parameters = cumulative_parameters(&intervals)?;
    let control_count = points.len() + 2;
    let mut knots = Vec::with_capacity(control_count + CUBIC_DEGREE + 1);
    knots.extend([parameters[0]; CUBIC_DEGREE + 1]);
    knots.extend_from_slice(&parameters[1..parameters.len() - 1]);
    knots.extend([parameters[parameters.len() - 1]; CUBIC_DEGREE + 1]);

    let mut rows = Vec::with_capacity(control_count);
    let mut targets = Vec::with_capacity(control_count);
    for (point, parameter) in points.iter().zip(&parameters) {
        rows.push(bspline_basis_values(
            &knots,
            CUBIC_DEGREE,
            control_count,
            *parameter,
        )?);
        targets.push(*point);
    }

    let start_direction = match start_tangent {
        Some(tangent) => tangent.normalized_nonzero()?,
        None => automatic_start_tangent(points)?,
    };
    let end_direction = match end_tangent {
        Some(tangent) => tangent.normalized_nonzero()?,
        None => automatic_end_tangent(points)?,
    };
    let start_handle = points[0].translated(
        start_direction
            .as_vector()
            .scaled(points[0].distance_to(points[1])? / 3.0)?,
    )?;
    let end_handle = points[points.len() - 1].translated(
        end_direction
            .as_vector()
            .scaled(-points[points.len() - 2].distance_to(points[points.len() - 1])? / 3.0)?,
    )?;
    let mut start_row = vec![0.0; control_count];
    start_row[1] = 1.0;
    rows.push(start_row);
    targets.push(start_handle);
    let mut end_row = vec![0.0; control_count];
    end_row[control_count - 2] = 1.0;
    rows.push(end_row);
    targets.push(end_handle);

    let controls = solve_control_points(&rows, &targets)?;
    NurbsCurve::try_new(CUBIC_DEGREE, controls, knots)
}

fn interpolate_periodic_cubic(
    points: &[Point3],
    spacing: CurveKnotSpacing,
) -> Result<NurbsCurve, GeometryError> {
    let intervals = interpolation_intervals(points, spacing, true)?;
    let parameters = cumulative_parameters(&intervals)?;
    let unique_control_count = points.len();
    let control_count = unique_control_count + CUBIC_DEGREE;

    let mut knots = Vec::with_capacity(control_count + CUBIC_DEGREE + 1);
    let mut before = Vec::with_capacity(CUBIC_DEGREE);
    let mut parameter = 0.0;
    for offset in 0..CUBIC_DEGREE {
        parameter -= intervals[intervals.len() - 1 - offset % intervals.len()];
        require_finite([parameter], "periodic interpolation knot")?;
        before.push(parameter);
    }
    knots.extend(before.into_iter().rev());
    knots.extend_from_slice(&parameters);
    parameter = parameters[parameters.len() - 1];
    for offset in 0..CUBIC_DEGREE {
        parameter += intervals[offset % intervals.len()];
        require_finite([parameter], "periodic interpolation knot")?;
        knots.push(parameter);
    }

    let mut rows = Vec::with_capacity(unique_control_count);
    for parameter in &parameters[..unique_control_count] {
        let basis = bspline_basis_values(&knots, CUBIC_DEGREE, control_count, *parameter)?;
        let mut folded = vec![0.0; unique_control_count];
        for (index, value) in basis.into_iter().enumerate() {
            folded[index % unique_control_count] += value;
        }
        rows.push(folded);
    }
    let unique_controls = solve_control_points(&rows, points)?;
    let controls = (0..control_count)
        .map(|index| unique_controls[index % unique_control_count])
        .collect();
    NurbsCurve::try_new(CUBIC_DEGREE, controls, knots)
}

fn interpolation_intervals(
    points: &[Point3],
    spacing: CurveKnotSpacing,
    closed: bool,
) -> Result<Vec<Real>, GeometryError> {
    let interval_count = points.len() - usize::from(!closed);
    let mut intervals = Vec::with_capacity(interval_count);
    for index in 0..interval_count {
        let distance = points[index].distance_to(points[(index + 1) % points.len()])?;
        let interval = match spacing {
            CurveKnotSpacing::Uniform => 1.0,
            CurveKnotSpacing::Chord => distance,
            CurveKnotSpacing::SquareRootChord => distance.sqrt(),
        };
        require_finite([interval], "curve interpolation interval")?;
        if interval <= 0.0 {
            return Err(GeometryError::CoincidentCurveInterpolationPoints {
                second_index: (index + 1) % points.len(),
            });
        }
        intervals.push(interval);
    }
    Ok(intervals)
}

fn cumulative_parameters(intervals: &[Real]) -> Result<Vec<Real>, GeometryError> {
    let mut parameters = Vec::with_capacity(intervals.len() + 1);
    parameters.push(0.0);
    for interval in intervals {
        let next = parameters[parameters.len() - 1] + interval;
        require_finite([next], "curve interpolation parameter")?;
        if next <= parameters[parameters.len() - 1] {
            return Err(GeometryError::InvalidKnotVector {
                context: "curve interpolation parameters must strictly increase",
            });
        }
        parameters.push(next);
    }
    Ok(parameters)
}

fn automatic_start_tangent(points: &[Point3]) -> Result<UnitVector3, GeometryError> {
    if points.len() == 2 {
        return points[0].vector_to(points[1])?.normalized_nonzero();
    }
    parabolic_start_tangent(points[0], points[1], points[2])
}

fn automatic_end_tangent(points: &[Point3]) -> Result<UnitVector3, GeometryError> {
    if points.len() == 2 {
        return points[0].vector_to(points[1])?.normalized_nonzero();
    }
    Ok(parabolic_start_tangent(
        points[points.len() - 1],
        points[points.len() - 2],
        points[points.len() - 3],
    )?
    .opposite())
}

fn parabolic_start_tangent(
    first: Point3,
    second: Point3,
    third: Point3,
) -> Result<UnitVector3, GeometryError> {
    let first_chord = first.vector_to(second)?;
    let second_chord = second.vector_to(third)?;
    let first_length = first_chord.length()?;
    let second_length = second_chord.length()?;
    let scale = first_length.max(second_length);
    let first_length = first_length / scale;
    let second_length = second_length / scale;
    let first_direction = first_chord.normalized_nonzero()?.as_vector().to_array();
    let second_direction = second_chord.normalized_nonzero()?.as_vector().to_array();
    let tangent = std::array::from_fn(|index| {
        first_direction[index].mul_add(
            2.0 * first_length + second_length,
            -second_direction[index] * first_length,
        )
    });
    Vector3::try_from(tangent)?.normalized_nonzero()
}

fn solve_control_points(
    rows: &[Vec<Real>],
    targets: &[Point3],
) -> Result<Vec<Point3>, GeometryError> {
    debug_assert_eq!(rows.len(), targets.len());
    let count = rows.len();
    debug_assert!(rows.iter().all(|row| row.len() == count));
    let matrix = Mat::from_fn(count, count, |row, column| rows[row][column]);
    let right_hand_side = Mat::from_fn(count, 3, |row, column| targets[row].to_array()[column]);
    let solution = matrix.full_piv_lu().solve(&right_hand_side);
    (0..count)
        .map(|row| Point3::try_new(solution[(row, 0)], solution[(row, 1)], solution[(row, 2)]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: Real = 4.0e-12;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn fixture_points() -> Vec<Point3> {
        vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 2.0, 0.5),
            point(4.0, -1.0, 2.0),
            point(4.5, 3.0, -0.5),
            point(10.0, 0.0, 1.0),
        ]
    }

    fn assert_real_near(actual: Real, expected: Real) {
        assert!(
            (actual - expected).abs() <= EPSILON * actual.abs().max(expected.abs()).max(1.0),
            "expected {expected:.16e}, got {actual:.16e}"
        );
    }

    fn assert_point_near(actual: Point3, expected: [Real; 3]) {
        for (actual, expected) in actual.to_array().into_iter().zip(expected) {
            assert_real_near(actual, expected);
        }
    }

    fn assert_controls_near(curve: &NurbsCurve, expected: &[[Real; 3]]) {
        assert_eq!(curve.control_points().len(), expected.len());
        for (actual, expected) in curve.control_points().iter().zip(expected) {
            assert_point_near(actual.point(), *expected);
            assert_eq!(actual.weight(), 1.0);
        }
    }

    fn assert_short_knots_near(curve: &NurbsCurve, expected: &[Real]) {
        let full = curve.knots();
        let short = &full[1..full.len() - 1];
        assert_eq!(short.len(), expected.len());
        for (actual, expected) in short.iter().zip(expected) {
            assert_real_near(*actual, *expected);
        }
    }

    #[test]
    fn open_chord_cubic_matches_rhino_8() {
        let curve = NurbsCurve::try_interpolate(
            &fixture_points(),
            CurveInterpolationOptions::new(
                3,
                CurveKnotSpacing::Chord,
                InterpolatedCurveClosure::Open,
            ),
            Tolerance::DEFAULT,
        )
        .unwrap();

        assert_controls_near(
            &curve,
            &[
                [0.0, 0.0, 0.0],
                [
                    0.189_111_489_512_884_84,
                    0.733_913_747_657_792_5,
                    0.094_555_744_756_442_42,
                ],
                [
                    1.159_131_068_467_325_3,
                    3.968_196_577_656_024_6,
                    0.462_997_233_388_727_4,
                ],
                [
                    5.139_509_379_019_493,
                    -4.210_962_918_202_457,
                    3.428_846_662_142_386,
                ],
                [
                    2.994_904_302_332_344,
                    6.420_848_302_549_852,
                    -2.320_271_291_652_048_3,
                ],
                [
                    8.543_433_866_731_853,
                    1.382_642_946_678_897_9,
                    0.239_852_190_342_902_28,
                ],
                [10.0, 0.0, 1.0],
            ],
        );
        assert_short_knots_near(
            &curve,
            &[
                0.0,
                0.0,
                0.0,
                2.291_287_847_477_92,
                6.791_287_847_477_919_5,
                11.534_704_337_730_489,
                17.976_753_701_093_052,
                17.976_753_701_093_052,
                17.976_753_701_093_052,
            ],
        );
    }

    #[test]
    fn endpoint_tangent_directions_match_rhino_8_and_ignore_magnitude() {
        let curve = NurbsCurve::try_interpolate(
            &fixture_points(),
            CurveInterpolationOptions::new(
                3,
                CurveKnotSpacing::Uniform,
                InterpolatedCurveClosure::Open,
            )
            .with_start_tangent(Vector3::try_new(2.0, -1.0, 3.0).unwrap())
            .with_end_tangent(Vector3::try_new(-1.0, 4.0, 2.0).unwrap()),
            Tolerance::DEFAULT,
        )
        .unwrap();

        assert_controls_near(
            &curve,
            &[
                [0.0, 0.0, 0.0],
                [
                    0.408_248_290_463_863,
                    -0.204_124_145_231_931_5,
                    0.612_372_435_695_794_6,
                ],
                [
                    -0.063_422_068_439_996_65,
                    4.804_570_516_041_926,
                    -0.429_416_477_980_926_2,
                ],
                [
                    5.609_604_803_844_195,
                    -4.509_810_588_298_841,
                    3.584_399_019_389_55,
                ],
                [
                    1.625_002_853_063_216_8,
                    7.234_671_837_153_438,
                    -1.908_179_599_577_272_3,
                ],
                [
                    10.468_590_140_289_699,
                    -1.874_360_561_158_795,
                    0.062_819_719_420_602_5,
                ],
                [10.0, 0.0, 1.0],
            ],
        );
        assert_short_knots_near(&curve, &[0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0, 4.0]);
    }

    #[test]
    fn smooth_periodic_chord_cubic_matches_rhino_8() {
        let mut closed_points = fixture_points();
        closed_points.push(closed_points[0]);
        let curve = NurbsCurve::try_interpolate(
            &closed_points,
            CurveInterpolationOptions::new(
                3,
                CurveKnotSpacing::Chord,
                InterpolatedCurveClosure::Smooth,
            ),
            Tolerance::DEFAULT,
        )
        .unwrap();

        assert!(curve.is_periodic());
        assert!(curve.is_closed().unwrap());
        assert_controls_near(
            &curve,
            &[
                [
                    16.521_448_922_648_386,
                    -2.265_528_214_077_550_6,
                    3.077_435_953_681_295,
                ],
                [
                    -2.103_806_590_269_69,
                    -3.743_302_563_830_965,
                    -0.644_416_512_964_814_1,
                ],
                [
                    1.357_397_809_956_871,
                    3.890_857_478_466_86,
                    0.508_214_980_456_405_6,
                ],
                [
                    5.168_461_482_183_58,
                    -4.154_784_447_044_371_6,
                    3.430_661_898_692_608,
                ],
                [
                    2.560_738_233_958_049_6,
                    6.276_534_035_563_515_5,
                    -2.397_031_680_656_004_6,
                ],
                [
                    16.521_448_922_648_386,
                    -2.265_528_214_077_550_6,
                    3.077_435_953_681_295,
                ],
                [
                    -2.103_806_590_269_69,
                    -3.743_302_563_830_965,
                    -0.644_416_512_964_814_1,
                ],
                [
                    1.357_397_809_956_871,
                    3.890_857_478_466_86,
                    0.508_214_980_456_405_6,
                ],
            ],
        );
        assert_short_knots_near(
            &curve,
            &[
                -16.491_924_984_483_454,
                -10.049_875_621_120_89,
                0.0,
                2.291_287_847_477_92,
                6.791_287_847_477_919_5,
                11.534_704_337_730_489,
                17.976_753_701_093_052,
                28.026_629_322_213_942,
                30.317_917_169_691_864,
                34.817_917_169_691_87,
            ],
        );
    }

    #[test]
    fn sharp_and_degree_one_closures_repeat_the_seam_without_becoming_periodic() {
        let points = fixture_points();
        let sharp = NurbsCurve::try_interpolate(
            &points,
            CurveInterpolationOptions::new(
                3,
                CurveKnotSpacing::Chord,
                InterpolatedCurveClosure::Sharp,
            ),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(sharp.is_closed().unwrap());
        assert!(!sharp.is_periodic());
        assert_eq!(sharp.control_points().len(), points.len() + 3);

        let linear = NurbsCurve::try_interpolate(
            &points[..3],
            CurveInterpolationOptions::new(
                1,
                CurveKnotSpacing::Chord,
                InterpolatedCurveClosure::Smooth,
            ),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(linear.degree(), 1);
        assert!(linear.is_closed().unwrap());
        assert!(!linear.is_periodic());
        assert_eq!(linear.control_points().len(), 4);
        assert_eq!(
            linear.control_points()[0].point(),
            linear.control_points()[3].point()
        );
    }

    #[test]
    fn two_points_become_a_line_unless_tangents_request_a_cubic() {
        let points = [point(0.0, 0.0, 0.0), point(10.0, 0.0, 0.0)];
        let line = NurbsCurve::try_interpolate(
            &points,
            CurveInterpolationOptions::default(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(line.degree(), 1);
        assert_eq!(line.domain(), 0.0..=10.0);

        let cubic = NurbsCurve::try_interpolate(
            &points,
            CurveInterpolationOptions::default()
                .with_start_tangent(Vector3::try_new(0.0, 2.0, 0.0).unwrap())
                .with_end_tangent(Vector3::try_new(0.0, -4.0, 0.0).unwrap()),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(cubic.degree(), 3);
        assert_eq!(cubic.domain(), 0.0..=10.0);
        assert_controls_near(
            &cubic,
            &[
                [0.0, 0.0, 0.0],
                [0.0, 10.0 / 3.0, 0.0],
                [10.0, 10.0 / 3.0, 0.0],
                [10.0, 0.0, 0.0],
            ],
        );
    }

    #[test]
    fn interpolation_rejects_invalid_structure() {
        let one = [point(0.0, 0.0, 0.0)];
        assert_eq!(
            NurbsCurve::try_interpolate(
                &one,
                CurveInterpolationOptions::default(),
                Tolerance::DEFAULT
            ),
            Err(GeometryError::InsufficientCurveInterpolationPoints { actual: 1 })
        );
        assert_eq!(
            NurbsCurve::try_interpolate(
                &[one[0], one[0]],
                CurveInterpolationOptions::default(),
                Tolerance::DEFAULT
            ),
            Err(GeometryError::CoincidentCurveInterpolationPoints { second_index: 1 })
        );
        assert_eq!(
            NurbsCurve::try_interpolate(
                &[one[0], point(1.0, 0.0, 0.0)],
                CurveInterpolationOptions::new(
                    2,
                    CurveKnotSpacing::Uniform,
                    InterpolatedCurveClosure::Open,
                ),
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::UnsupportedCurveInterpolationDegree { actual: 2 })
        );
        assert_eq!(
            NurbsCurve::try_interpolate(
                &fixture_points(),
                CurveInterpolationOptions::new(
                    3,
                    CurveKnotSpacing::Uniform,
                    InterpolatedCurveClosure::Smooth,
                )
                .with_start_tangent(Vector3::try_new(1.0, 0.0, 0.0).unwrap()),
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::CurveInterpolationTangentsRequireOpenCubic)
        );
    }
}
