#![cfg(feature = "native")]
use viboceros_geometry::{Brep, Circle3, NurbsSurface, Point3, Tolerance, UnitVector3, Vector3};
use viboceros_smlib::Shape;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn v(z: f64) -> Vector3 {
    Vector3::try_new(0., 0., z).unwrap()
}
fn tol() -> Tolerance {
    Tolerance::try_new(1e-5, 1e-12, 1e-8).unwrap()
}
fn patch(bump: bool) -> Brep {
    let points = (0..3)
        .flat_map(|j| {
            (0..3).map(move |i| {
                p(
                    i as f64 * 3. - 3.,
                    j as f64 * 3. - 3.,
                    if bump && i == 1 && j == 1 { 1. } else { 0. },
                )
            })
        })
        .collect();
    Brep::try_surface_face(
        NurbsSurface::try_new(
            2,
            2,
            3,
            3,
            points,
            vec![0., 0., 0., 1., 1., 1.],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap(),
        tol(),
    )
    .unwrap()
}
#[test]
fn native_surface_taper_retains_curved_base_and_has_analytic_frustum_volume() {
    let k = 10_f64.to_radians().tan();
    for bump in [false, true] {
        let source = Shape::from_brep(&patch(bump), tol()).unwrap();
        let before = source.to_brep(tol()).unwrap();
        for height in [2., -2.] {
            let b = source
                .tapered_face(v(height), 10., false, true, tol())
                .unwrap()
                .to_brep(tol())
                .unwrap();
            assert!(b.is_solid());
            let expected =
                72. - 48. * k + 32. / 3. * k * k + if bump { -height.signum() * 4. } else { 0. };
            assert!(
                (b.signed_volume(tol()).unwrap().abs() - expected).abs() < 1e-3,
                "bump={bump} h={height}"
            );
            for (u, v) in [(0.2, 0.3), (0.5, 0.5), (0.8, 0.7)] {
                let point = before.faces()[0].surface().evaluate(u, v).unwrap();
                assert!(b.faces().iter().any(|f| {
                    f.surface()
                        .closest_parameters(point, tol())
                        .is_ok_and(|(a, b)| {
                            f.surface()
                                .evaluate(a, b)
                                .unwrap()
                                .distance_to(point)
                                .unwrap()
                                < 1e-5
                        })
                }));
            }
        }
        assert_eq!(source.to_brep(tol()).unwrap(), before);
    }
}
#[test]
fn native_surface_taper_holes_expand_for_positive_draft_and_contract_for_negative() {
    let normal = UnitVector3::try_new(0., 0., 1., tol()).unwrap();
    let circle = |r| {
        Circle3::try_new(p(0., 0., 0.), r, normal, tol())
            .unwrap()
            .to_nurbs()
            .unwrap()
    };
    let source = Shape::from_brep(
        &Brep::try_planar_face_with_holes(&circle(3.), &[circle(1.)], tol()).unwrap(),
        tol(),
    )
    .unwrap();
    let before = source.to_brep(tol()).unwrap();
    for angle in [10_f64, -10., 0.] {
        let b = source
            .tapered_face(v(2.), angle, false, true, tol())
            .unwrap()
            .to_brep(tol())
            .unwrap();
        assert!(b.is_solid());
        for radius in [3. - angle.to_radians().tan(), 1. + angle.to_radians().tan()] {
            let point = p(radius, 0., 1.);
            assert!(b.faces().iter().any(|f| {
                f.surface()
                    .closest_parameters(point, tol())
                    .is_ok_and(|(u, v)| {
                        f.surface()
                            .evaluate(u, v)
                            .unwrap()
                            .distance_to(point)
                            .unwrap()
                            < 1e-5
                    })
            }));
        }
        let expected = 16. * std::f64::consts::PI * (1. - angle.to_radians().tan());
        assert!(
            (b.signed_volume(tol()).unwrap().abs() - expected).abs() < 1e-3,
            "angle={angle}"
        );
    }
    let walls = source
        .tapered_face(v(2.), 10., false, false, tol())
        .unwrap()
        .to_brep(tol())
        .unwrap();
    assert!(!walls.is_solid());
    assert_eq!(source.to_brep(tol()).unwrap(), before);
}
#[test]
fn native_surface_taper_round_outward_corners_and_zero_draft() {
    let source = Shape::from_brep(&patch(false), tol()).unwrap();
    let k = 10_f64.to_radians().tan();
    let b = source
        .tapered_face(v(2.), -10., true, true, tol())
        .unwrap()
        .to_brep(tol())
        .unwrap();
    assert!(b.is_solid());
    let expected = 72. + 48. * k + 8. * std::f64::consts::PI / 3. * k * k;
    assert!((b.signed_volume(tol()).unwrap().abs() - expected).abs() < 1e-3);
    let b = source
        .tapered_face(v(2.), 0., false, true, tol())
        .unwrap()
        .to_brep(tol())
        .unwrap();
    assert!((b.signed_volume(tol()).unwrap().abs() - 72.).abs() < 1e-4);
}
#[test]
fn native_surface_taper_invalid_input_declines_without_editing_source() {
    let source = Shape::from_brep(&patch(false), tol()).unwrap();
    let before = source.to_brep(tol()).unwrap();
    for angle in [90., f64::NAN] {
        assert!(
            source
                .tapered_face(v(2.), angle, false, true, tol())
                .is_err()
        );
    }
    assert!(source.tapered_face(v(0.), 10., false, true, tol()).is_err());
    assert!(
        source
            .tapered_face(
                Vector3::try_new(1., 0., 2.).unwrap(),
                10.,
                false,
                true,
                tol()
            )
            .is_err()
    );
    assert_eq!(source.to_brep(tol()).unwrap(), before);
}

#[test]
fn native_surface_taper_nonplanar_boundary_walls_and_uncappable_top() {
    let surface = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![p(0., 0., 0.), p(6., 0., 0.), p(0., 6., 0.), p(6., 6., 1.)],
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let source = Shape::from_brep(&Brep::try_surface_face(surface, tol()).unwrap(), tol()).unwrap();
    let before = source.to_brep(tol()).unwrap();
    let walls = source
        .tapered_face(v(2.), 10., false, false, tol())
        .unwrap()
        .to_brep(tol())
        .unwrap();
    assert!(!walls.is_solid());
    assert!(!walls.faces().is_empty());
    assert!(source.tapered_face(v(2.), 10., false, true, tol()).is_err());
    let linear = source
        .tapered_face(v(2.), 0., false, true, tol())
        .unwrap()
        .to_brep(tol())
        .unwrap();
    assert!(linear.is_solid());
    assert!((linear.signed_volume(tol()).unwrap().abs() - 72.).abs() < 1e-4);
    assert_eq!(source.to_brep(tol()).unwrap(), before);
}
