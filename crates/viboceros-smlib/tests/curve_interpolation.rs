#![cfg(feature = "native")]
use viboceros_geometry::{
    CurveInterpolationOptions, CurveKnotSpacing, GeometryError, InterpolatedCurveClosure,
    NurbsCurve, Point3, Tolerance, Vector3,
};
use viboceros_smlib::{Error, interpolate_curve, interpolate_curve_for_command};

fn points() -> Vec<Point3> {
    [
        (0., 0., 0.),
        (1., 2., 0.5),
        (4., -1., 2.),
        (4.5, 3., -0.5),
        (10., 0., 1.),
    ]
    .map(|(x, y, z)| Point3::try_new(x, y, z).unwrap())
    .to_vec()
}
fn compare(a: &NurbsCurve, e: &NurbsCurve) {
    assert_eq!(a.degree(), e.degree());
    assert_eq!(a.knots(), e.knots());
    assert_eq!(a.is_periodic(), e.is_periodic());
    assert_eq!(a.control_points().len(), e.control_points().len());
    for (a, e) in a.control_points().iter().zip(e.control_points()) {
        for (a, e) in a.point().to_array().into_iter().zip(e.point().to_array()) {
            assert!((a - e).abs() <= 1e-10 + 1e-12 * a.abs().max(e.abs()));
        }
        assert_eq!(a.weight(), e.weight());
    }
    for i in 0..=64 {
        let t = a.parameter_at(i as f64 / 64.).unwrap();
        let p = a.evaluate(t).unwrap();
        let q = e.evaluate(t).unwrap();
        for (a, e) in p.to_array().into_iter().zip(q.to_array()) {
            assert!((a - e).abs() <= 1e-10 + 1e-12 * a.abs().max(e.abs()));
        }
    }
}

#[test]
fn native_interpolation_preserves_all_spacings_degrees_seams_and_endpoint_directions() {
    let points = points();
    let before = points.clone();
    for spacing in [
        CurveKnotSpacing::Uniform,
        CurveKnotSpacing::Chord,
        CurveKnotSpacing::SquareRootChord,
    ] {
        for degree in [1, 3] {
            for closure in [
                InterpolatedCurveClosure::Open,
                InterpolatedCurveClosure::Smooth,
                InterpolatedCurveClosure::Sharp,
            ] {
                let options = CurveInterpolationOptions::new(degree, spacing, closure);
                compare(
                    &interpolate_curve(&points, options, Tolerance::DEFAULT).unwrap(),
                    &NurbsCurve::try_interpolate(&points, options, Tolerance::DEFAULT).unwrap(),
                );
                compare(
                    &interpolate_curve_for_command(&points, options).unwrap(),
                    &NurbsCurve::try_interpolate_for_command(&points, options, Tolerance::DEFAULT)
                        .unwrap(),
                );
            }
        }
        for (start, end) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut options =
                CurveInterpolationOptions::new(3, spacing, InterpolatedCurveClosure::Open);
            if start {
                options = options.with_start_tangent(Vector3::try_new(-1., 0.2, 0.5).unwrap());
            }
            if end {
                options = options.with_end_tangent(Vector3::try_new(0., 1., -0.3).unwrap());
            }
            compare(
                &interpolate_curve_for_command(&points, options).unwrap(),
                &NurbsCurve::try_interpolate_for_command(&points, options, Tolerance::DEFAULT)
                    .unwrap(),
            );
        }
    }
    assert_eq!(points, before);
}

#[test]
fn two_point_sdk_command_degree_policies_and_periodic_two_point_systems_remain_distinct() {
    let p = points()[..2].to_vec();
    for closure in [
        InterpolatedCurveClosure::Open,
        InterpolatedCurveClosure::Smooth,
        InterpolatedCurveClosure::Sharp,
    ] {
        let options = CurveInterpolationOptions::new(3, CurveKnotSpacing::Chord, closure);
        compare(
            &interpolate_curve(&p, options, Tolerance::DEFAULT).unwrap(),
            &NurbsCurve::try_interpolate(&p, options, Tolerance::DEFAULT).unwrap(),
        );
        compare(
            &interpolate_curve_for_command(&p, options).unwrap(),
            &NurbsCurve::try_interpolate_for_command(&p, options, Tolerance::DEFAULT).unwrap(),
        );
    }
    let o = CurveInterpolationOptions::default();
    assert_eq!(
        interpolate_curve(&p, o, Tolerance::DEFAULT)
            .unwrap()
            .degree(),
        1
    );
    assert_eq!(interpolate_curve_for_command(&p, o).unwrap().degree(), 3);
}

#[test]
fn native_interpolation_keeps_small_channel_detail_and_command_nearby_points() {
    for tiny in [1e-18, 1e-180, 1e-280] {
        let p = [(0., 0.), (0.5, 1.), (1.7, -2.), (4., 3.)]
            .map(|(x, z)| Point3::try_new(x, 0., z * tiny).unwrap());
        for closure in [
            InterpolatedCurveClosure::Open,
            InterpolatedCurveClosure::Smooth,
        ] {
            let options = CurveInterpolationOptions::new(3, CurveKnotSpacing::Chord, closure);
            let a = interpolate_curve_for_command(&p, options).unwrap();
            let e =
                NurbsCurve::try_interpolate_for_command(&p, options, Tolerance::DEFAULT).unwrap();
            for (a, e) in a.control_points().iter().zip(e.control_points()) {
                assert!((a.point().z() / tiny - e.point().z() / tiny).abs() < 1e-10);
            }
        }
    }
    let p = [
        Point3::try_new(1e9, 0., 0.).unwrap(),
        Point3::try_new(1e9 + 1e-6, 2e-6, 0.).unwrap(),
        Point3::try_new(1e9 + 3e-6, -1e-6, 0.).unwrap(),
        Point3::try_new(1e9 + 5e-6, 1e-6, 0.).unwrap(),
    ];
    compare(
        &interpolate_curve_for_command(&p, CurveInterpolationOptions::default()).unwrap(),
        &NurbsCurve::try_interpolate_for_command(
            &p,
            CurveInterpolationOptions::default(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
    );
}

#[test]
fn open_banded_interpolation_covers_small_systems_and_the_public_point_budget() {
    for count in [3, 4, 64, viboceros_geometry::MAX_CURVE_INTERPOLATION_POINTS] {
        let points = (0..count)
            .map(|i| {
                let x = i as f64 / (count - 1) as f64;
                Point3::try_new(x + x * x, (17. * x).sin(), (5. * x).cos()).unwrap()
            })
            .collect::<Vec<_>>();
        for spacing in [
            CurveKnotSpacing::Uniform,
            CurveKnotSpacing::Chord,
            CurveKnotSpacing::SquareRootChord,
        ] {
            let options =
                CurveInterpolationOptions::new(3, spacing, InterpolatedCurveClosure::Open);
            compare(
                &interpolate_curve_for_command(&points, options).unwrap(),
                &NurbsCurve::try_interpolate_for_command(&points, options, Tolerance::DEFAULT)
                    .unwrap(),
            );
        }
    }
}

#[test]
fn invalid_interpolation_requests_preserve_geometry_errors() {
    let p = points();
    assert!(matches!(
        interpolate_curve_for_command(
            &p,
            CurveInterpolationOptions::new(
                2,
                CurveKnotSpacing::Chord,
                InterpolatedCurveClosure::Open
            )
        ),
        Err(Error::Geometry(
            GeometryError::UnsupportedCurveInterpolationDegree { actual: 2 }
        ))
    ));
    assert!(matches!(
        interpolate_curve_for_command(&[p[0], p[0]], CurveInterpolationOptions::default()),
        Err(Error::Geometry(_))
    ));
}
