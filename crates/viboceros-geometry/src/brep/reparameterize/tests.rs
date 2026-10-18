use super::*;
fn tolerance() -> Tolerance {
    Tolerance::try_new(1e-8, 1e-12, 1e-8).unwrap()
}
fn frame() -> Frame3 {
    Frame3::try_from_x_and_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        tolerance(),
    )
    .unwrap()
}
#[test]
fn face_charts_preserve_shared_spatial_geometry_and_orientations() {
    let tolerance = tolerance();
    let b = Brep::try_box(frame(), [[0., 2.], [0., 3.], [0., 4.]], tolerance).unwrap();
    let domains = [-2. ..=7., 10. ..=30.];
    let out = b
        .try_reparameterized_face_uv(
            [
                (0, domains.clone()),
                (3, [1e12..=1e12 + 8., -1e-12..=2e-12]),
            ],
            tolerance,
        )
        .unwrap();
    assert_eq!(b.vertices(), out.vertices());
    assert_eq!(b.edges(), out.edges());
    for (i, (a, e)) in out.faces().iter().zip(b.faces()).enumerate() {
        assert_eq!(a.is_reversed(), e.is_reversed());
        if i != 0 && i != 3 {
            assert_eq!(a, e);
        }
        for (al, el) in a.loops().iter().zip(e.loops()) {
            for (at, et) in al.trims().iter().zip(el.trims()) {
                assert_eq!(at.vertices(), et.vertices());
                assert_eq!(at.edge(), et.edge());
                assert_eq!(at.is_reversed_3d(), et.is_reversed_3d());
                assert_eq!(at.trim_type(), et.trim_type());
                assert_eq!(at.iso(), et.iso());
                assert_eq!(at.curve().knots(), et.curve().knots());
            }
        }
    }
    assert_eq!(out.faces()[0].surface().domain_u(), domains[0]);
    assert_eq!(out.faces()[0].surface().domain_v(), domains[1]);
    assert!((b.area(tolerance).unwrap() - out.area(tolerance).unwrap()).abs() < 1e-9);
    assert!(out.is_solid());
    assert!(out.is_manifold());
}
#[test]
fn invalid_or_duplicate_face_domains_leave_the_source_unchanged() {
    let t = tolerance();
    let b = Brep::try_box(frame(), [[0., 1.]; 3], t).unwrap();
    let before = b.clone();
    for changes in [
        vec![(99, [0. ..=1., 0. ..=1.])],
        vec![(0, [1. ..=0., 0. ..=1.])],
        vec![(0, [0. ..=Real::INFINITY, 0. ..=1.])],
        vec![(0, [0. ..=2., 0. ..=1.]), (0, [0. ..=3., 0. ..=1.])],
    ] {
        assert!(b.try_reparameterized_face_uv(changes, t).is_err());
        assert_eq!(b, before);
    }
    assert_eq!(b.try_reparameterized_face_uv([], t).unwrap(), b);
}

fn annulus(t: Tolerance) -> Brep {
    let p = Point3::try_new(0., 0., 0.).unwrap();
    let n = UnitVector3::try_new(0., 0., 1., t).unwrap();
    let outer = crate::Circle3::try_new(p, 5., n, t)
        .unwrap()
        .to_nurbs()
        .unwrap();
    let inner = crate::Circle3::try_new(p, 2., n, t)
        .unwrap()
        .to_nurbs()
        .unwrap();
    Brep::try_planar_face_with_holes(&outer, &[inner], t).unwrap()
}
#[test]
fn curved_holes_seams_poles_and_rational_supports_keep_their_images() {
    let t = tolerance();
    let cylinder =
        Brep::try_surface_face(NurbsSurface::try_cylinder(frame(), 2., -3., 5.).unwrap(), t)
            .unwrap();
    let sphere = Brep::try_surface_face(NurbsSurface::try_sphere(frame(), 2.).unwrap(), t).unwrap();
    let warped = Brep::try_surface_face(
        NurbsSurface::try_new_rational(
            2,
            1,
            3,
            2,
            [
                [0., 0., 0.],
                [2., 3., 1.],
                [5., 0., 2.],
                [0., 4., 1.],
                [2., 7., 3.],
                [5., 4., 4.],
            ]
            .into_iter()
            .enumerate()
            .map(|(i, p)| {
                WeightedPoint3::try_new(
                    Point3::try_from(p).unwrap(),
                    if i % 3 == 1 { 0.5 } else { 1. },
                )
                .unwrap()
            })
            .collect(),
            vec![-4., -4., -4., 2., 2., 2.],
            vec![3., 3., 9., 9.],
        )
        .unwrap(),
        t,
    )
    .unwrap();
    for source in [annulus(t), cylinder, sphere, warped] {
        for mut source in [source.clone(), source.reversed()] {
            for trim in source
                .faces
                .iter_mut()
                .flat_map(|f| &mut f.loops)
                .flat_map(|l| &mut l.trims)
            {
                trim.tolerance = [1e-12, 2e-12];
            }
            for domains in [[-2. ..=8., 10. ..=30.], [2e-8..=7e-8, -8e3..=-3e3]] {
                let target = source
                    .try_reparameterized_face_uv([(0, domains.clone())], t)
                    .unwrap();
                assert_eq!(source.vertices(), target.vertices());
                assert_eq!(source.edges(), target.edges());
                let a = &source.faces()[0];
                let b = &target.faces()[0];
                assert_eq!(a.is_reversed(), b.is_reversed());
                for (al, bl) in a.loops().iter().zip(b.loops()) {
                    assert_eq!(al.loop_type(), bl.loop_type());
                    for (at, bt) in al.trims().iter().zip(bl.trims()) {
                        assert_eq!(at.vertices(), bt.vertices());
                        assert_eq!(at.edge(), bt.edge());
                        assert_eq!(at.trim_type(), bt.trim_type());
                        assert_eq!(at.iso(), bt.iso());
                        assert_eq!(at.is_reversed_3d(), bt.is_reversed_3d());
                        assert_eq!(at.curve().knots(), bt.curve().knots());
                        assert_eq!(
                            at.curve()
                                .control_points()
                                .iter()
                                .map(|p| p.weight())
                                .collect::<Vec<_>>(),
                            bt.curve()
                                .control_points()
                                .iter()
                                .map(|p| p.weight())
                                .collect::<Vec<_>>()
                        );
                        for (i, old) in [a.surface().domain_u(), a.surface().domain_v()]
                            .iter()
                            .enumerate()
                        {
                            let expected = at.tolerance()[i]
                                * ((domains[i].end() - domains[i].start())
                                    / (old.end() - old.start()));
                            assert!((bt.tolerance()[i] - expected).abs() <= expected * 1e-14);
                        }
                        for i in 0..=8 {
                            let parameter = at.curve().parameter_at(i as f64 / 8.).unwrap();
                            let ap = at.curve().evaluate(parameter).unwrap();
                            let bp = bt.curve().evaluate(parameter).unwrap();
                            assert!(
                                a.surface()
                                    .evaluate(ap.x(), ap.y())
                                    .unwrap()
                                    .distance_to(b.surface().evaluate(bp.x(), bp.y()).unwrap())
                                    .unwrap()
                                    < 1e-8
                            );
                        }
                    }
                }
                for i in 1..8 {
                    for j in 1..8 {
                        let (u, v) = (i as f64 / 8., j as f64 / 8.);
                        let au = a.surface().parameter_at_u(u).unwrap();
                        let av = a.surface().parameter_at_v(v).unwrap();
                        let bu = b.surface().parameter_at_u(u).unwrap();
                        let bv = b.surface().parameter_at_v(v).unwrap();
                        assert_eq!(
                            a.contains_parameters(au, av, t).unwrap(),
                            b.contains_parameters(bu, bv, t).unwrap()
                        );
                        assert!(
                            a.surface()
                                .evaluate(au, av)
                                .unwrap()
                                .distance_to(b.surface().evaluate(bu, bv).unwrap())
                                .unwrap()
                                < 1e-8
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn insufficient_chart_precision_is_rejected_without_changing_geometry_or_tolerances() {
    let t = tolerance();
    let source = annulus(t);
    let before = source.clone();
    assert!(
        source
            .try_reparameterized_face_uv([(0, [1e16..=1e16 + 2., 0. ..=1.])], t)
            .is_err()
    );
    assert_eq!(source, before);
}
