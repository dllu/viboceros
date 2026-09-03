use crate::{
    CurveInterpolationOptions, CurveKnotSpacing, CurveRef, GeometryError, InterpolatedCurveClosure,
    NurbsCurve, Point3, Real, Tolerance, WeightedPoint3, require_finite,
};

/// Resource ceiling for the number of curves created by one tween operation.
pub const MAX_CURVE_TWEEN_COUNT: usize = 1_000_000;

/// Allocation guard across every output curve's control points.
pub const MAX_CURVE_TWEEN_OUTPUT_CONTROL_POINTS: usize = 1_000_000;

/// Smallest sampling division count accepted by Rhino's `TweenCurves` command.
pub const MIN_CURVE_TWEEN_SAMPLE_NUMBER: usize = 2;

/// Largest sampling division count accepted by Rhino's `TweenCurves` command.
pub const MAX_CURVE_TWEEN_SAMPLE_NUMBER: usize = 9_999;

/// How the two source curves are made structurally suitable for tweening.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveTweenMatchMethod {
    /// Connect corresponding Euclidean control locations and weights. Extra
    /// locations connect to the final location of the shorter curve.
    ControlPoint,
    /// Refit both sources to a shared non-rational structure. This method is
    /// recognized but currently reports an explicit unsupported error.
    Refit,
    /// Divide each source into this many equal-length segments, interpolate
    /// each sampled source, then tween the resulting control structures.
    SamplePoints { sample_number: usize },
}

/// Creates Rhino-style intermediate NURBS curves at evenly spaced fractions.
///
/// The source curves themselves are excluded, so `number == 2` creates curves
/// at one-third and two-thirds of the transition. Curve direction is never
/// matched automatically; callers can pass [`NurbsCurve::reversed`] sources.
pub fn try_tween_nurbs_curves(
    start: &NurbsCurve,
    end: &NurbsCurve,
    number: usize,
    method: CurveTweenMatchMethod,
    tolerance: Tolerance,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    validate_tween_count(number)?;
    match method {
        CurveTweenMatchMethod::ControlPoint => tween_control_points(start, end, number),
        CurveTweenMatchMethod::Refit => Err(GeometryError::UnsupportedCurveTweenRefit),
        CurveTweenMatchMethod::SamplePoints { sample_number } => {
            validate_sample_number(sample_number)?;
            let maximum_points = MAX_CURVE_TWEEN_SAMPLE_NUMBER + 1;
            let sampled_start = interpolate_sampled_source(
                CurveRef::NurbsCurve(start),
                sample_number,
                maximum_points,
                tolerance,
            )?;
            let sampled_end = interpolate_sampled_source(
                CurveRef::NurbsCurve(end),
                sample_number,
                maximum_points,
                tolerance,
            )?;
            tween_control_points(&sampled_start, &sampled_end, number)
        }
    }
}

fn validate_tween_count(number: usize) -> Result<(), GeometryError> {
    if !(1..=MAX_CURVE_TWEEN_COUNT).contains(&number) {
        Err(GeometryError::InvalidCurveTweenCount {
            actual: number,
            maximum: MAX_CURVE_TWEEN_COUNT,
        })
    } else {
        Ok(())
    }
}

fn validate_sample_number(sample_number: usize) -> Result<(), GeometryError> {
    if !(MIN_CURVE_TWEEN_SAMPLE_NUMBER..=MAX_CURVE_TWEEN_SAMPLE_NUMBER).contains(&sample_number) {
        Err(GeometryError::InvalidCurveTweenSampleCount {
            actual: sample_number,
            minimum: MIN_CURVE_TWEEN_SAMPLE_NUMBER,
            maximum: MAX_CURVE_TWEEN_SAMPLE_NUMBER,
        })
    } else {
        Ok(())
    }
}

fn interpolate_sampled_source(
    source: CurveRef<'_>,
    sample_number: usize,
    maximum_points: usize,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    let points = source.divide_by_count_for_tween(sample_number, true, tolerance)?;
    let linear = NurbsCurve::try_interpolate_for_tween_sampling(
        &points,
        CurveInterpolationOptions::new(1, CurveKnotSpacing::Chord, InterpolatedCurveClosure::Open),
        maximum_points,
    )?;
    if linear.is_linear(tolerance)? {
        return Ok(linear);
    }
    NurbsCurve::try_interpolate_for_tween_sampling(
        &points,
        CurveInterpolationOptions::new(3, CurveKnotSpacing::Chord, InterpolatedCurveClosure::Open),
        maximum_points,
    )
}

fn tween_control_points(
    start: &NurbsCurve,
    end: &NurbsCurve,
    number: usize,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    let start_controls = start.control_points();
    let end_controls = end.control_points();
    let template = if end_controls.len() > start_controls.len() {
        end
    } else {
        start
    };
    let control_count = template.control_points().len();
    if control_count
        .checked_mul(number)
        .is_none_or(|total| total > MAX_CURVE_TWEEN_OUTPUT_CONTROL_POINTS)
    {
        return Err(GeometryError::TooManyCurveTweenControlPoints {
            maximum: MAX_CURVE_TWEEN_OUTPUT_CONTROL_POINTS,
        });
    }
    let denominator = number + 1;
    let mut curves = Vec::with_capacity(number);
    for tween_index in 1..=number {
        let fraction = tween_index as Real / denominator as Real;
        let controls = (0..control_count)
            .map(|index| {
                let start_control = start_controls[index.min(start_controls.len() - 1)];
                let end_control = end_controls[index.min(end_controls.len() - 1)];
                // Rhino expands a shorter first control array with zero-weight
                // entries while retaining its final Euclidean location. A
                // shorter second array instead repeats its final full control.
                let start_weight = if index < start_controls.len() {
                    start_control.weight()
                } else {
                    0.0
                };
                let point = blend_points(start_control.point(), end_control.point(), fraction)?;
                let weight = blend_real(start_weight, end_control.weight(), fraction)?;
                WeightedPoint3::try_new(point, weight)
            })
            .collect::<Result<Vec<_>, _>>()?;
        curves.push(NurbsCurve::try_new_rational(
            template.degree(),
            controls,
            template.knots().to_vec(),
        )?);
    }
    Ok(curves)
}

fn blend_points(start: Point3, end: Point3, fraction: Real) -> Result<Point3, GeometryError> {
    let start = start.to_array();
    let end = end.to_array();
    Point3::try_from(std::array::from_fn(|coordinate| {
        start[coordinate].mul_add(1.0 - fraction, end[coordinate] * fraction)
    }))
}

fn blend_real(start: Real, end: Real, fraction: Real) -> Result<Real, GeometryError> {
    let value = start.mul_add(1.0 - fraction, end * fraction);
    require_finite([value], "curve tween weight")?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn controls(values: &[(Real, Real, Real, Real)]) -> Vec<WeightedPoint3> {
        values
            .iter()
            .map(|&(x, y, z, weight)| WeightedPoint3::try_new(point(x, y, z), weight).unwrap())
            .collect()
    }

    fn assert_near(actual: Real, expected: Real, epsilon: Real) {
        assert!(
            (actual - expected).abs() <= epsilon * actual.abs().max(expected.abs()).max(1.0),
            "expected {expected:.17e}, got {actual:.17e}"
        );
    }

    #[test]
    fn control_point_tweens_use_interior_fractions_and_start_structure() {
        let start = NurbsCurve::try_new_rational(
            2,
            controls(&[
                (0.0, 0.0, 0.0, 1.0),
                (2.0, 3.0, 0.0, 2.0),
                (4.0, 0.0, 0.0, 4.0),
            ]),
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let end = NurbsCurve::try_new_rational(
            2,
            controls(&[
                (0.0, 6.0, 2.0, 3.0),
                (2.0, 9.0, 4.0, 4.0),
                (4.0, 6.0, 2.0, 2.0),
            ]),
            vec![2.0, 2.0, 2.0, 8.0, 8.0, 8.0],
        )
        .unwrap();
        let curves = try_tween_nurbs_curves(
            &start,
            &end,
            2,
            CurveTweenMatchMethod::ControlPoint,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(curves.len(), 2);
        assert_eq!(curves[0].knots(), start.knots());
        assert_eq!(curves[0].domain(), 0.0..=1.0);
        for (curve, fraction) in curves.iter().zip([1.0 / 3.0, 2.0 / 3.0]) {
            for ((actual, first), last) in curve
                .control_points()
                .iter()
                .zip(start.control_points())
                .zip(end.control_points())
            {
                for ((coordinate, first), last) in actual
                    .point()
                    .to_array()
                    .into_iter()
                    .zip(first.point().to_array())
                    .zip(last.point().to_array())
                {
                    assert_near(
                        coordinate,
                        first * (1.0 - fraction) + last * fraction,
                        2.0e-15,
                    );
                }
                assert_near(
                    actual.weight(),
                    first.weight() * (1.0 - fraction) + last.weight() * fraction,
                    2.0e-15,
                );
            }
        }
    }

    #[test]
    fn unmatched_control_counts_repeat_locations_like_rhino() {
        let start = NurbsCurve::try_new(
            3,
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 2.0, 0.0),
                point(3.0, -1.0, 0.0),
                point(5.0, 0.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let end = NurbsCurve::try_new(
            3,
            vec![
                point(0.0, 6.0, 0.0),
                point(1.0, 8.0, 0.0),
                point(3.0, 5.0, 0.0),
                point(5.0, 8.0, 0.0),
                point(7.0, 6.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let curve = &try_tween_nurbs_curves(
            &start,
            &end,
            1,
            CurveTweenMatchMethod::ControlPoint,
            Tolerance::DEFAULT,
        )
        .unwrap()[0];
        assert_eq!(curve.degree(), 3);
        assert_eq!(curve.knots(), end.knots());
        assert_eq!(curve.control_points()[4].point(), point(6.0, 3.0, 0.0));
        assert_eq!(curve.control_points()[4].weight(), 0.5);
    }

    #[test]
    fn sample_matching_reproduces_rhino_source_interpolation_structure() {
        let start = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 0.0, 0.0),
                point(3.0, 5.0, 0.0),
                point(8.0, 0.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let end = NurbsCurve::try_new(
            3,
            vec![
                point(0.0, 6.0, 1.0),
                point(1.0, 9.0, 2.0),
                point(6.0, 4.0, -1.0),
                point(8.0, 7.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let curve = &try_tween_nurbs_curves(
            &start,
            &end,
            1,
            CurveTweenMatchMethod::SamplePoints { sample_number: 5 },
            Tolerance::DEFAULT,
        )
        .unwrap()[0];
        assert_eq!(curve.degree(), 3);
        assert_eq!(curve.control_points().len(), 8);
        let expected = [
            [0.0, 3.0, 0.5],
            [0.31265575238569965, 3.5080652962739354, 0.5545476657420163],
            [1.2317678499077533, 4.486426057765961, 0.6383098111966589],
            [2.9809685585909578, 4.642193356950808, 0.3201329734645392],
            [4.805044555705415, 4.27050765747252, 0.08572310633346512],
            [6.556939734477419, 3.5438456381351244, -0.15633406035297173],
            [7.586576962211255, 3.5237352358861043, -0.06403318049371869],
            [8.0, 3.5, 0.0],
        ];
        for (actual, expected) in curve.control_points().iter().zip(expected) {
            for (actual, expected) in actual.point().to_array().into_iter().zip(expected) {
                assert_near(actual, expected, 5.0e-9);
            }
        }
        assert_near(*curve.domain().end(), 9.71761875671063, 5.0e-9);
    }

    #[test]
    fn sampled_lines_remain_degree_one_and_keep_every_sample() {
        let start = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0, 0.0), point(9.0, 0.0, 0.0)],
            vec![0.0, 0.0, 9.0, 9.0],
        )
        .unwrap();
        let end = NurbsCurve::try_new(
            1,
            vec![point(0.0, 6.0, 3.0), point(9.0, 9.0, 0.0)],
            vec![-2.0, -2.0, 1.0, 1.0],
        )
        .unwrap();
        let curves = try_tween_nurbs_curves(
            &start,
            &end,
            2,
            CurveTweenMatchMethod::SamplePoints { sample_number: 4 },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(curves.iter().all(|curve| curve.degree() == 1));
        assert!(curves.iter().all(|curve| curve.control_points().len() == 5));
        assert_eq!(curves[0].knots(), &[0.0, 0.0, 2.25, 4.5, 6.75, 9.0, 9.0]);
    }

    #[test]
    fn sampled_closed_curves_use_a_clamped_nonperiodic_seam() {
        let closed_square = |z| {
            NurbsCurve::try_new(
                1,
                vec![
                    point(0.0, 0.0, z),
                    point(4.0, 0.0, z),
                    point(4.0, 4.0, z),
                    point(0.0, 4.0, z),
                    point(0.0, 0.0, z),
                ],
                vec![0.0, 0.0, 4.0, 8.0, 12.0, 16.0, 16.0],
            )
            .unwrap()
        };
        let curve = &try_tween_nurbs_curves(
            &closed_square(0.0),
            &closed_square(6.0),
            1,
            CurveTweenMatchMethod::SamplePoints { sample_number: 8 },
            Tolerance::DEFAULT,
        )
        .unwrap()[0];

        assert_eq!(curve.degree(), 3);
        assert_eq!(curve.control_points().len(), 11);
        assert!(curve.is_closed().unwrap());
        assert!(!curve.is_periodic());
        assert_eq!(
            curve.evaluate(*curve.domain().start()).unwrap(),
            curve.evaluate(*curve.domain().end()).unwrap()
        );
    }

    #[test]
    fn sampled_points_may_be_closer_than_the_model_tolerance() {
        let tolerance = Tolerance::try_new(1.0e-6, 1.0e-12, 1.0e-10).unwrap();
        let line = |y| {
            NurbsCurve::try_new(
                1,
                vec![point(0.0, y, 0.0), point(1.0e-5, y, 0.0)],
                vec![0.0, 0.0, 1.0e-5, 1.0e-5],
            )
            .unwrap()
        };
        let curve = &try_tween_nurbs_curves(
            &line(0.0),
            &line(1.0e-5),
            1,
            CurveTweenMatchMethod::SamplePoints { sample_number: 100 },
            tolerance,
        )
        .unwrap()[0];

        assert_eq!(curve.degree(), 1);
        assert_eq!(curve.control_points().len(), 101);
        assert_eq!(
            curve.control_points()[50].point(),
            point(5.0e-6, 5.0e-6, 0.0)
        );
    }

    #[test]
    fn refit_is_rejected_instead_of_silently_using_another_algorithm() {
        let start = NurbsCurve::try_new(
            3,
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 2.0, 0.0),
                point(3.0, -1.0, 0.0),
                point(5.0, 0.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 0.0, 5.0, 5.0, 5.0, 5.0],
        )
        .unwrap();
        let compatible = NurbsCurve::try_new(
            3,
            vec![
                point(0.0, 6.0, 0.0),
                point(1.0, 8.0, 0.0),
                point(3.0, 5.0, 0.0),
                point(5.0, 6.0, 0.0),
            ],
            vec![10.0, 10.0, 10.0, 10.0, 30.0, 30.0, 30.0, 30.0],
        )
        .unwrap();
        assert_eq!(
            try_tween_nurbs_curves(
                &start,
                &compatible,
                1,
                CurveTweenMatchMethod::Refit,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::UnsupportedCurveTweenRefit)
        );
    }

    #[test]
    fn validates_counts_before_allocating() {
        let curve = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0, 0.0), point(1.0, 0.0, 0.0)],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        assert!(matches!(
            try_tween_nurbs_curves(
                &curve,
                &curve,
                0,
                CurveTweenMatchMethod::ControlPoint,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidCurveTweenCount { .. })
        ));
        assert!(matches!(
            try_tween_nurbs_curves(
                &curve,
                &curve,
                MAX_CURVE_TWEEN_OUTPUT_CONTROL_POINTS / 2 + 1,
                CurveTweenMatchMethod::ControlPoint,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::TooManyCurveTweenControlPoints { .. })
        ));
        for sample_number in [1, MAX_CURVE_TWEEN_SAMPLE_NUMBER + 1] {
            assert!(matches!(
                try_tween_nurbs_curves(
                    &curve,
                    &curve,
                    1,
                    CurveTweenMatchMethod::SamplePoints { sample_number },
                    Tolerance::DEFAULT,
                ),
                Err(GeometryError::InvalidCurveTweenSampleCount { .. })
            ));
        }
    }
}
