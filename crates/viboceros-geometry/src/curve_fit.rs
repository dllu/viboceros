use faer::{Mat, prelude::*};
use std::collections::HashMap;

use crate::{
    CurveRef, GeometryError, NurbsCurve, Point3, Real, Tolerance,
    curve::{ArcLengthKink, ArcLengthSampler},
    nurbs::bspline_basis_values,
};

/// Largest degree accepted by Rhino's `FitCrv` command.
pub const MAX_CURVE_FIT_DEGREE: usize = 11;

/// Resource ceiling for adaptive curve fitting (cubic banded, otherwise dense).
pub const MAX_CURVE_FIT_CONTROL_POINTS: usize = 512;

const CURVE_FIT_ERROR_SAMPLES_PER_SPAN: usize = 16;
const MAX_CACHED_FIT_POINTS: usize = MAX_CURVE_FIT_CONTROL_POINTS * 32;

#[derive(Clone, Copy, Debug)]
struct FitBreak {
    distance: Real,
    multiplicity: usize,
}

/// Approximates a curve with a non-rational NURBS curve of the requested
/// degree, using arc length as the output parameter.
///
/// Natural source-span boundaries seed the adaptive approximation. Boundaries
/// whose one-sided tangents differ by more than `angle_tolerance_radians` are
/// retained as C0 knots; the output also preserves one-sided tangents there and
/// at the natural endpoints. Failing spans are bisected, prioritized by their
/// sampled errors, until the tolerance or resource ceiling is reached.
pub fn try_fit_curve(
    source: CurveRef<'_>,
    degree: usize,
    fit_tolerance: Real,
    angle_tolerance_radians: Real,
    numerical_tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    if degree == 0 || degree > MAX_CURVE_FIT_DEGREE {
        return Err(GeometryError::InvalidCurveFitDegree {
            actual: degree,
            maximum: MAX_CURVE_FIT_DEGREE,
        });
    }
    if !fit_tolerance.is_finite() || fit_tolerance <= 0.0 {
        return Err(GeometryError::InvalidCurveFitTolerance);
    }
    if !angle_tolerance_radians.is_finite()
        || !(0.0..=std::f64::consts::PI).contains(&angle_tolerance_radians)
    {
        return Err(GeometryError::InvalidCurveFitAngleTolerance);
    }

    let mut sampler = ArcLengthSampler::try_new(source, numerical_tolerance)?;
    sampler.prepare_repeated_sampling(32)?;
    let total_length = sampler.total_length();
    let kinks = sampler.kinks(angle_tolerance_radians)?;
    let mut breaks = sampler
        .natural_break_distances()
        .map(|distance| FitBreak {
            distance,
            multiplicity: if kinks.iter().any(|kink| kink.distance == distance) {
                degree
            } else {
                1
            },
        })
        .collect::<Vec<_>>();

    // A quadratic needs two knot spans between consecutive tangent-constrained
    // boundaries so the incoming and outgoing handle controls do not overlap.
    if degree == 2 {
        let mut hard_boundaries = Vec::with_capacity(kinks.len() + 2);
        hard_boundaries.push(0.0);
        hard_boundaries.extend(kinks.iter().map(|kink| kink.distance));
        hard_boundaries.push(total_length);
        for interval in hard_boundaries.windows(2) {
            if !breaks
                .iter()
                .any(|item| item.distance > interval[0] && item.distance < interval[1])
            {
                let midpoint = stable_midpoint(interval[0], interval[1]);
                if midpoint <= interval[0] || midpoint >= interval[1] {
                    return Err(GeometryError::Degenerate {
                        context: "quadratic curve fit span",
                    });
                }
                breaks.push(FitBreak {
                    distance: midpoint,
                    multiplicity: 1,
                });
            }
        }
        breaks.sort_by(|left, right| left.distance.total_cmp(&right.distance));
    }

    // Balanced refinement revisits many identical source distances. Cache
    // exact floating-point keys, without quantizing stations or changing the
    // integration tolerance. The cache is local to this fit and bounded.
    let mut points = HashMap::new();
    loop {
        let control_count = fit_control_count(degree, &breaks)?;
        if control_count > MAX_CURVE_FIT_CONTROL_POINTS {
            return Err(GeometryError::TooManyCurveFitControlPoints {
                maximum: MAX_CURVE_FIT_CONTROL_POINTS,
            });
        }
        let knots = fit_knots(degree, total_length, &breaks, control_count);
        let approximation = interpolate_fit(&sampler, degree, &knots, &kinks)?;
        let errors =
            sampled_span_errors(&sampler, &approximation, total_length, &breaks, &mut points)?;
        let maximum_deviation = errors
            .iter()
            .map(|error| error.deviation)
            .fold(0.0_f64, Real::max);
        if maximum_deviation <= fit_tolerance {
            return Ok(approximation);
        }

        let available = MAX_CURVE_FIT_CONTROL_POINTS - control_count;
        if available == 0 {
            return Err(GeometryError::CurveFitDidNotConverge {
                tolerance: fit_tolerance,
                deviation: maximum_deviation,
                maximum: MAX_CURVE_FIT_CONTROL_POINTS,
            });
        }
        let mut refinements = errors
            .into_iter()
            .filter(|error| error.deviation > fit_tolerance)
            .collect::<Vec<_>>();
        refinements.sort_by(|left, right| right.deviation.total_cmp(&left.deviation));
        refinements.truncate(available);
        refinements.sort_by(|left, right| left.refinement.total_cmp(&right.refinement));

        let previous_len = breaks.len();
        for refinement in refinements {
            insert_smooth_break(&mut breaks, refinement.refinement);
        }
        if breaks.len() == previous_len {
            return Err(GeometryError::CurveFitDidNotConverge {
                tolerance: fit_tolerance,
                deviation: maximum_deviation,
                maximum: MAX_CURVE_FIT_CONTROL_POINTS,
            });
        }
    }
}

fn fit_control_count(degree: usize, breaks: &[FitBreak]) -> Result<usize, GeometryError> {
    breaks
        .iter()
        .try_fold(degree + 1, |count, item| {
            count.checked_add(item.multiplicity)
        })
        .ok_or(GeometryError::TooManyCurveFitControlPoints {
            maximum: MAX_CURVE_FIT_CONTROL_POINTS,
        })
}

fn fit_knots(
    degree: usize,
    total_length: Real,
    breaks: &[FitBreak],
    control_count: usize,
) -> Vec<Real> {
    let mut knots = Vec::with_capacity(control_count + degree + 1);
    knots.extend(std::iter::repeat_n(0.0, degree + 1));
    for item in breaks {
        knots.extend(std::iter::repeat_n(item.distance, item.multiplicity));
    }
    knots.extend(std::iter::repeat_n(total_length, degree + 1));
    knots
}

fn interpolate_fit(
    sampler: &ArcLengthSampler<'_>,
    degree: usize,
    knots: &[Real],
    kinks: &[ArcLengthKink],
) -> Result<NurbsCurve, GeometryError> {
    let control_count = knots.len() - degree - 1;
    if degree == 1 {
        let controls = (0..control_count)
            .map(|index| sampler.point_at_distance(knots[index + 1]))
            .collect::<Result<Vec<_>, _>>()?;
        return NurbsCurve::try_new(degree, controls, knots.to_vec());
    }

    let mut controls = vec![None; control_count];
    constrain_endpoint_handles(sampler, degree, knots, &mut controls)?;
    for kink in kinks {
        constrain_kink_handles(sampler, degree, knots, kink, &mut controls)?;
    }

    if degree == 3 {
        solve_cubic_fit_controls(sampler, knots, &mut controls)?;
    } else {
        solve_dense_fit_controls(sampler, degree, knots, &mut controls)?;
    }

    NurbsCurve::try_new(
        degree,
        controls
            .into_iter()
            .map(|point| point.expect("all curve-fit controls are constrained or solved"))
            .collect(),
        knots.to_vec(),
    )
}

fn solve_cubic_fit_controls(
    sampler: &ArcLengthSampler<'_>,
    knots: &[Real],
    controls: &mut [Option<Point3>],
) -> Result<(), GeometryError> {
    let origin = controls[0].unwrap().to_array();
    let mut rows = Vec::with_capacity(controls.len());
    let mut targets: Vec<[Real; 3]> = Vec::with_capacity(controls.len());
    for (i, fixed) in controls.iter().enumerate() {
        let (row, point) = if let Some(point) = fixed {
            ([0.0, 0.0, 1.0, 0.0, 0.0], *point)
        } else {
            let (row, parameter, _, _) = crate::spline_collocation::collocation_row(knots, i)?;
            (row, sampler.point_at_distance(parameter)?)
        };
        rows.push(row);
        targets.push(std::array::from_fn(|k| point.to_array()[k] - origin[k]));
    }
    let rhs = Mat::from_fn(controls.len(), 3, |i, k| targets[i][k]);
    let solution = crate::spline_collocation::solve(&rows, rhs)?;
    for (i, control) in controls.iter_mut().enumerate() {
        if control.is_none() {
            *control = Some(Point3::try_from(std::array::from_fn(|k| {
                solution[(i, k)] + origin[k]
            }))?);
        }
    }
    Ok(())
}

fn solve_dense_fit_controls(
    sampler: &ArcLengthSampler<'_>,
    degree: usize,
    knots: &[Real],
    controls: &mut [Option<Point3>],
) -> Result<(), GeometryError> {
    let control_count = controls.len();

    let unknown = controls
        .iter()
        .enumerate()
        .filter_map(|(index, point)| point.is_none().then_some(index))
        .collect::<Vec<_>>();
    if !unknown.is_empty() {
        let mut rows = Vec::with_capacity(unknown.len());
        let mut targets = Vec::with_capacity(unknown.len());
        for &control_index in &unknown {
            let parameter = greville_parameter(knots, degree, control_index)?;
            let basis = bspline_basis_values(knots, degree, control_count, parameter)?;
            let mut target = sampler.point_at_distance(parameter)?.to_array();
            for (fixed_index, fixed) in controls.iter().enumerate() {
                if let Some(point) = fixed {
                    let coordinates = point.to_array();
                    for coordinate in 0..3 {
                        target[coordinate] = (-basis[fixed_index])
                            .mul_add(coordinates[coordinate], target[coordinate]);
                    }
                }
            }
            rows.push(
                unknown
                    .iter()
                    .map(|&index| basis[index])
                    .collect::<Vec<_>>(),
            );
            targets.push(target);
        }

        let count = unknown.len();
        let matrix = Mat::from_fn(count, count, |row, column| rows[row][column]);
        let right_hand_side = Mat::from_fn(count, 3, |row, column| targets[row][column]);
        let solution = matrix.full_piv_lu().solve(&right_hand_side);
        for (row, &control_index) in unknown.iter().enumerate() {
            controls[control_index] = Some(Point3::try_new(
                solution[(row, 0)],
                solution[(row, 1)],
                solution[(row, 2)],
            )?);
        }
    }

    Ok(())
}

fn constrain_endpoint_handles(
    sampler: &ArcLengthSampler<'_>,
    degree: usize,
    knots: &[Real],
    controls: &mut [Option<Point3>],
) -> Result<(), GeometryError> {
    let last = controls.len() - 1;
    let start = sampler.sample_at_distance(0.0)?;
    let end = sampler.sample_at_distance(sampler.total_length())?;
    let start_handle_length = (knots[degree + 1] - knots[degree]) / degree as Real;
    let end_handle_length = (knots[controls.len()] - knots[controls.len() - 1]) / degree as Real;
    controls[0] = Some(start.point());
    controls[1] = Some(
        start
            .point()
            .translated(start.tangent().as_vector().scaled(start_handle_length)?)?,
    );
    controls[last - 1] = Some(
        end.point()
            .translated(end.tangent().as_vector().scaled(-end_handle_length)?)?,
    );
    controls[last] = Some(end.point());
    Ok(())
}

fn constrain_kink_handles(
    sampler: &ArcLengthSampler<'_>,
    degree: usize,
    knots: &[Real],
    kink: &ArcLengthKink,
    controls: &mut [Option<Point3>],
) -> Result<(), GeometryError> {
    let run_start = knots.partition_point(|knot| *knot < kink.distance);
    let junction = run_start - 1;
    let point = sampler.point_at_distance(kink.distance)?;
    let incoming_length = (kink.distance - knots[junction]) / degree as Real;
    let outgoing_length = (knots[junction + degree + 1] - kink.distance) / degree as Real;
    controls[junction - 1] =
        Some(point.translated(kink.incoming_tangent.as_vector().scaled(-incoming_length)?)?);
    controls[junction] = Some(point);
    controls[junction + 1] =
        Some(point.translated(kink.outgoing_tangent.as_vector().scaled(outgoing_length)?)?);
    Ok(())
}

fn greville_parameter(
    knots: &[Real],
    degree: usize,
    control_index: usize,
) -> Result<Real, GeometryError> {
    let values = &knots[control_index + 1..=control_index + degree];
    let scale = values.iter().copied().fold(0.0_f64, Real::max);
    if scale == 0.0 {
        return Ok(0.0);
    }
    let normalized_sum = values.iter().map(|value| value / scale).sum::<Real>();
    let parameter = scale * (normalized_sum / degree as Real);
    if parameter.is_finite() {
        Ok(parameter)
    } else {
        Err(GeometryError::NonFinite {
            context: "curve-fit Greville parameter",
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct SpanError {
    deviation: Real,
    refinement: Real,
}

fn sampled_span_errors(
    sampler: &ArcLengthSampler<'_>,
    approximation: &NurbsCurve,
    total_length: Real,
    breaks: &[FitBreak],
    points: &mut HashMap<u64, Point3>,
) -> Result<Vec<SpanError>, GeometryError> {
    let mut boundaries = Vec::with_capacity(breaks.len() + 2);
    boundaries.push(0.0);
    boundaries.extend(breaks.iter().map(|item| item.distance));
    boundaries.push(total_length);

    boundaries
        .windows(2)
        .map(|interval| {
            let mut largest = SpanError {
                deviation: 0.0,
                refinement: stable_midpoint(interval[0], interval[1]),
            };
            for sample_index in 1..CURVE_FIT_ERROR_SAMPLES_PER_SPAN {
                let fraction = sample_index as Real / CURVE_FIT_ERROR_SAMPLES_PER_SPAN as Real;
                let parameter = interval[0].mul_add(1.0 - fraction, interval[1] * fraction);
                let exact = if let Some(&point) = points.get(&parameter.to_bits()) {
                    point
                } else {
                    let point = sampler.point_at_distance(parameter)?;
                    if points.len() < MAX_CACHED_FIT_POINTS {
                        points.insert(parameter.to_bits(), point);
                    }
                    point
                };
                let fitted = approximation.evaluate(parameter)?;
                let deviation = exact.distance_to(fitted)?;
                if deviation > largest.deviation {
                    largest.deviation = deviation;
                }
            }
            Ok(largest)
        })
        .collect()
}

fn insert_smooth_break(breaks: &mut Vec<FitBreak>, parameter: Real) {
    match breaks.binary_search_by(|item| item.distance.total_cmp(&parameter)) {
        Ok(_) => {}
        Err(index) => breaks.insert(
            index,
            FitBreak {
                distance: parameter,
                multiplicity: 1,
            },
        ),
    }
}

fn stable_midpoint(start: Real, end: Real) -> Real {
    0.5 * start + 0.5 * end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LineSegment, Polyline3, WeightedPoint3};

    #[test]
    fn cubic_banded_fit_matches_dense_elimination_with_fixed_kink_handles() {
        let spatial = NurbsCurve::try_new(
            3,
            vec![
                point(1., 2., 0.),
                point(2., 0., 4.),
                point(3., 4., -2.),
                point(5., 2., 3.),
            ],
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap();
        let kinked = Polyline3::try_new(
            vec![
                point(0., 0., 0.),
                point(1., 0., 0.),
                point(1., 2., 0.),
                point(2., 2., 1.),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        for source in [CurveRef::NurbsCurve(&spatial), CurveRef::Polyline(&kinked)] {
            let sampler = ArcLengthSampler::try_new(source, Tolerance::DEFAULT).unwrap();
            let kinks = sampler.kinks(1e-10).unwrap();
            let mut breaks = kinks
                .iter()
                .map(|k| FitBreak {
                    distance: k.distance,
                    multiplicity: 3,
                })
                .collect::<Vec<_>>();
            for f in [0.03, 0.07, 0.23, 0.31, 0.44, 0.67, 0.88, 0.99] {
                insert_smooth_break(&mut breaks, f * sampler.total_length());
            }
            let n = fit_control_count(3, &breaks).unwrap();
            let knots = fit_knots(3, sampler.total_length(), &breaks, n);
            let mut banded = vec![None; n];
            constrain_endpoint_handles(&sampler, 3, &knots, &mut banded).unwrap();
            for kink in &kinks {
                constrain_kink_handles(&sampler, 3, &knots, kink, &mut banded).unwrap();
            }
            let mut dense = banded.clone();
            solve_cubic_fit_controls(&sampler, &knots, &mut banded).unwrap();
            solve_dense_fit_controls(&sampler, 3, &knots, &mut dense).unwrap();
            for (a, b) in banded.into_iter().zip(dense) {
                assert!(a.unwrap().distance_to(b.unwrap()).unwrap() < 2e-13);
            }
        }
    }

    #[test]
    fn cubic_spatial_refit_reaches_a_tight_geometric_tolerance() {
        let rail = NurbsCurve::try_new(
            3,
            [[0., 0., 0.], [2., 0., 4.], [3., 4., -2.], [5., 2., 3.]]
                .map(|p| Point3::try_from(p).unwrap())
                .to_vec(),
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap();
        let fit = try_fit_curve(
            CurveRef::NurbsCurve(&rail),
            3,
            2.5e-8,
            1e-10,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&rail), Tolerance::DEFAULT).unwrap();
        for i in 0..=257 {
            let s = sampler.total_length() * i as Real / 257.;
            assert!(
                fit.evaluate(s)
                    .unwrap()
                    .distance_to(sampler.point_at_distance(s).unwrap())
                    .unwrap()
                    < 3e-8
            );
        }
    }

    #[test]
    fn cubic_quarter_arc_refit_reaches_a_tight_geometric_tolerance() {
        let circle = crate::Circle3::try_new(
            Point3::try_new(0., 0., 0.).unwrap(),
            3.,
            crate::UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let arc = crate::CircularArc3::try_from_circle_sweep(circle, std::f64::consts::FRAC_PI_2)
            .unwrap();
        let fit = try_fit_curve(CurveRef::Arc(&arc), 3, 2.5e-8, 1e-10, Tolerance::DEFAULT).unwrap();
        for i in 0..=257 {
            let f = i as Real / 257.;
            let expected = arc.evaluate(*arc.domain().end() * f).unwrap();
            let actual = fit.evaluate(*fit.domain().end() * f).unwrap();
            assert!(expected.distance_to(actual).unwrap() < 3e-8);
        }
    }
    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn assert_near(actual: Real, expected: Real, epsilon: Real) {
        assert!(
            (actual - expected).abs() <= epsilon * actual.abs().max(expected.abs()).max(1.0),
            "expected {expected:.16e}, got {actual:.16e}"
        );
    }

    #[test]
    fn line_fit_matches_rhino_arc_length_bezier() {
        let line = LineSegment::try_new(
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let fit = try_fit_curve(
            CurveRef::Line(&line),
            3,
            1.0e-3,
            Tolerance::DEFAULT.angular(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(fit.degree(), 3);
        assert_eq!(fit.knots(), &[0.0, 0.0, 0.0, 0.0, 10.0, 10.0, 10.0, 10.0]);
        for (control, expected_x) in
            fit.control_points()
                .iter()
                .zip([0.0, 10.0 / 3.0, 20.0 / 3.0, 10.0])
        {
            assert_near(control.point().x(), expected_x, 2.0e-15);
            assert_eq!(control.weight(), 1.0);
        }
    }

    #[test]
    fn cubic_fit_preserves_polyline_kinks_exactly() {
        let square = Polyline3::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(5.0, 0.0, 0.0),
                point(5.0, 5.0, 0.0),
                point(0.0, 5.0, 0.0),
                point(0.0, 0.0, 0.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let fit = try_fit_curve(
            CurveRef::Polyline(&square),
            3,
            1.0e-2,
            0.1,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(fit.control_points().len(), 13);
        assert_eq!(
            fit.knots(),
            &[
                0.0, 0.0, 0.0, 0.0, 5.0, 5.0, 5.0, 10.0, 10.0, 10.0, 15.0, 15.0, 15.0, 20.0, 20.0,
                20.0, 20.0,
            ]
        );
        let sampler =
            ArcLengthSampler::try_new(CurveRef::Polyline(&square), Tolerance::DEFAULT).unwrap();
        for distance in 0..=20 {
            let expected = sampler.point_at_distance(distance as Real).unwrap();
            assert!(
                fit.evaluate(distance as Real)
                    .unwrap()
                    .distance_to(expected)
                    .unwrap()
                    < 2.0e-14
            );
        }
    }

    #[test]
    fn rational_circle_fit_is_closed_non_rational_and_within_tolerance() {
        let weight = std::f64::consts::FRAC_1_SQRT_2;
        let controls = [
            (5.0, 0.0, 1.0),
            (5.0, 5.0, weight),
            (0.0, 5.0, 1.0),
            (-5.0, 5.0, weight),
            (-5.0, 0.0, 1.0),
            (-5.0, -5.0, weight),
            (0.0, -5.0, 1.0),
            (5.0, -5.0, weight),
            (5.0, 0.0, 1.0),
        ]
        .into_iter()
        .map(|(x, y, weight)| WeightedPoint3::try_new(point(x, y, 0.0), weight).unwrap())
        .collect();
        let circle = NurbsCurve::try_new_rational(
            2,
            controls,
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 4.0],
        )
        .unwrap();
        let fit = try_fit_curve(
            CurveRef::NurbsCurve(&circle),
            3,
            1.0e-3,
            0.1,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(!fit.is_rational());
        assert!(fit.is_closed().unwrap());
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&circle), Tolerance::DEFAULT).unwrap();
        for sample in 0..=1024 {
            let distance = sampler.total_length() * sample as Real / 1024.0;
            assert!(
                fit.evaluate(distance)
                    .unwrap()
                    .distance_to(sampler.point_at_distance(distance).unwrap())
                    .unwrap()
                    <= 1.05e-3
            );
        }
    }

    #[test]
    fn adaptive_fit_resolves_a_multispan_spatial_curve() {
        let source = NurbsCurve::try_new(
            3,
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 3.0, 0.0),
                point(2.0, -2.0, 1.0),
                point(4.0, 4.0, -1.0),
                point(6.0, -3.0, 2.0),
                point(8.0, 2.0, 0.0),
                point(10.0, 0.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0, 4.0, 4.0],
        )
        .unwrap();
        let fit = try_fit_curve(
            CurveRef::NurbsCurve(&source),
            3,
            1.0e-3,
            0.1,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&source), Tolerance::DEFAULT).unwrap();
        for sample in 0..=1024 {
            let distance = sampler.total_length() * sample as Real / 1024.0;
            assert!(
                fit.evaluate(distance)
                    .unwrap()
                    .distance_to(sampler.point_at_distance(distance).unwrap())
                    .unwrap()
                    <= 1.05e-3
            );
        }
    }

    #[test]
    fn validates_fit_options() {
        let line = LineSegment::try_new(
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(matches!(
            try_fit_curve(CurveRef::Line(&line), 0, 0.01, 0.1, Tolerance::DEFAULT),
            Err(GeometryError::InvalidCurveFitDegree { .. })
        ));
        assert_eq!(
            try_fit_curve(CurveRef::Line(&line), 3, 0.0, 0.1, Tolerance::DEFAULT),
            Err(GeometryError::InvalidCurveFitTolerance)
        );
        assert_eq!(
            try_fit_curve(
                CurveRef::Line(&line),
                3,
                0.01,
                Real::NAN,
                Tolerance::DEFAULT
            ),
            Err(GeometryError::InvalidCurveFitAngleTolerance)
        );
    }
}
