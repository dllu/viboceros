#![cfg(feature = "native")]
use viboceros_geometry::{NurbsSurface, Point3, SurfaceSurfaceIntersectionEvent, Tolerance};
use viboceros_smlib::surface_intersection_events;
#[path = "surface_intersections/trim.rs"]
mod trim;
fn patch(points: [[f64; 3]; 4]) -> NurbsSurface {
    NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        points
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}
#[test]
fn native_general_surface_contact_curves_have_continuous_source_correspondence() {
    let tolerance = Tolerance::try_new(1e-8, 1e-12, 1e-10).unwrap();
    for kind in 0..6 {
        let mut a = patch([[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [1., 1., 0.]]);
        let b = match kind {
            0 => patch([[0.5, 0., -1.], [0.5, 1., -1.], [0.5, 0., 1.], [0.5, 1., 1.]]),
            1 => patch([[0., 0., 1.], [1., 0., 1.], [0., 1., 1.], [1., 1., 1.]]),
            2 => patch([
                [0.25, 0.25, 0.],
                [1.25, 0.25, 0.],
                [0.25, 1.25, 0.],
                [1.25, 1.25, 0.],
            ]),
            3 => patch([[1., 1., 0.], [2., 1., 0.], [1., 2., 0.], [2., 2., 0.]]),
            4 => patch([
                [0., 0., 0.25],
                [1., 0., -0.25],
                [0., 1., -0.25],
                [1., 1., 0.25],
            ]),
            _ => {
                a = patch([
                    [0., 0., 0.25],
                    [1., 0., 0.25],
                    [0., 1., 0.25],
                    [1., 1., 0.25],
                ]);
                patch([[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [1., 1., 1.]])
            }
        };
        let before = (a.clone(), b.clone());
        let events = surface_intersection_events(&a, &b, tolerance).unwrap();
        assert_eq!(
            events.len(),
            match kind {
                1 | 3 => 0,
                2 => 4,
                4 => 4,
                _ => 1,
            },
            "kind {kind}: {events:?}"
        );
        for event in &events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("expected curve")
            };
            for station in 0..=32 {
                let t = curve.parameter_at(station as f64 / 32.).unwrap();
                let point = curve.evaluate(t).unwrap();
                for surface in [&a, &b] {
                    let (u, v) = surface.closest_parameters(point, tolerance).unwrap();
                    assert!(surface.evaluate(u, v).unwrap().distance_to(point).unwrap() < 1e-7);
                }
            }
        }
        assert_eq!((a, b), before);
    }
}

#[test]
fn general_polynomial_surface_pairs_extend_the_unimplemented_dispatcher() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let controls = (0..4)
        .flat_map(|v| {
            (0..3).map(move |u| {
                Point3::try_new(
                    u as f64 / 2.,
                    v as f64 / 3.,
                    f64::from(u == 2) + f64::from(v == 3),
                )
                .unwrap()
            })
        })
        .collect();
    let curved = NurbsSurface::try_new(
        2,
        3,
        3,
        4,
        controls,
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let other = patch([[0., 0., 1.], [1., 0., 1.], [0., 1., 1.], [1., 1., 0.]]);
    assert!(matches!(
        viboceros_geometry::surface_surface_intersection_events(&curved, &other, tolerance),
        Err(viboceros_geometry::GeometryError::UnsupportedSurfaceSurfaceIntersection { .. })
    ));
    let events =
        viboceros_smlib::modeling_surface_intersection_events(&curved, &other, tolerance).unwrap();
    assert!(!events.is_empty());
    for event in events {
        let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
            panic!("expected intersection curve")
        };
        for station in 0..=64 {
            let point = curve
                .evaluate(curve.parameter_at(station as f64 / 64.).unwrap())
                .unwrap();
            let residual = point.x() * point.x() + point.y().powi(3) + point.x() * point.y() - 1.;
            assert!(residual.abs() < 2e-6, "{point:?}: {residual}");
        }
    }
}

#[test]
fn general_face_pairs_clip_partial_domains_and_preserve_operands() {
    use viboceros_geometry::{Brep, BrepBrepIntersectionEvent, SurfaceBrepIntersectionEvent};
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let controls = (0..4)
        .flat_map(|v| {
            (0..3).map(move |u| {
                Point3::try_new(
                    u as f64 / 2.,
                    v as f64 / 3.,
                    f64::from(u == 2) + f64::from(v == 3),
                )
                .unwrap()
            })
        })
        .collect();
    let curved = NurbsSurface::try_new(
        2,
        3,
        3,
        4,
        controls,
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let other = patch([[0., 0., 1.], [1., 0., 1.], [0., 1., 1.], [1., 1., 0.]]);
    let face = Brep::try_rectangular_surface_face(other.clone(), 0.25..=0.75, 0. ..=1., tolerance)
        .unwrap();
    let first =
        Brep::try_rectangular_surface_face(curved.clone(), 0. ..=1., 0. ..=1., tolerance).unwrap();
    let before = (first.clone(), face.clone());
    let events =
        viboceros_smlib::surface_brep_intersection_events(&curved, &face, tolerance).unwrap();
    let curves = events
        .iter()
        .map(|event| {
            let SurfaceBrepIntersectionEvent::Curve(curve) = event else {
                panic!("expected curve");
            };
            curve
        })
        .collect::<Vec<_>>();
    assert_eq!(curves.len(), 1);
    let brep_events = viboceros_smlib::brep_intersection_events(&first, &face, tolerance).unwrap();
    for curve in curves.into_iter().chain(brep_events.iter().map(|event| {
        let BrepBrepIntersectionEvent::Curve(curve) = event else {
            panic!("expected curve");
        };
        curve
    })) {
        for station in 0..=32 {
            let p = curve
                .evaluate(curve.parameter_at(station as f64 / 32.).unwrap())
                .unwrap();
            assert!(p.x() >= 0.25 - 1e-6 && p.x() <= 0.75 + 1e-6, "{p:?}");
            assert!((p.x() * p.x() + p.y().powi(3) + p.x() * p.y() - 1.).abs() < 2e-6);
        }
        let ends = [
            curve.evaluate(*curve.domain().start()).unwrap().x(),
            curve.evaluate(*curve.domain().end()).unwrap().x(),
        ];
        assert!((ends[0].min(ends[1]) - 0.25).abs() < 1e-6);
        assert!((ends[0].max(ends[1]) - 0.75).abs() < 1e-6);
    }
    let full = Brep::try_surface_face(other, tolerance).unwrap();
    let events =
        viboceros_smlib::surface_brep_intersection_events(&curved, &full, tolerance).unwrap();
    assert!(!events.is_empty());
    assert!(
        events
            .iter()
            .all(|event| matches!(event, SurfaceBrepIntersectionEvent::Curve(_)))
    );
    let events = viboceros_smlib::brep_intersection_events(&first, &full, tolerance).unwrap();
    assert!(!events.is_empty());
    assert!(
        events
            .iter()
            .all(|event| matches!(event, BrepBrepIntersectionEvent::Curve(_)))
    );
    assert_eq!((first, face), before);
}

#[test]
fn native_general_contacts_restore_independent_charts_rotations_and_signed_gauges() {
    use viboceros_geometry::WeightedPoint3;
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let curved = NurbsSurface::try_new(
        2,
        3,
        3,
        4,
        (0..4)
            .flat_map(|v| {
                (0..3).map(move |u| {
                    Point3::try_new(
                        u as f64 / 2.,
                        v as f64 / 3.,
                        f64::from(u == 2) + f64::from(v == 3),
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let other = patch([[0., 0., 1.], [1., 0., 1.], [0., 1., 1.], [1., 1., 0.]]);
    for chart in 0..2 {
        for plane in 0..3 {
            for gauge in [1., 2_f64.powi(300), -2_f64.powi(-300)] {
                let prepare = |surface: &NurbsSurface, second: bool| {
                    let points = surface
                        .control_points()
                        .iter()
                        .map(|p| {
                            let [x, y, z] = p.point().to_array();
                            let q = match plane {
                                1 => [z, x, y],
                                2 => [y, z, x],
                                _ => [x, y, z],
                            };
                            WeightedPoint3::try_new(
                                Point3::try_new(q[0] + 10., q[1] + 20., q[2] + 30.).unwrap(),
                                p.weight() * gauge,
                            )
                            .unwrap()
                        })
                        .collect();
                    let result = NurbsSurface::try_new_rational(
                        surface.degree_u(),
                        surface.degree_v(),
                        surface.control_point_count_u(),
                        surface.control_point_count_v(),
                        points,
                        surface.knots_u().to_vec(),
                        surface.knots_v().to_vec(),
                    )
                    .unwrap();
                    if chart == 0 {
                        result
                    } else if second {
                        result.try_reparameterized(30. ..=90., -8. ..=3.).unwrap()
                    } else {
                        result.try_reparameterized(-4. ..=7., 10. ..=16.).unwrap()
                    }
                };
                let a = prepare(&curved, false);
                let b = prepare(&other, true);
                let before = (a.clone(), b.clone());
                let events = surface_intersection_events(&a, &b, tolerance).unwrap();
                assert!(!events.is_empty());
                for event in events {
                    let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                        panic!("expected curve");
                    };
                    for station in 0..=16 {
                        let p = curve
                            .evaluate(curve.parameter_at(station as f64 / 16.).unwrap())
                            .unwrap();
                        let q = [p.x() - 10., p.y() - 20., p.z() - 30.];
                        let [x, y, z] = match plane {
                            1 => [q[1], q[2], q[0]],
                            2 => [q[2], q[0], q[1]],
                            _ => q,
                        };
                        assert!((x * x + y.powi(3) + x * y - 1.).abs() < 2e-6);
                        assert!((z - x * x - y.powi(3)).abs() < 2e-6);
                    }
                }
                assert_eq!((a, b), before);
            }
        }
    }
}
