#![cfg(feature = "native")]
use viboceros_geometry::{
    Circle3, CurveRef, GeometryError, NurbsCurve, Point3, Polyline3, Tolerance, UnitVector3,
    try_fit_curve,
};
use viboceros_smlib::{Error, fit_curve};
fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn compare(a: &NurbsCurve, e: &NurbsCurve) {
    assert_eq!(a.degree(), e.degree());
    assert_eq!(a.knots(), e.knots());
    assert_eq!(a.control_points().len(), e.control_points().len());
    for (a, e) in a.control_points().iter().zip(e.control_points()) {
        assert_eq!(a.weight(), 1.);
        assert_eq!(e.weight(), 1.);
        for (a, e) in a.point().to_array().into_iter().zip(e.point().to_array()) {
            assert!(
                (a - e).abs() < 1e-9 + 1e-12 * a.abs().max(e.abs()),
                "{a} != {e}"
            );
        }
    }
    for i in 0..=128 {
        let t = a.parameter_at(i as f64 / 128.).unwrap();
        assert!(
            a.evaluate(t)
                .unwrap()
                .distance_to(e.evaluate(t).unwrap())
                .unwrap()
                < 1e-8
        );
    }
}
#[test]
fn native_curve_fit_covers_all_degrees_smooth_closed_and_kink_constraints() {
    let smooth = NurbsCurve::try_new(
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
    let closed = Polyline3::try_new(
        vec![
            point(0., 0., 0.),
            point(5., 0., 0.),
            point(5., 5., 0.),
            point(0., 5., 0.),
            point(0., 0., 0.),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let before = (smooth.clone(), kinked.clone(), closed.clone());
    for degree in 1..=11 {
        for source in [
            CurveRef::NurbsCurve(&smooth),
            CurveRef::Polyline(&kinked),
            CurveRef::Polyline(&closed),
        ] {
            let actual = fit_curve(source, degree, 0.01, 0.1, Tolerance::DEFAULT).unwrap();
            let expected = try_fit_curve(source, degree, 0.01, 0.1, Tolerance::DEFAULT).unwrap();
            compare(&actual, &expected);
        }
    }
    assert_eq!((smooth, kinked, closed), before);
}
#[test]
fn native_cubic_fit_keeps_tight_circle_geometry_beyond_the_old_native_point_budget() {
    let circle = Circle3::try_new(
        point(0., 0., 0.),
        10.,
        UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let a = fit_curve(
        CurveRef::Circle(&circle),
        3,
        2.5e-10,
        1e-10,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert!(a.control_points().len() > 1000);
    assert!(a.control_points().len() <= viboceros_geometry::MAX_CURVE_FIT_CONTROL_POINTS);
    assert!(a.is_closed().unwrap());
    for i in 0..=257 {
        let t = a.parameter_at(i as f64 / 257.).unwrap();
        assert!(
            a.evaluate(t)
                .unwrap()
                .distance_to(circle.evaluate(t).unwrap())
                .unwrap()
                < 2.5e-10
        );
    }
    compare(
        &a,
        &try_fit_curve(
            CurveRef::Circle(&circle),
            3,
            2.5e-10,
            1e-10,
            Tolerance::DEFAULT,
        )
        .unwrap(),
    );
}
#[test]
fn native_fixed_row_fits_preserve_tiny_channels_and_translated_nearby_detail() {
    for tiny in [1e-18, 1e-180, 1e-280] {
        let s = NurbsCurve::try_new(
            3,
            vec![
                point(0., 0., 0.),
                point(1., 2., tiny),
                point(2., -1., -tiny),
                point(3., 0., tiny),
            ],
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap();
        for degree in [2, 3, 5, 11] {
            let a = fit_curve(
                CurveRef::NurbsCurve(&s),
                degree,
                0.01,
                0.1,
                Tolerance::DEFAULT,
            )
            .unwrap();
            let e = try_fit_curve(
                CurveRef::NurbsCurve(&s),
                degree,
                0.01,
                0.1,
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert_eq!(a.knots(), e.knots());
            for (a, e) in a.control_points().iter().zip(e.control_points()) {
                assert!((a.point().z() / tiny - e.point().z() / tiny).abs() < 1e-8);
            }
        }
    }
    let s = NurbsCurve::try_new(
        3,
        vec![
            point(1e9, 0., 0.),
            point(1e9 + 0.001, 0.002, 0.),
            point(1e9 + 0.003, -0.001, 0.),
            point(1e9 + 0.005, 0., 0.),
        ],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let a = fit_curve(CurveRef::NurbsCurve(&s), 3, 1e-5, 0.1, Tolerance::DEFAULT).unwrap();
    let e = try_fit_curve(CurveRef::NurbsCurve(&s), 3, 1e-5, 0.1, Tolerance::DEFAULT).unwrap();
    compare(&a, &e);
}
#[test]
fn native_fit_validation_retains_typed_errors_and_source_purity() {
    let s = Polyline3::try_new(
        vec![point(0., 0., 0.), point(1., 0., 0.)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let before = s.clone();
    assert!(matches!(
        fit_curve(CurveRef::Polyline(&s), 12, 0.01, 0.1, Tolerance::DEFAULT),
        Err(Error::Geometry(GeometryError::InvalidCurveFitDegree { .. }))
    ));
    assert!(matches!(
        fit_curve(CurveRef::Polyline(&s), 3, 0., 0.1, Tolerance::DEFAULT),
        Err(Error::Geometry(GeometryError::InvalidCurveFitTolerance))
    ));
    assert_eq!(s, before);
}

#[test]
fn native_constrained_cubic_fit_agrees_with_portable_solver_at_crossings_and_kinks() {
    use viboceros_geometry::{CurveFitConstraint, try_fit_cubic_curve_with_constraints};
    let smooth = NurbsCurve::try_new(
        3,
        vec![
            point(0., 0., 0.),
            point(1., 2., 3.),
            point(3., -1., 1.),
            point(4., 1., 2.),
        ],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap()
    .reversed()
    .unwrap()
    .try_reparameterized(-7. ..=13.)
    .unwrap();
    let kinked = Polyline3::try_new(
        vec![point(0., 0., 0.), point(2., 0., 0.), point(2., 3., 0.)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    for source in [CurveRef::NurbsCurve(&smooth), CurveRef::Polyline(&kinked)] {
        let mut pins = [0.79, 0., 0.321, 0.32, 1., 0.07, 0.5].map(|fraction| {
            let t = source.parameter_at(fraction).unwrap();
            CurveFitConstraint {
                source_parameter: t,
                point: source.evaluate(t).unwrap(),
            }
        });
        // Exercise an allowed small target displacement as well as exact source points.
        if matches!(source, CurveRef::NurbsCurve(_)) {
            pins[0].point = point(
                pins[0].point.x(),
                pins[0].point.y(),
                pins[0].point.z() + 1e-7,
            );
        }
        let a = viboceros_smlib::fit_cubic_curve_with_constraints(
            source,
            &pins,
            1e-5,
            0.,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let e = try_fit_cubic_curve_with_constraints(source, &pins, 1e-5, 0., Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(a.constraint_parameters, e.constraint_parameters);
        compare(&a.curve, &e.curve);
        for (pin, t) in pins.iter().zip(a.constraint_parameters) {
            assert!(a.curve.evaluate(t).unwrap().distance_to(pin.point).unwrap() < 1e-10);
        }
    }
}

#[test]
fn native_common_station_fits_agree_with_portable_controls_and_preserve_positions() {
    use viboceros_geometry::{CurveFitStation, try_fit_cubic_curve_at_stations};
    let smooth = NurbsCurve::try_new(
        3,
        vec![
            point(0., 0., 0.),
            point(1., 2., 3.),
            point(3., -1., 1.),
            point(4., 1., 2.),
        ],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap()
    .reversed()
    .unwrap()
    .try_reparameterized(-13. ..=29.)
    .unwrap();
    let corner = Polyline3::try_new(
        vec![point(0., 0., 0.), point(2., 0., 0.), point(2., 3., 0.)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    for source in [CurveRef::NurbsCurve(&smooth), CurveRef::Polyline(&corner)] {
        let stations = [1., 0.5, 0.].map(|fit_parameter| {
            let source_parameter = source.parameter_at(fit_parameter).unwrap();
            CurveFitStation {
                source_parameter,
                fit_parameter,
                point: source.evaluate(source_parameter).unwrap(),
            }
        });
        let actual = viboceros_smlib::fit_cubic_curve_at_stations(
            source,
            &stations,
            1e-6,
            0.1,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let expected =
            try_fit_cubic_curve_at_stations(source, &stations, 1e-6, 0.1, Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(actual.constraint_parameters, [1., 0.5, 0.]);
        compare(&actual.curve, &expected.curve);
        for station in stations {
            assert!(
                actual
                    .curve
                    .evaluate(station.fit_parameter)
                    .unwrap()
                    .distance_to(station.point)
                    .unwrap()
                    < 1e-11
            );
        }
    }
}
