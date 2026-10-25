use super::*;
use crate::{Circle3, LineSegment, Polyline3, UnitVector3};

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn external_rebuild_solver_observes_shared_normalization_and_validates_layout() {
    let line = LineSegment::try_new(
        point(1e9, 0., 0.),
        point(1e9 + 1., 0., 0.),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut calls = 0;
    let result = try_rebuild_curve_with_interpolation(
        CurveRef::Line(&line),
        6,
        3,
        false,
        Tolerance::DEFAULT,
        &mut |input| {
            calls += 1;
            assert_eq!(input.degree, 3);
            assert!(!input.periodic);
            assert_eq!(input.targets.len(), 6);
            assert_eq!(input.targets[0], [0.; 3]);
            assert_eq!(input.targets[5], [1., 0., 0.]);
            solve_portable_rebuild(input)
        },
    )
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(
        result,
        try_rebuild_curve(CurveRef::Line(&line), 6, 3, false, Tolerance::DEFAULT).unwrap()
    );
    let bad = try_rebuild_curve_with_interpolation(
        CurveRef::Line(&line),
        6,
        3,
        false,
        Tolerance::DEFAULT,
        &mut |_| -> Result<Vec<[Real; 3]>, GeometryError> { Ok(Vec::new()) },
    );
    assert!(matches!(bad, Err(GeometryError::InvalidControlNet { .. })));
    let invalid = try_rebuild_curve_with_interpolation(
        CurveRef::Line(&line),
        0,
        3,
        false,
        Tolerance::DEFAULT,
        &mut |_| -> Result<Vec<[Real; 3]>, GeometryError> {
            panic!("invalid request entered solver")
        },
    );
    assert!(matches!(
        invalid,
        Err(GeometryError::InvalidCurveRebuildPointCount { .. })
    ));
}

fn assert_near(actual: Real, expected: Real, epsilon: Real) {
    assert!(
        (actual - expected).abs() <= epsilon,
        "expected {expected:.16e}, got {actual:.16e}"
    );
}

fn assert_point_near(actual: Point3, expected: Point3, epsilon: Real) {
    for (actual, expected) in actual.to_array().into_iter().zip(expected.to_array()) {
        assert_near(actual, expected, epsilon);
    }
}

#[test]
fn open_line_matches_rhino_uniform_cubic_structure() {
    let line = LineSegment::try_new(
        point(0.0, 0.0, 0.0),
        point(10.0, 0.0, 0.0),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let curve = try_rebuild_curve(CurveRef::Line(&line), 6, 3, false, Tolerance::DEFAULT).unwrap();
    assert_eq!(curve.degree(), 3);
    assert_eq!(curve.domain(), 0.0..=3.0);
    assert_eq!(
        curve.knots(),
        &[0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0, 3.0, 3.0]
    );
    for (control, x) in curve.control_points().iter().zip([
        0.0,
        10.0 / 9.0,
        10.0 / 3.0,
        20.0 / 3.0,
        80.0 / 9.0,
        10.0,
    ]) {
        assert_point_near(control.point(), point(x, 0.0, 0.0), 8.0e-15);
        assert_eq!(control.weight(), 1.0);
    }
}

#[test]
fn open_rebuild_interpolates_equal_length_greville_stations() {
    let source = NurbsCurve::try_new(
        3,
        vec![
            point(0.0, 0.0, 0.0),
            point(2.0, 4.0, 0.0),
            point(7.0, -2.0, 1.0),
            point(10.0, 0.0, 0.0),
        ],
        vec![5.0, 5.0, 5.0, 5.0, 15.0, 15.0, 15.0, 15.0],
    )
    .unwrap();
    let curve = try_rebuild_curve(
        CurveRef::NurbsCurve(&source),
        6,
        3,
        false,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut sampler =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&source), Tolerance::DEFAULT).unwrap();
    sampler.prepare_budgeted_repeated_sampling(32).unwrap();
    for (index, parameter) in [0.0, 1.0 / 3.0, 1.0, 2.0, 8.0 / 3.0, 3.0]
        .into_iter()
        .enumerate()
    {
        let expected = sampler
            .point_at_distance(sampler.total_length() * (parameter / 3.0))
            .unwrap();
        assert_point_near(curve.evaluate(parameter).unwrap(), expected, 2.0e-12);
        if index == 0 || index == 5 {
            assert_eq!(curve.evaluate(parameter).unwrap(), expected);
        }
    }
}

#[test]
fn preserving_tangents_projects_only_the_end_handles() {
    let source = NurbsCurve::try_new(
        3,
        vec![
            point(0.0, 0.0, 0.0),
            point(2.0, 4.0, 0.0),
            point(7.0, -2.0, 1.0),
            point(10.0, 0.0, 0.0),
        ],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let free = try_rebuild_curve(
        CurveRef::NurbsCurve(&source),
        6,
        3,
        false,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let preserved = try_rebuild_curve(
        CurveRef::NurbsCurve(&source),
        6,
        3,
        true,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(
        &free.control_points()[2..4],
        &preserved.control_points()[2..4]
    );
    let source_start = CurveRef::NurbsCurve(&source)
        .evaluate_with_tangent(0.0)
        .unwrap()
        .tangent();
    let source_end = CurveRef::NurbsCurve(&source)
        .evaluate_with_tangent(1.0)
        .unwrap()
        .tangent();
    let rebuilt_start = CurveRef::NurbsCurve(&preserved)
        .evaluate_with_tangent(0.0)
        .unwrap()
        .tangent();
    let rebuilt_end = CurveRef::NurbsCurve(&preserved)
        .evaluate_with_tangent(3.0)
        .unwrap()
        .tangent();
    assert_near(
        source_start
            .as_vector()
            .dot(rebuilt_start.as_vector())
            .unwrap(),
        1.0,
        2.0e-14,
    );
    assert_near(
        source_end.as_vector().dot(rebuilt_end.as_vector()).unwrap(),
        1.0,
        2.0e-14,
    );
}

#[test]
fn closed_rebuilds_use_rhino_periodic_layout_and_arc_stations() {
    let square = Polyline3::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(4.0, 4.0, 0.0),
            point(0.0, 4.0, 0.0),
            point(0.0, 0.0, 0.0),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let square_sampler =
        ArcLengthSampler::try_new(CurveRef::Polyline(&square), Tolerance::DEFAULT).unwrap();
    for degree in 1..=5 {
        let curve = try_rebuild_curve(
            CurveRef::Polyline(&square),
            8,
            degree,
            true,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(curve.degree(), degree);
        assert_eq!(curve.control_points().len(), 8 + degree);
        assert_eq!(curve.domain(), 0.0..=8.0);
        assert!(curve.is_closed().unwrap());
        assert_eq!(curve.is_periodic(), degree > 1);
        let offset = if degree.is_multiple_of(2) { 0.5 } else { 0.0 };
        for index in 0..8 {
            let parameter = index as Real + offset;
            let expected = square_sampler.point_at_distance(2.0 * parameter).unwrap();
            assert_point_near(curve.evaluate(parameter).unwrap(), expected, 2.0e-12);
        }
    }

    let cubic =
        try_rebuild_curve(CurveRef::Polyline(&square), 8, 3, false, Tolerance::DEFAULT).unwrap();
    assert_point_near(
        cubic.control_points()[0].point(),
        point(2.0 / 7.0, 2.0, 0.0),
        2.0e-12,
    );
    assert_point_near(
        cubic.control_points()[1].point(),
        point(-4.0 / 7.0, -4.0 / 7.0, 0.0),
        2.0e-12,
    );

    let raised =
        try_rebuild_curve(CurveRef::Polyline(&square), 2, 4, false, Tolerance::DEFAULT).unwrap();
    assert_eq!(raised.control_points().len(), 8);
    assert_eq!(raised.domain(), 0.0..=4.0);
}

#[test]
fn validates_degree_and_open_or_closed_point_limits() {
    let line = LineSegment::try_new(
        point(0.0, 0.0, 0.0),
        point(1.0, 0.0, 0.0),
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(
        try_rebuild_curve(CurveRef::Line(&line), 6, 0, false, Tolerance::DEFAULT),
        Err(GeometryError::InvalidCurveRebuildDegree {
            actual: 0,
            maximum: MAX_CURVE_REBUILD_DEGREE,
        })
    );
    let raised = try_rebuild_curve(CurveRef::Line(&line), 3, 3, false, Tolerance::DEFAULT).unwrap();
    assert_eq!(raised.control_points().len(), 4);
    assert_eq!(raised.domain(), 0.0..=1.0);
    assert_eq!(
        try_rebuild_curve(CurveRef::Line(&line), 0, 3, false, Tolerance::DEFAULT),
        Err(GeometryError::InvalidCurveRebuildPointCount {
            actual: 0,
            minimum: 1,
            maximum: MAX_CURVE_REBUILD_POINT_COUNT,
        })
    );
    assert_eq!(
        try_rebuild_curve(
            CurveRef::Line(&line),
            MAX_CURVE_REBUILD_POINT_COUNT + 1,
            3,
            false,
            Tolerance::DEFAULT,
        ),
        Err(GeometryError::InvalidCurveRebuildPointCount {
            actual: MAX_CURVE_REBUILD_POINT_COUNT + 1,
            minimum: 1,
            maximum: MAX_CURVE_REBUILD_POINT_COUNT,
        })
    );
}

#[test]
fn thousand_point_open_rebuild_remains_bounded_and_exact_for_a_line() {
    let offset = 1.0e12;
    let line = LineSegment::try_new(
        point(offset - 5.0, offset + 2.0, offset + 1.0),
        point(offset + 9.0, offset - 3.0, offset + 4.0),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let curve = try_rebuild_curve(
        CurveRef::Line(&line),
        MAX_CURVE_REBUILD_POINT_COUNT,
        MAX_CURVE_REBUILD_DEGREE,
        false,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(curve.control_points().len(), MAX_CURVE_REBUILD_POINT_COUNT);
    assert_point_near(
        curve.evaluate(*curve.domain().start()).unwrap(),
        line.start(),
        2.0e-12,
    );
    assert_point_near(
        curve.evaluate(*curve.domain().end()).unwrap(),
        line.end(),
        2.0e-12,
    );
    assert_point_near(
        curve.evaluate(0.5 * *curve.domain().end()).unwrap(),
        point(offset + 2.0, offset - 0.5, offset + 2.5),
        2.0e-3,
    );
}

#[test]
fn thousand_point_closed_rebuild_is_stable_far_from_the_origin() {
    let offset = 1.0e12;
    let square = Polyline3::try_new(
        vec![
            point(offset, offset, offset),
            point(offset + 400.0, offset, offset),
            point(offset + 400.0, offset + 400.0, offset),
            point(offset, offset + 400.0, offset),
            point(offset, offset, offset),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let curve = try_rebuild_curve(
        CurveRef::Polyline(&square),
        MAX_CURVE_REBUILD_POINT_COUNT,
        MAX_CURVE_REBUILD_DEGREE,
        false,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(
        curve.control_points().len(),
        MAX_CURVE_REBUILD_POINT_COUNT + MAX_CURVE_REBUILD_DEGREE
    );
    assert!(curve.is_periodic());

    let sampler =
        ArcLengthSampler::try_new(CurveRef::Polyline(&square), Tolerance::DEFAULT).unwrap();
    for index in [0, 137, 500, 999] {
        let expected = sampler
            .point_at_distance(
                sampler.total_length() * (index as Real / MAX_CURVE_REBUILD_POINT_COUNT as Real),
            )
            .unwrap();
        assert_point_near(curve.evaluate(index as Real).unwrap(), expected, 2.0e-3);
    }
}

#[test]
fn forced_open_rebuild_uses_clamped_sampling_for_a_closed_source() {
    let circle = Circle3::try_new(
        Point3::try_new(0., 0., 0.).unwrap(),
        1.,
        UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let rebuilt = try_rebuild_open_curve_with_interpolation(
        CurveRef::Circle(&circle),
        7,
        3,
        Tolerance::DEFAULT,
        &mut |input| -> Result<_, GeometryError> {
            assert!(!input.periodic);
            assert_eq!(&input.knots[..4], &[0.; 4]);
            solve_portable_rebuild(input)
        },
    )
    .unwrap();
    assert_eq!(rebuilt.control_points().len(), 7);
    assert_eq!(
        rebuilt.control_points()[0].point(),
        rebuilt.control_points()[6].point()
    );
}
