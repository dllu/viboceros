#![cfg(feature = "native")]
use viboceros_geometry::{Circle3, NurbsCurve, Point3, Tolerance, UnitVector3, Vector3};
use viboceros_smlib::{KernelCurve, Shape};
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn v(x: f64, y: f64, z: f64) -> Vector3 {
    Vector3::try_new(x, y, z).unwrap()
}
fn tol() -> Tolerance {
    Tolerance::try_new(1e-5, 1e-12, 1e-8).unwrap()
}
fn circle() -> KernelCurve {
    let c = Circle3::try_new(
        p(0., 0., 0.),
        3.,
        UnitVector3::try_new(0., 0., 1., tol()).unwrap(),
        tol(),
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    KernelCurve::from_nurbs_for_modeling(&c).unwrap()
}
#[test]
fn native_tapered_circle_matches_analytic_frustum_and_retains_source() {
    let profile = circle();
    let before = profile.to_nurbs().unwrap();
    for (height, angle) in [(2., 10.), (-2., 10.), (2., -10.), (2., 0.)] {
        let shape =
            Shape::tapered_extrusion(&profile, v(0., 0., height), angle, false, true, tol())
                .unwrap();
        let brep = shape.to_brep(tol()).unwrap();
        assert!(brep.is_solid());
        let r = 3. - 2. * f64::to_radians(angle).tan();
        let expected = std::f64::consts::PI * 2. / 3. * (9. + 3. * r + r * r);
        assert!(
            (brep.signed_volume(tol()).unwrap() - expected).abs() < 1e-3,
            "height={height}, angle={angle}"
        );
        let bounds = brep.bounds();
        assert!((bounds.min().z() - height.min(0.)).abs() < 1e-5);
        assert!((bounds.max().z() - height.max(0.)).abs() < 1e-5);
    }
    assert_eq!(profile.to_nurbs().unwrap(), before);
}
#[test]
fn native_tapered_polygon_sharp_sides_and_open_profile() {
    let c = NurbsCurve::try_new(
        1,
        vec![
            p(-3., -3., 0.),
            p(3., -3., 0.),
            p(3., 3., 0.),
            p(-3., 3., 0.),
            p(-3., -3., 0.),
        ],
        vec![0., 0., 1., 2., 3., 4., 4.],
    )
    .unwrap();
    let profile = KernelCurve::from_nurbs_for_modeling(&c).unwrap();
    let shape = Shape::tapered_extrusion(&profile, v(0., 0., 2.), 10., false, true, tol()).unwrap();
    let brep = shape.to_brep(tol()).unwrap();
    assert!(brep.is_solid());
    let width = 6. - 4. * 10_f64.to_radians().tan();
    let expected = 2. / 3. * (36. + 6. * width + width * width);
    assert!((brep.signed_volume(tol()).unwrap() - expected).abs() < 1e-3);
    let line =
        NurbsCurve::try_new(1, vec![p(0., 0., 0.), p(4., 0., 0.)], vec![0., 0., 1., 1.]).unwrap();
    let line = KernelCurve::from_nurbs_for_modeling(&line).unwrap();
    let sheet = Shape::tapered_extrusion(&line, v(0., 0., 2.), 10., false, false, tol())
        .unwrap()
        .to_brep(tol())
        .unwrap();
    assert!(!sheet.is_solid());
    assert!(!sheet.faces().is_empty());
}

#[test]
fn native_tapered_round_polygon_matches_parallel_area_integral() {
    let source = NurbsCurve::try_new(
        1,
        vec![
            p(-3., -3., 0.),
            p(3., -3., 0.),
            p(3., 3., 0.),
            p(-3., 3., 0.),
            p(-3., -3., 0.),
        ],
        vec![0., 0., 1., 2., 3., 4., 4.],
    )
    .unwrap();
    let profile = KernelCurve::from_nurbs_for_modeling(&source).unwrap();
    let brep = Shape::tapered_extrusion(&profile, v(0., 0., 2.), -10., true, true, tol())
        .unwrap()
        .to_brep(tol())
        .unwrap();
    assert!(brep.is_solid());
    let k = 10_f64.to_radians().tan();
    let expected = 72. + 48. * k + std::f64::consts::PI * k * k * 8. / 3.;
    assert!((brep.signed_volume(tol()).unwrap() - expected).abs() < 1e-3);
    assert_eq!(profile.to_nurbs().unwrap(), source);
}
#[test]
fn native_tapered_declines_invalid_and_disconnected_profiles_without_reusing_sources() {
    let profile = circle();
    let before = profile.to_nurbs().unwrap();
    for (direction, angle) in [
        (v(0., 0., 0.), 10.),
        (v(1., 0., 2.), 10.),
        (v(0., 0., 2.), 90.),
        (v(0., 0., 2.), f64::NAN),
        (v(0., 0., 2.), f64::from_bits(1)),
    ] {
        assert!(Shape::tapered_extrusion(&profile, direction, angle, false, true, tol()).is_err());
        assert_eq!(profile.to_nurbs().unwrap(), before);
    }
    let broken = NurbsCurve::try_new(
        1,
        vec![p(0., 0., 0.), p(1., 0., 0.), p(2., 1., 0.), p(3., 1., 0.)],
        vec![0., 0., 1., 1., 2., 2.],
    )
    .unwrap();
    assert!(KernelCurve::from_nurbs_for_modeling(&broken).is_err());
    assert!(
        Shape::tapered_extrusion(&profile, v(0., 0., 2.), 10., false, true, tol())
            .unwrap()
            .to_brep(tol())
            .unwrap()
            .is_solid()
    );
}

#[test]
fn native_tapered_polygon_is_invariant_under_shifted_parameter_domains() {
    let polygon = NurbsCurve::try_new(
        1,
        vec![
            p(-3., -3., 0.),
            p(3., -3., 0.),
            p(3., 3., 0.),
            p(-3., 3., 0.),
            p(-3., -3., 0.),
        ],
        vec![0., 0., 1., 2., 3., 4., 4.],
    )
    .unwrap();
    let width = 6. - 4. * 10_f64.to_radians().tan();
    let expected = 2. / 3. * (36. + 6. * width + width * width);
    for domain in [2.0..=5.0, 10.0..=30.0, -100.0..=-80.0] {
        let source = polygon.try_reparameterized(domain).unwrap();
        let profile = KernelCurve::from_nurbs_for_modeling(&source).unwrap();
        let brep = Shape::tapered_extrusion(&profile, v(0., 0., 2.), 10., false, true, tol())
            .unwrap()
            .to_brep(tol())
            .unwrap();
        assert!(brep.is_solid());
        assert_eq!(brep.faces().len(), 6);
        assert!((brep.signed_volume(tol()).unwrap() - expected).abs() < 1e-3);
        assert_eq!(profile.to_nurbs().unwrap(), source);
    }
}
