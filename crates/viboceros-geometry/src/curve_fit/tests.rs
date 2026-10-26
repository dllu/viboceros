use super::*;
use crate::{LineSegment, Polyline3, WeightedPoint3};

#[test]
fn external_fit_solver_observes_fixed_rows_and_direct_paths_bypass_it() {
    let line =
        LineSegment::try_new(point(0., 0., 0.), point(10., 0., 0.), Tolerance::DEFAULT).unwrap();
    let mut calls = 0;
    let result = try_fit_curve_with_solver(
        CurveRef::Line(&line),
        5,
        0.001,
        0.1,
        Tolerance::DEFAULT,
        &mut |input| -> Result<Vec<Real>, GeometryError> {
            calls += 1;
            assert_eq!(input.fixed_rows, &[1, 1, 0, 0, 1, 1]);
            assert_eq!(input.system.columns, 4);
            assert!(input.system.targets.chunks_exact(4).all(|p| p[3] == 1.));
            // Linear precision independently gives these Bezier controls.
            Ok(input.system.targets.to_vec())
        },
    )
    .unwrap();
    assert_eq!(calls, 1);
    for (i, p) in result.control_points().iter().enumerate() {
        assert_eq!(p.point(), point(2. * i as Real, 0., 0.));
    }
    for degree in [1, 3] {
        try_fit_curve_with_solver(
            CurveRef::Line(&line),
            degree,
            0.001,
            0.1,
            Tolerance::DEFAULT,
            &mut |_| -> Result<Vec<Real>, GeometryError> {
                panic!("fully constrained fit entered solver")
            },
        )
        .unwrap();
    }
    let invalid = try_fit_curve_with_solver(
        CurveRef::Line(&line),
        5,
        0.001,
        0.1,
        Tolerance::DEFAULT,
        &mut |_| -> Result<Vec<Real>, GeometryError> { Ok(Vec::new()) },
    );
    assert!(matches!(
        invalid,
        Err(GeometryError::InvalidControlNet { .. })
    ));
}

#[test]
fn cubic_full_circle_refit_reaches_default_sweep_tolerance() {
    for radius in [3., 10., 100.] {
        let circle = crate::Circle3::try_new(
            Point3::try_new(0., 0., 0.).unwrap(),
            radius,
            crate::UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let fit = try_fit_curve(
            CurveRef::Circle(&circle),
            3,
            2.5e-10,
            1e-10,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(fit.control_points().len() <= MAX_CURVE_FIT_CONTROL_POINTS);
        assert!(fit.is_closed().unwrap(), "radius {radius}");
        for i in 0..=257 {
            let t = *fit.domain().end() * i as Real / 257.;
            let source = circle.evaluate(t).unwrap();
            let actual = fit.evaluate(t).unwrap();
            assert!(
                source.distance_to(actual).unwrap() < 2.5e-10,
                "radius {radius}, station {i}"
            );
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
    let arc =
        crate::CircularArc3::try_from_circle_sweep(circle, std::f64::consts::FRAC_PI_2).unwrap();
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
