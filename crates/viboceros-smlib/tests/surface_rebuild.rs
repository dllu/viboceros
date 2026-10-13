#![cfg(feature = "native")]
use serde_json::Value;
use viboceros_geometry::{
    Frame3, GeometryError, NurbsSurface, Point3, Tolerance, Vector3, WeightedPoint3,
    try_rebuild_nurbs_surface,
};
use viboceros_smlib::{Error, rebuild_surface};

fn surface(v: &Value) -> NurbsSurface {
    let degree = v
        .get("degree")
        .map(|d| {
            [
                d[0].as_u64().unwrap() as usize,
                d[1].as_u64().unwrap() as usize,
            ]
        })
        .unwrap_or_else(|| {
            [
                v["degree_u"].as_u64().unwrap() as usize,
                v["degree_v"].as_u64().unwrap() as usize,
            ]
        });
    let count = v
        .get("control_count")
        .map(|d| {
            [
                d[0].as_u64().unwrap() as usize,
                d[1].as_u64().unwrap() as usize,
            ]
        })
        .unwrap_or_else(|| {
            [
                v["control_point_count_u"].as_u64().unwrap() as usize,
                v["control_point_count_v"].as_u64().unwrap() as usize,
            ]
        });
    NurbsSurface::try_new_rational(
        degree[0],
        degree[1],
        count[0],
        count[1],
        support::points(&v["control_points"]),
        support::knots(&v["knots_u"]),
        support::knots(&v["knots_v"]),
    )
    .unwrap()
}

fn compare(a: &NurbsSurface, e: &NurbsSurface, epsilon: f64) {
    assert_eq!((a.degree_u(), a.degree_v()), (e.degree_u(), e.degree_v()));
    assert_eq!(a.knots_u(), e.knots_u());
    assert_eq!(a.knots_v(), e.knots_v());
    assert_eq!(
        (a.is_periodic_u(), a.is_periodic_v()),
        (e.is_periodic_u(), e.is_periodic_v())
    );
    assert_eq!(a.control_points().len(), e.control_points().len());
    for (a, e) in a.control_points().iter().zip(e.control_points()) {
        assert_eq!(a.weight(), e.weight());
        for (a, e) in a.point().to_array().into_iter().zip(e.point().to_array()) {
            assert!(
                (a - e).abs() <= epsilon + 1e-12 * a.abs().max(e.abs()),
                "{a} != {e}"
            );
        }
    }
    for j in 0..=16 {
        for i in 0..=16 {
            let u = a.parameter_at_u(i as f64 / 16.).unwrap();
            let v = a.parameter_at_v(j as f64 / 16.).unwrap();
            assert!(
                a.evaluate(u, v)
                    .unwrap()
                    .distance_to(e.evaluate(u, v).unwrap())
                    .unwrap()
                    <= epsilon * 4.
            );
        }
    }
}

fn bilinear() -> NurbsSurface {
    NurbsSurface::try_bilinear([
        Point3::try_new(0., 0., 0.).unwrap(),
        Point3::try_new(4., 0., 0.).unwrap(),
        Point3::try_new(4., 6., 1.).unwrap(),
        Point3::try_new(0., 6., 0.).unwrap(),
    ])
    .unwrap()
}

#[test]
fn native_surface_rebuild_matches_retained_sdk_and_command_control_nets() {
    let mut cases = 0;
    for text in [
        include_str!("../../../tools/rhino_oracle/observations/surface_rebuild.json"),
        include_str!("../../../tools/rhino_oracle/observations/surface_rebuild_closed.json"),
        include_str!(
            "../../../tools/rhino_oracle/observations/surface_rebuild_closed_degrees.json"
        ),
    ] {
        let d: Value = serde_json::from_str(text).unwrap();
        for row in d["results"].as_array().unwrap() {
            let v = &row["value"];
            let s = surface(&v["before"][0]["definition"]);
            let before = s.clone();
            let count = serde_json::from_value(v["spec"]["count"].clone()).unwrap();
            let degree = serde_json::from_value(v["spec"]["degree"].clone()).unwrap();
            let actual = rebuild_surface(&s, count, degree).unwrap();
            compare(&actual, &surface(&v["rebuild_sdk"]), 1e-6);
            compare(
                &actual,
                &try_rebuild_nurbs_surface(&s, count, degree).unwrap(),
                1e-9,
            );
            assert_eq!(s, before);
            cases += 1;
        }
    }
    assert_eq!(cases, 40);
}

#[test]
fn native_surface_rebuild_covers_all_degrees_open_closed_swapped_axes_and_exact_poles() {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let sources = [
        bilinear(),
        NurbsSurface::try_sphere(frame, 2.).unwrap(),
        NurbsSurface::try_cylinder(frame, 2., 0., 4.).unwrap(),
        NurbsSurface::try_cone(frame, 2., 4.).unwrap(),
        NurbsSurface::try_torus(frame, 4., 1.).unwrap(),
    ];
    for s in sources {
        for swap in [false, true] {
            let s = if swap {
                s.try_swapped_uv().unwrap()
            } else {
                s.clone()
            };
            let before = s.clone();
            for degree in 1..=11 {
                for count in [[12, 13], [31, 32]] {
                    let a = rebuild_surface(&s, count, [degree, degree]).unwrap();
                    compare(
                        &a,
                        &try_rebuild_nurbs_surface(&s, count, [degree, degree]).unwrap(),
                        1e-9,
                    );
                    for direction in [
                        viboceros_geometry::SurfaceKnotDirection::U,
                        viboceros_geometry::SurfaceKnotDirection::V,
                    ] {
                        let domain = if direction == viboceros_geometry::SurfaceKnotDirection::U {
                            s.domain_v()
                        } else {
                            s.domain_u()
                        };
                        for t in [*domain.start(), *domain.end()] {
                            let c = viboceros_smlib::surface_isocurve(&s, direction, t).unwrap();
                            let pole = c.control_points()[0].point();
                            if c.control_points().iter().all(|p| p.point() == pole) {
                                let d = if direction == viboceros_geometry::SurfaceKnotDirection::U
                                {
                                    a.domain_v()
                                } else {
                                    a.domain_u()
                                };
                                let t = if t == *domain.start() {
                                    *d.start()
                                } else {
                                    *d.end()
                                };
                                let c =
                                    viboceros_smlib::surface_isocurve(&a, direction, t).unwrap();
                                assert!(c.control_points().iter().all(|p| p.point() == pole));
                            }
                        }
                    }
                }
            }
            assert_eq!(s, before);
        }
    }
}

#[test]
fn maximum_tensor_rebuild_retains_independent_bilinear_greville_controls() {
    for count in [[12, 13], [256, 256]] {
        let a = rebuild_surface(&bilinear(), count, [11, 11]).unwrap();
        assert_eq!(a.control_points().len(), count[0] * count[1]);
        for j in 0..count[1] {
            for i in 0..count[0] {
                let u =
                    a.knots_u()[i + 1..=i + 11].iter().sum::<f64>() / 11. / (count[0] - 11) as f64;
                let v =
                    a.knots_v()[j + 1..=j + 11].iter().sum::<f64>() / 11. / (count[1] - 11) as f64;
                let p = a.control_points()[j * count[0] + i];
                assert_eq!(p.weight(), 1.);
                assert_eq!(p.point().y(), a.control_points()[j * count[0]].point().y());
                for (a, e) in p
                    .point()
                    .to_array()
                    .into_iter()
                    .zip([4. * u, 6. * v, u * v])
                {
                    assert!((a - e).abs() < 1e-10);
                }
            }
        }
    }
}

#[test]
fn native_tensor_rebuild_retains_tiny_channels_common_gauges_and_typed_limits() {
    for tiny in [1e-18, 1e-180, 1e-280] {
        for gauge in [1e-300, 1., 1e300] {
            let base = bilinear();
            let s = NurbsSurface::try_new_rational(
                1,
                1,
                2,
                2,
                base.control_points()
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let p = p.point();
                        WeightedPoint3::try_new(
                            Point3::try_new(p.x(), p.y(), p.z() * tiny).unwrap(),
                            gauge * (1. + i as f64 / 4.),
                        )
                        .unwrap()
                    })
                    .collect(),
                base.knots_u().to_vec(),
                base.knots_v().to_vec(),
            )
            .unwrap();
            let a = rebuild_surface(&s, [12, 13], [3, 4]).unwrap();
            let e = try_rebuild_nurbs_surface(&s, [12, 13], [3, 4]).unwrap();
            for (a, e) in a.control_points().iter().zip(e.control_points()) {
                assert!((a.point().z() / tiny - e.point().z() / tiny).abs() < 1e-9);
            }
        }
    }
    assert!(matches!(
        rebuild_surface(&bilinear(), [257, 4], [3, 2]),
        Err(Error::Geometry(GeometryError::InvalidSurfaceRebuild { .. }))
    ));
    assert!(matches!(
        rebuild_surface(&bilinear(), [19, 4], [18, 2]),
        Err(Error::Kernel(_))
    ));
}

#[path = "support/nurbs.rs"]
#[allow(dead_code)]
mod support;
