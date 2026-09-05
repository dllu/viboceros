use super::*;
use crate::{LineSegment, WeightedPoint3};

fn point(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.0).unwrap()
}

fn line(a: [Real; 2], b: [Real; 2], domain: RangeInclusive<Real>) -> NurbsCurve {
    LineSegment::try_new(point(a[0], a[1]), point(b[0], b[1]), Tolerance::DEFAULT)
        .unwrap()
        .to_nurbs()
        .unwrap()
        .try_reparameterized(domain)
        .unwrap()
}

fn example() -> PolyCurve3 {
    let arc = NurbsCurve::try_new_rational(
        2,
        [
            (point(0.0, 0.0), 1.0),
            (point(1.0, 0.0), 0.5_f64.sqrt()),
            (point(1.0, 1.0), 1.0),
        ]
        .into_iter()
        .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
        .collect(),
        vec![-10.0, -10.0, -10.0, -6.0, -6.0, -6.0],
    )
    .unwrap();
    PolyCurve3::try_new(vec![
        line([-2.0, 0.0], [0.0, 0.0], -3.0..=-1.0),
        arc,
        line([1.0, 1.0], [1.0, 2.0], 20.0..=21.0),
    ])
    .unwrap()
}

fn near(a: Point3, b: Point3) {
    assert!(a.distance_to(b).unwrap() < 2e-12, "{a:?} != {b:?}");
}
fn vector_near(a: Vector3, b: Vector3) {
    for (a, b) in a.to_array().into_iter().zip(b.to_array()) {
        assert!((a - b).abs() < 2e-11, "{a} != {b}");
    }
}

#[test]
fn control_polygon_preserves_mixed_degrees_and_closes_the_seam() {
    let source = example();
    let expected = [
        point(-2.0, 0.0),
        point(0.0, 0.0),
        point(1.0, 0.0),
        point(1.0, 1.0),
        point(1.0, 2.0),
    ];
    assert_eq!(
        source
            .control_polygon(Tolerance::DEFAULT)
            .unwrap()
            .vertices(),
        &expected
    );
    let mut segments = source.segments().to_vec();
    segments.push(line([1.0, 2.0], [-2.0, 0.0], 0.0..=1.0).into());
    let closed = PolyCurve3::try_new(segments).unwrap();
    let polygon = closed.control_polygon(Tolerance::DEFAULT).unwrap();
    assert!(polygon.is_closed());
    assert_eq!(&polygon.vertices()[..5], &expected);
    assert_eq!(polygon.vertices().len(), 6);
}

#[test]
fn closed_segments_are_only_valid_as_the_entire_composite() {
    let closed = NurbsCurve::try_clamped_uniform(
        2,
        vec![
            point(0.0, 0.0),
            point(1.0, 2.0),
            point(2.0, -1.0),
            point(0.0, 0.0),
        ],
    )
    .unwrap();
    assert!(closed.is_closed().unwrap());
    assert!(PolyCurve3::try_new(vec![closed.clone()]).is_ok());
    assert!(matches!(
        PolyCurve3::try_new(vec![closed, line([0.0, 0.0], [2.0, 0.0], 0.0..=1.0)]),
        Err(GeometryError::InvalidPolyCurve { .. })
    ));
}

#[test]
fn retains_each_segment_and_maps_independent_domains() {
    let curve = example();
    assert_eq!(curve.parameters(), &[-3.0, -1.0, 3.0, 4.0]);
    assert_eq!(curve.segments()[1].domain(), -10.0..=-6.0);
    assert_eq!(curve.segments()[2].degree(), 1);
    let changed = curve.try_reparameterized(-12.0..=16.0).unwrap();
    assert_eq!(changed.segments(), curve.segments());
    assert_eq!(changed.parameters(), &[-12.0, -4.0, 12.0, 16.0]);
    for index in 0..3 {
        for step in 0..=32 {
            let local = curve.segments()[index]
                .parameter_at(step as Real / 32.0)
                .unwrap();
            let global = changed.polycurve_parameter(index, local).unwrap();
            assert!((changed.segment_parameter(index, global).unwrap() - local).abs() < 1e-13);
            near(
                changed.evaluate(global).unwrap(),
                curve.segments()[index].evaluate(local).unwrap(),
            );
        }
    }
    assert_eq!(curve, example());
}

#[test]
fn scales_both_derivatives_and_defines_one_sided_junctions() {
    let curve = example();
    let changed = curve.try_reparameterized(-12.0..=16.0).unwrap();
    for original in [-2.0, 0.0, 2.0, 3.5] {
        let (point_a, first_a, second_a) = curve
            .evaluate_with_second_derivative(original, CurveEvaluationSide::Right)
            .unwrap();
        let (point_b, first_b, second_b) = changed
            .evaluate_with_second_derivative(original * 4.0, CurveEvaluationSide::Right)
            .unwrap();
        near(point_a, point_b);
        vector_near(first_a.scaled(0.25).unwrap(), first_b);
        vector_near(second_a.scaled(0.0625).unwrap(), second_b);
    }
    let left = curve
        .evaluate_with_second_derivative(-1.0, CurveEvaluationSide::Left)
        .unwrap();
    let right = curve
        .evaluate_with_second_derivative(-1.0, CurveEvaluationSide::Right)
        .unwrap();
    near(left.0, right.0);
    assert!((left.1.x() - 1.0).abs() < 1e-13);
    assert!((right.1.x() - 0.5_f64.sqrt() / 2.0).abs() < 1e-13);
    assert!(right.2.length().unwrap() > 0.1);
    assert_eq!(
        curve
            .segment_index(-3.0, CurveEvaluationSide::Left)
            .unwrap(),
        0
    );
    assert_eq!(
        curve
            .segment_index(4.0, CurveEvaluationSide::Right)
            .unwrap(),
        2
    );
}

#[test]
fn measures_analytic_length_independently_of_domains() {
    let curve = example();
    let expected = 3.0 + std::f64::consts::FRAC_PI_2;
    assert!((curve.length(Tolerance::DEFAULT).unwrap() - expected).abs() < 1e-12);
    let changed = curve.try_reparameterized(1e100..=2e100).unwrap();
    assert!((changed.length(Tolerance::DEFAULT).unwrap() - expected).abs() < 1e-12);
}

#[test]
fn trims_and_splits_inside_segments_and_exactly_at_junctions() {
    let curve = example();
    for interval in [-2.0..=3.5, -1.0..=3.0, 0.0..=2.0, -2.0..=-1.0, 3.0..=4.0] {
        let trimmed = curve.try_trimmed(interval.clone()).unwrap();
        assert_eq!(trimmed.domain(), interval);
        for step in 0..=32 {
            let t = trimmed.parameter_at(step as Real / 32.0).unwrap();
            near(trimmed.evaluate(t).unwrap(), curve.evaluate(t).unwrap());
        }
    }
    let middle = curve.try_trimmed(-1.0..=3.0).unwrap();
    assert_eq!(middle.segments(), &curve.segments()[1..2]);
    for t in [-2.0, -1.0, 1.0, 3.0, 3.5] {
        let (left, right) = curve.try_split(t).unwrap();
        near(left.evaluate(t).unwrap(), right.evaluate(t).unwrap());
        let joined = PolyCurve3::concatenate(&[left, right]).unwrap();
        assert_eq!(joined.domain(), curve.domain());
        assert!(
            (joined.length(Tolerance::DEFAULT).unwrap()
                - curve.length(Tolerance::DEFAULT).unwrap())
            .abs()
                < 2e-12
        );
    }
    assert_eq!(curve.try_trimmed(curve.domain()).unwrap(), curve);
}

#[test]
fn reverses_parameter_sign_and_flattens_without_refitting() {
    let curve = example();
    let reversed = curve.reversed().unwrap();
    assert_eq!(reversed.domain(), -4.0..=3.0);
    assert_eq!(reversed.reversed().unwrap(), curve);
    for step in 0..=32 {
        let t = curve.parameter_at(step as Real / 32.0).unwrap();
        near(curve.evaluate(t).unwrap(), reversed.evaluate(-t).unwrap());
        if curve.parameters().contains(&t) {
            continue;
        }
        vector_near(
            curve
                .evaluate_with_derivative(t)
                .unwrap()
                .1
                .scaled(-1.0)
                .unwrap(),
            reversed.evaluate_with_derivative(-t).unwrap().1,
        );
    }
    let out_and_back = PolyCurve3::concatenate(&[curve.clone(), reversed]).unwrap();
    assert_eq!(out_and_back.segments().len(), 6);
    assert_eq!(&out_and_back.segments()[..3], curve.segments());
    assert!(out_and_back.is_closed().unwrap());
    assert!(!curve.is_closed().unwrap());
}

#[test]
fn exact_nurbs_conversion_preserves_rational_segments_and_parameterization() {
    let curve = example().try_reparameterized(-12.0..=16.0).unwrap();
    let nurbs = curve.to_nurbs().unwrap();
    assert_eq!(nurbs.degree(), 2);
    assert_eq!(nurbs.domain(), curve.domain());
    for &junction in &curve.parameters()[1..3] {
        assert_eq!(nurbs.knot_multiplicity(junction).unwrap(), 2);
    }
    for step in 0..=128 {
        let t = curve.parameter_at(step as Real / 128.0).unwrap();
        near(curve.evaluate(t).unwrap(), nurbs.evaluate(t).unwrap());
        if curve.parameters().contains(&t) {
            continue;
        }
        vector_near(
            curve.evaluate_with_derivative(t).unwrap().1,
            nurbs.derivative_at(t).unwrap(),
        );
    }
    let pieces = nurbs
        .try_split_at_parameters(&curve.parameters()[1..3])
        .unwrap();
    assert_eq!(pieces.len(), 3);
    assert!(
        (nurbs.length(Tolerance::DEFAULT).unwrap() - curve.length(Tolerance::DEFAULT).unwrap())
            .abs()
            < 2e-12
    );
}

#[test]
fn conversion_keeps_independent_extreme_homogeneous_scales() {
    let curve = example();
    let segments = curve
        .segments()
        .iter()
        .enumerate()
        .map(|(index, s)| {
            let s = s.to_nurbs().unwrap();
            let factor = [1e-200, 1e200, -1.0][index];
            NurbsCurve::try_new_rational(
                s.degree(),
                s.control_points()
                    .iter()
                    .map(|c| WeightedPoint3::try_new(c.point(), c.weight() * factor).unwrap())
                    .collect(),
                s.knots().to_vec(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let scaled =
        PolyCurve3::try_with_segment_domains(segments, curve.parameters().to_vec()).unwrap();
    let nurbs = scaled.to_nurbs().unwrap();
    for &junction in &scaled.parameters()[1..3] {
        assert_eq!(nurbs.knot_multiplicity(junction).unwrap(), 3);
    }
    for step in 0..=128 {
        let t = curve.parameter_at(step as Real / 128.0).unwrap();
        near(curve.evaluate(t).unwrap(), nurbs.evaluate(t).unwrap());
    }
}

#[test]
fn affine_transform_keeps_segment_structure_and_large_translation() {
    let curve = example();
    let transform =
        AffineTransform3::from_translation(Vector3::try_new(1e12, -2e12, 3e12).unwrap());
    let translated = curve.transformed(transform).unwrap();
    assert_eq!(curve.parameters(), translated.parameters());
    for (a, b) in curve.segments().iter().zip(translated.segments()) {
        let a = a.to_nurbs().unwrap();
        let b = b.to_nurbs().unwrap();
        assert_eq!(a.knots(), b.knots());
        for (a, b) in a.control_points().iter().zip(b.control_points()) {
            assert_eq!(a.weight(), b.weight());
            assert_eq!(transform.transform_point(a.point()).unwrap(), b.point());
        }
    }
    assert!(translated.control_point_bounds().min().x() >= 1e12 - 2.0);
}

#[test]
fn rejects_bad_domains_gaps_counts_indices_and_nonfinite_parameters() {
    assert!(PolyCurve3::try_new::<CurveSegment3>(vec![]).is_err());
    assert!(PolyCurve3::concatenate(&[]).is_err());
    let curve = example();
    for breaks in [
        vec![0.0],
        vec![0.0, 1.0, 1.0, 3.0],
        vec![0.0, Real::NAN, 2.0, 3.0],
        vec![-Real::MAX, -1.0, 1.0, Real::MAX],
    ] {
        assert!(PolyCurve3::try_with_segment_domains(curve.segments().to_vec(), breaks).is_err());
    }
    assert!(
        PolyCurve3::try_new(vec![
            curve.segments()[0].clone(),
            curve.segments()[2].clone()
        ])
        .is_err()
    );
    assert!(
        PolyCurve3::try_new(vec![
            curve.segments()[0].clone();
            MAX_POLYCURVE_SEGMENTS + 1
        ])
        .is_err()
    );
    assert!(curve.try_reparameterized(4.0..=4.0).is_err());
    assert!(curve.try_trimmed(-4.0..=0.0).is_err());
    assert!(curve.try_split(-3.0).is_err());
    assert!(curve.try_split(Real::NAN).is_err());
    assert!(curve.evaluate(Real::INFINITY).is_err());
    assert!(curve.evaluate(Real::NAN).is_err());
    assert!(curve.segment_domain(3).is_err());
    assert!(curve.polycurve_parameter(3, 0.0).is_err());
}

#[test]
fn rejects_parameter_spans_lost_by_append_or_reparameterization_roundoff() {
    let first = line([-2.0, 0.0], [0.0, 0.0], 1e16..=1e16 + 2.0);
    let next = line([0.0, 0.0], [1.0, 0.0], 0.0..=1.0);
    assert!(PolyCurve3::try_new(vec![first.clone(), next.clone()]).is_err());
    assert!(
        PolyCurve3::concatenate(&[
            PolyCurve3::try_new(vec![first]).unwrap(),
            PolyCurve3::try_new(vec![next]).unwrap()
        ])
        .is_err()
    );
    assert!(example().try_reparameterized(1e16..=1e16 + 2.0).is_err());
}

#[test]
fn derivative_scaling_avoids_overflow_and_subnormal_ratio_loss() {
    assert!((scaled_ratio(0.1, 0.1, 1e-310).unwrap() / 1e308 - 1.0).abs() < 1e-13);
    assert!((scaled_ratio(1e-200, 1e200, 1e-200).unwrap() / 1e200 - 1.0).abs() < 1e-14);
    assert!((scaled_ratio(1e300, 1e-300, 1e20).unwrap() / 1e-20 - 1.0).abs() < 1e-14);
    assert!(scaled_ratio(1e200, 1e200, 1e-200).is_err());
}

#[test]
fn generic_curve_sampling_measures_equal_arc_length_across_junctions() {
    let curve = example().try_reparameterized(-12.0..=16.0).unwrap();
    let borrowed = crate::CurveRef::PolyCurve(&curve);
    assert!(borrowed.is_planar(Tolerance::DEFAULT).unwrap());
    let points = borrowed
        .divide_by_count(37, true, Tolerance::DEFAULT)
        .unwrap();
    let total = 3.0 + std::f64::consts::FRAC_PI_2;
    for (index, actual) in points.into_iter().enumerate() {
        let distance = index as Real * total / 37.0;
        let expected = if distance <= 2.0 {
            point(distance - 2.0, 0.0)
        } else if distance <= 2.0 + std::f64::consts::FRAC_PI_2 {
            point((distance - 2.0).sin(), 1.0 - (distance - 2.0).cos())
        } else {
            point(1.0, distance - 1.0 - std::f64::consts::FRAC_PI_2)
        };
        assert!(actual.distance_to(expected).unwrap() < 2e-10);
    }
    vector_near(
        borrowed.curvature_vector(4.0).unwrap(),
        Vector3::try_new(-0.5_f64.sqrt(), 0.5_f64.sqrt(), 0.0).unwrap(),
    );
}

#[test]
fn single_periodic_segment_and_unclamped_segments_preserve_the_active_locus() {
    let periodic = NurbsCurve::try_control_point_curve_with_closure(
        3,
        vec![
            point(-1.0, -1.0),
            point(1.0, -1.0),
            point(1.0, 1.0),
            point(-1.0, 1.0),
        ],
        crate::ControlPointCurveClosure::Smooth,
    )
    .unwrap();
    assert!(periodic.is_periodic());
    let curve = PolyCurve3::try_new(vec![periodic.clone()]).unwrap();
    assert!(curve.is_closed().unwrap());
    assert_eq!(
        curve.segments()[0],
        CurveSegment3::NurbsCurve(periodic.clone())
    );
    for source in [
        periodic,
        NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 0.0),
                point(1.0, 2.0),
                point(3.0, 1.0),
                point(4.0, 0.0),
            ],
            vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0],
        )
        .unwrap(),
    ] {
        let curve = PolyCurve3::try_new(vec![source.clone()]).unwrap();
        let converted = curve.to_nurbs().unwrap();
        for step in 0..=32 {
            let t = source.parameter_at(step as Real / 32.0).unwrap();
            near(source.evaluate(t).unwrap(), converted.evaluate(t).unwrap());
        }
        let trimmed = curve
            .try_trimmed(source.parameter_at(0.125).unwrap()..=source.parameter_at(0.875).unwrap())
            .unwrap();
        for step in 0..=32 {
            let t = trimmed.parameter_at(step as Real / 32.0).unwrap();
            near(source.evaluate(t).unwrap(), trimmed.evaluate(t).unwrap());
        }
    }
}

#[test]
fn length_based_domains_preserve_segments_but_change_relative_parameter_speeds() {
    let curve = example();
    let changed = curve
        .try_reparameterized_by_length(-12.0..=16.0, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(changed.segments(), curve.segments());
    let total = 3.0 + std::f64::consts::FRAC_PI_2;
    assert!((changed.parameters()[1] - (-12.0 + 28.0 * 2.0 / total)).abs() < 2e-12);
    assert!((changed.parameters()[2] - (16.0 - 28.0 / total)).abs() < 2e-12);
    for (index, segment) in curve.segments().iter().enumerate() {
        for step in 0..=32 {
            let t = segment.parameter_at(step as Real / 32.0).unwrap();
            let global = changed.polycurve_parameter(index, t).unwrap();
            near(
                changed.evaluate(global).unwrap(),
                segment.evaluate(t).unwrap(),
            );
        }
    }
    let source =
        NurbsCurve::try_new(1, vec![point(0.0, 0.0); 2], vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let collapsed = PolyCurve3::try_new(vec![source]).unwrap();
    assert!(
        collapsed
            .try_reparameterized_by_length(0.0..=1.0, Tolerance::DEFAULT)
            .is_err()
    );
}
