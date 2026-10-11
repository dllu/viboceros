#![cfg(feature = "native")]
use viboceros_geometry::{Circle3, NurbsCurve, Point3, Tolerance, UnitVector3, WeightedPoint3};
use viboceros_smlib::{KernelCurve, Shape};
fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn rational_apex_surface_matches_affine_profile_samples_and_preserves_native_curve() {
    let source = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(point(0., 0., 0.), 1.).unwrap(),
            WeightedPoint3::try_new(point(1., 2., 1.), 0.5).unwrap(),
            WeightedPoint3::try_new(point(3., 0., 0.), 1.).unwrap(),
        ],
        vec![2., 2., 2., 7., 7., 7.],
    )
    .unwrap();
    let native = KernelCurve::from_nurbs(&source).unwrap();
    let apex = point(1., 2., 5.);
    let surface = native.apex_surface(apex).unwrap();
    assert_eq!(surface.degree_u(), 1);
    assert_eq!(surface.degree_v(), source.degree());
    assert_eq!(surface.domain_v(), source.domain());
    for i in 0..=16 {
        let v = source.parameter_at(i as f64 / 16.).unwrap();
        let p = source.evaluate(v).unwrap();
        assert!(native.evaluate(v).unwrap().distance_to(p).unwrap() < 1e-12);
        for j in 0..=8 {
            let f = j as f64 / 8.;
            let expected = p
                .translated(p.vector_to(apex).unwrap().scaled(f).unwrap())
                .unwrap();
            assert!(
                surface
                    .evaluate(surface.parameter_at_u(f).unwrap(), v)
                    .unwrap()
                    .distance_to(expected)
                    .unwrap()
                    < 1e-12
            );
        }
    }
}

#[test]
fn native_cone_cap_is_solid_with_correct_volume_and_preserved_profile() {
    let t = Tolerance::DEFAULT;
    let curve = Circle3::try_new(
        point(0., 0., 0.),
        2.,
        UnitVector3::try_new(0., 0., 1., t).unwrap(),
        t,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let native = KernelCurve::from_nurbs(&curve).unwrap();
    let before = native.evaluate(curve.parameter_at(0.375).unwrap()).unwrap();
    for height in [6., -6.] {
        let output = Shape::apex_solid(&native, point(1., 0., height), t)
            .unwrap()
            .to_brep(t)
            .unwrap();
        assert!(output.is_solid());
        assert!((output.signed_volume(t).unwrap() - 8. * std::f64::consts::PI).abs() < 1e-8);
        assert_eq!(
            native.evaluate(curve.parameter_at(0.375).unwrap()).unwrap(),
            before
        );
    }
}

#[test]
fn invalid_apex_paths_and_coplanar_caps_reject_without_changing_the_curve() {
    let t = Tolerance::DEFAULT;
    let source = Circle3::try_new(
        point(0., 0., 0.),
        2.,
        UnitVector3::try_new(0., 0., 1., t).unwrap(),
        t,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let native = KernelCurve::from_nurbs(&source).unwrap();
    let p = source.parameter_at(0.375).unwrap();
    let before = native.evaluate(p).unwrap();
    assert!(
        native
            .apex_surface(source.evaluate(*source.domain().start()).unwrap())
            .is_err()
    );
    assert!(Shape::apex_solid(&native, point(0., 0., 0.), t).is_err());
    assert_eq!(native.evaluate(p).unwrap(), before);
}
