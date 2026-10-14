#![cfg(feature = "native")]
use viboceros_geometry::{
    CurveSurfaceIntersectionEvent, NurbsCurve, NurbsSurface, Point3, Tolerance, WeightedPoint3,
};
use viboceros_smlib::curve_surface_intersection_events;
fn p(x: [f64; 3]) -> Point3 {
    Point3::try_from(x).unwrap()
}
fn rotate(p: [f64; 3], axis: usize) -> [f64; 3] {
    match axis {
        1 => [p[2], p[0], p[1]],
        2 => [p[1], p[2], p[0]],
        _ => p,
    }
}
#[test]
fn contacts_and_finite_ranges_preserve_charts_planes_signed_gauges_and_original_images() {
    let tolerance = Tolerance::try_new(1e-9, 1e-12, 1e-10).unwrap();
    for kind in 0..9 {
        for charts in [[0., 1., 0., 1., 0., 1.], [-4., 7., 10., 16., -8., 3.]] {
            for axis in 0..3 {
                for gauge in [1., 1e-280, -1e280] {
                    let coordinates = match kind {
                        0 | 7 | 8 => vec![[0.5, 0.5, -1.], [0.5, 0.5, 1.]],
                        1 => vec![[0., 0.5, 1.], [0.5, 0.5, -1.], [1., 0.5, 1.]],
                        2 => vec![[-1., 0.5, 0.], [2., 0.5, 0.]],
                        3 => vec![[0.25, 0.5, 0.], [0.75, 0.5, 0.]],
                        4 => vec![[0., 0., 0.], [1., 0., 0.]],
                        5 => vec![[0., 0.5, 1.], [1., 0.5, 1.]],
                        _ => vec![[2., 0.5, -1.], [2., 0.5, 1.]],
                    };
                    let degree = coordinates.len() - 1;
                    let source = NurbsCurve::try_new_rational(
                        degree,
                        coordinates
                            .into_iter()
                            .map(|x| WeightedPoint3::try_new(p(rotate(x, axis)), gauge).unwrap())
                            .collect(),
                        [vec![charts[0]; degree + 1], vec![charts[1]; degree + 1]].concat(),
                    )
                    .unwrap();
                    let controls = (0..2)
                        .flat_map(|v| {
                            (0..2).map(move |u| {
                                let z = if kind >= 7 { u * v } else { 0 };
                                WeightedPoint3::try_new(
                                    p(rotate([u as f64, v as f64, z as f64], axis)),
                                    gauge * if kind == 8 && u == 1 { 2. } else { 1. },
                                )
                                .unwrap()
                            })
                        })
                        .collect();
                    let surface = NurbsSurface::try_new_rational(
                        1,
                        1,
                        2,
                        2,
                        controls,
                        vec![charts[2], charts[2], charts[3], charts[3]],
                        vec![charts[4], charts[4], charts[5], charts[5]],
                    )
                    .unwrap();
                    let before = (source.clone(), surface.clone());
                    let events =
                        curve_surface_intersection_events(&source, &surface, tolerance).unwrap();
                    assert_eq!(
                        events.len(),
                        usize::from(kind != 5 && kind != 6),
                        "kind {kind}, axis {axis}, charts {charts:?}, gauge {gauge}"
                    );
                    for event in events {
                        let hits = match event {
                            CurveSurfaceIntersectionEvent::Point(point) => {
                                assert!(!(2..7).contains(&kind));
                                vec![point]
                            }
                            CurveSurfaceIntersectionEvent::Overlap(overlap) => {
                                assert!((2..=4).contains(&kind));
                                vec![overlap.start(), overlap.end()]
                            }
                        };
                        for (end, hit) in hits.into_iter().enumerate() {
                            let fraction = if kind >= 7 {
                                0.625
                            } else if kind < 2 {
                                0.5
                            } else if kind == 2 {
                                if end == 0 { 1. / 3. } else { 2. / 3. }
                            } else {
                                end as f64
                            };
                            assert!(
                                (hit.curve_parameter() - source.parameter_at(fraction).unwrap())
                                    .abs()
                                    < 1e-7
                            );
                            let q = surface.evaluate(hit.u(), hit.v()).unwrap();
                            let r = source.evaluate(hit.curve_parameter()).unwrap();
                            assert!(q.distance_to(r).unwrap() <= tolerance.absolute());
                            assert!(q.distance_to(hit.point()).unwrap() <= tolerance.absolute());
                            if kind >= 7 {
                                let expected_u = if kind == 8 { 1. / 3. } else { 0.5 };
                                assert!(
                                    (hit.u() - surface.parameter_at_u(expected_u).unwrap()).abs()
                                        < 1e-8
                                );
                                assert!(
                                    (hit.v() - surface.parameter_at_v(0.5).unwrap()).abs() < 1e-8
                                );
                            }
                        }
                    }
                    assert_eq!((source, surface), before);
                }
            }
        }
    }
}

#[test]
fn unclamped_surface_active_domains_and_extreme_charts_keep_original_parameters() {
    let tolerance = Tolerance::try_new(1e-9, 1e-12, 1e-10).unwrap();
    for scale in [2_f64.powi(-360), 1., 2_f64.powi(360)] {
        let surface = NurbsSurface::try_new(
            1,
            1,
            2,
            2,
            vec![
                p([0., 0., 0.]),
                p([1., 0., 0.]),
                p([0., 1., 0.]),
                p([1., 1., 0.]),
            ],
            vec![0., scale, 2. * scale, 3. * scale],
            vec![0., scale, 2. * scale, 3. * scale],
        )
        .unwrap();
        let source = NurbsCurve::try_new(
            1,
            vec![p([0.5, 0.5, -1.]), p([0.5, 0.5, 1.])],
            vec![-scale, -scale, scale, scale],
        )
        .unwrap();
        let before = (source.clone(), surface.clone());
        let events = curve_surface_intersection_events(&source, &surface, tolerance).unwrap();
        let [CurveSurfaceIntersectionEvent::Point(hit)] = events.as_slice() else {
            panic!("expected one point")
        };
        assert!((hit.curve_parameter() / scale).abs() < 1e-8);
        assert!((hit.u() / scale - 1.5).abs() < 1e-8 && (hit.v() / scale - 1.5).abs() < 1e-8);
        assert!(hit.point().distance_to(p([0.5, 0.5, 0.])).unwrap() < 1e-9);
        assert_eq!((source, surface), before);
    }
}

#[test]
fn trimmed_face_holes_and_view_projection_reuse_original_uv_containment() {
    use viboceros_geometry::{AffineTransform3, Brep, CurveBrepIntersectionEvent, Plane, Vector3};
    let tolerance = Tolerance::DEFAULT;
    let polygon = |lo: f64, hi: f64| {
        NurbsCurve::try_new(
            1,
            vec![
                p([lo, lo, 0.]),
                p([hi, lo, 0.]),
                p([hi, hi, 0.]),
                p([lo, hi, 0.]),
                p([lo, lo, 0.]),
            ],
            vec![0., 0., 1., 2., 3., 4., 4.],
        )
        .unwrap()
    };
    let outer = polygon(0., 10.);
    let hole = polygon(4., 6.);
    let brep = Brep::try_planar_face_with_holes(&outer, &[hole], tolerance).unwrap();
    let projection = AffineTransform3::try_planar_projection(Plane::new(
        p([0., 0., 0.]),
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized(tolerance)
            .unwrap(),
    ))
    .unwrap();
    for (height, transform) in [(0., None), (1., Some(projection))] {
        let source = NurbsCurve::try_new(
            1,
            vec![p([-1., 5., height]), p([11., 5., height])],
            vec![0., 0., 12., 12.],
        )
        .unwrap();
        let before = (source.clone(), brep.clone());
        let events =
            viboceros_smlib::curve_brep_intersection_events(&source, &brep, transform, tolerance)
                .unwrap();
        assert_eq!(events.len(), 2);
        for (event, expected) in events.iter().zip([[1., 5.], [7., 11.]]) {
            let CurveBrepIntersectionEvent::Overlap(overlap) = event else {
                panic!("expected overlap")
            };
            assert!((overlap.start().curve_parameter() - expected[0]).abs() < 1e-8);
            assert!((overlap.end().curve_parameter() - expected[1]).abs() < 1e-8);
        }
        assert_eq!((&source, &brep), (&before.0, &before.1));
    }
}

#[test]
fn analytic_cylinder_tangency_and_sphere_seams_and_poles_remain_point_contacts() {
    use viboceros_geometry::{Frame3, Vector3};
    let tolerance = Tolerance::try_new(1e-9, 1e-12, 1e-10).unwrap();
    let frame = Frame3::try_from_normal(
        p([0., 0., 0.]),
        Vector3::try_new(0., 0., 1.).unwrap(),
        tolerance,
    )
    .unwrap();
    let cylinder = NurbsSurface::try_cylinder(frame, 2., -2., 2.).unwrap();
    let sphere = NurbsSurface::try_sphere(frame, 1.).unwrap();
    for (surface, ends, expected) in [
        (&cylinder, [[2., -3., 0.], [2., 3., 0.]], vec![[2., 0., 0.]]),
        (
            &sphere,
            [[-2., 0., 0.], [2., 0., 0.]],
            vec![[-1., 0., 0.], [1., 0., 0.]],
        ),
        (
            &sphere,
            [[0., 0., -2.], [0., 0., 2.]],
            vec![[0., 0., -1.], [0., 0., 1.]],
        ),
    ] {
        let source =
            NurbsCurve::try_new(1, ends.into_iter().map(p).collect(), vec![0., 0., 1., 1.])
                .unwrap();
        let events = curve_surface_intersection_events(&source, surface, tolerance).unwrap();
        assert_eq!(events.len(), expected.len(), "{events:?}");
        for (event, expected) in events.iter().zip(expected) {
            let CurveSurfaceIntersectionEvent::Point(point) = event else {
                panic!("expected analytic point contact: {events:?}")
            };
            assert!(point.point().distance_to(p(expected)).unwrap() < 1e-8);
        }
    }
}

#[test]
fn unsupported_degree_and_collapsed_surface_frames_keep_portable_results() {
    let source = NurbsCurve::try_new(
        1,
        vec![p([0.5, 0.5, -1.]), p([0.5, 0.5, 1.])],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let high_degree = NurbsSurface::try_new(
        18,
        1,
        19,
        2,
        (0..2)
            .flat_map(|v| (0..=18).map(move |u| p([u as f64 / 18., v as f64, 0.])))
            .collect(),
        [vec![0.; 19], vec![1.; 19]].concat(),
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert_eq!(
        curve_surface_intersection_events(&source, &high_degree, Tolerance::DEFAULT).unwrap(),
        viboceros_geometry::curve_surface_intersection_events(
            &source,
            &high_degree,
            Tolerance::DEFAULT
        )
        .unwrap()
    );
    let collapsed = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![
            p([0., 0.5, 0.]),
            p([1., 0.5, 0.]),
            p([0., 0.5, 0.]),
            p([1., 0.5, 0.]),
        ],
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert_eq!(
        curve_surface_intersection_events(&source, &collapsed, Tolerance::DEFAULT).unwrap(),
        viboceros_geometry::curve_surface_intersection_events(
            &source,
            &collapsed,
            Tolerance::DEFAULT
        )
        .unwrap()
    );
}
