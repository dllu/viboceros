#![cfg(feature = "native")]
use viboceros_geometry::{
    AffineTransform3, NurbsCurve, Point3, Tolerance, Vector3, WeightedPoint3,
};
use viboceros_smlib::KernelCurve;

#[test]
fn full_native_revolution_retains_profile_domains_weights_and_curve_handle() {
    let curve = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(Point3::try_new(2., 0., 0.).unwrap(), 1.).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(2., 0., 1.5).unwrap(), 0.75).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(2., 0., 3.).unwrap(), 1.).unwrap(),
        ],
        vec![2., 2., 2., 7., 7., 7.],
    )
    .unwrap();
    let native = KernelCurve::from_nurbs(&curve).unwrap();
    let before = native.evaluate(4.5).unwrap();
    let surface = native
        .revolved_surface(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            360.,
            4. * std::f64::consts::PI,
        )
        .unwrap();
    assert_eq!(surface.degree_u(), 2);
    assert_eq!(surface.degree_v(), 2);
    assert_eq!(surface.domain_v(), curve.domain());
    assert_eq!(surface.control_point_count_u(), 9);
    assert_eq!(
        surface.control_point(1, 1).unwrap().weight(),
        0.75 * 0.5_f64.sqrt()
    );
    for i in 0..=16 {
        let u = surface.parameter_at_u(i as f64 / 16.).unwrap();
        for j in 0..=8 {
            let v = curve.parameter_at(j as f64 / 8.).unwrap();
            let p = surface.evaluate(u, v).unwrap();
            let expected = curve.evaluate(v).unwrap();
            assert!((p.x().hypot(p.y()) - 2.).abs() < 1e-12);
            assert!((p.z() - expected.z()).abs() < 1e-12);
        }
    }
    assert_eq!(native.evaluate(4.5).unwrap(), before);
}

#[test]
fn signed_partial_sweeps_retain_all_profile_spans_and_correct_endpoints() {
    let point = |p: [f64; 3]| Point3::try_new(p[0], p[1], p[2]).unwrap();
    let curve = NurbsCurve::try_new(
        1,
        vec![
            point([2., 0., 0.]),
            point([3., 0., 1.]),
            point([2., 0., 3.]),
        ],
        vec![0., 0., 1., 2., 2.],
    )
    .unwrap();
    let native = KernelCurve::from_nurbs(&curve).unwrap();
    let origin = point([0., 0., 0.]);
    for axis in [
        Vector3::try_new(0., 0., 1.).unwrap(),
        Vector3::try_new(1., 1., 1.).unwrap(),
    ] {
        for sign in [1., -1.] {
            let direction = axis.scaled(sign).unwrap();
            let surface = native
                .revolved_surface(origin, direction, 120., 7.)
                .unwrap();
            assert_eq!(surface.control_point_count_v(), 3);
            assert_eq!(surface.domain_v(), curve.domain());
            let end = AffineTransform3::try_rotation(
                origin,
                direction.normalized(Tolerance::DEFAULT).unwrap(),
                120_f64.to_radians(),
            )
            .unwrap();
            for j in 0..=16 {
                let v = curve.parameter_at(j as f64 / 16.).unwrap();
                let p = curve.evaluate(v).unwrap();
                assert!(surface.evaluate(0., v).unwrap().distance_to(p).unwrap() < 1e-12);
                assert!(
                    surface
                        .evaluate(7., v)
                        .unwrap()
                        .distance_to(end.transform_point(p).unwrap())
                        .unwrap()
                        < 1e-12
                );
            }
        }
    }
    for angle in [0., -90., 361., f64::NAN] {
        assert!(
            native
                .revolved_surface(origin, Vector3::try_new(0., 0., 1.).unwrap(), angle, 7.)
                .is_err()
        );
    }
}
