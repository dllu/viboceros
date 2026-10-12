#![cfg(feature = "native")]
use serde_json::Value;
use viboceros_geometry::{
    Frame3, GeometryError, NurbsSurface, Point3, SurfaceKnotDirection, Tolerance, Vector3,
    WeightedPoint3,
};
use viboceros_smlib::{Error, surface_isocurve};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/surface_isocurve_geometry.json"
    ))
    .unwrap()
}
fn source(s: &Value) -> NurbsSurface {
    let mut source = NurbsSurface::try_new_rational(
        s["degree_u"].as_u64().unwrap() as usize,
        s["degree_v"].as_u64().unwrap() as usize,
        s["control_point_count_u"].as_u64().unwrap() as usize,
        s["control_point_count_v"].as_u64().unwrap() as usize,
        support::points(&s["control_points"]),
        support::knots(&s["knots_u"]),
        support::knots(&s["knots_v"]),
    )
    .unwrap();
    if let Some(u) = s.get("domain_u") {
        let v = &s["domain_v"];
        source = source
            .try_reparameterized(
                u[0].as_f64().unwrap()..=u[1].as_f64().unwrap(),
                v[0].as_f64().unwrap()..=v[1].as_f64().unwrap(),
            )
            .unwrap();
    }
    source
}
fn compare(source: &NurbsSurface, direction: SurfaceKnotDirection, parameter: f64) {
    let before = source.clone();
    let actual = surface_isocurve(source, direction, parameter).unwrap();
    let expected = if direction == SurfaceKnotDirection::U {
        source.isocurve_u(parameter).unwrap()
    } else {
        source.isocurve_v(parameter).unwrap()
    };
    assert_eq!(actual.degree(), expected.degree());
    assert_eq!(actual.knots(), expected.knots());
    assert_eq!(
        actual.control_points().len(),
        expected.control_points().len()
    );
    for (a, e) in actual
        .control_points()
        .iter()
        .zip(expected.control_points())
    {
        assert!(a.point().distance_to(e.point()).unwrap() < 1e-10);
        assert!((a.weight() / e.weight() - 1.).abs() < 1e-12);
    }
    for i in 0..=64 {
        let t = actual.parameter_at(i as f64 / 64.).unwrap();
        let p = if direction == SurfaceKnotDirection::U {
            source.evaluate(t, parameter).unwrap()
        } else {
            source.evaluate(parameter, t).unwrap()
        };
        assert!(actual.evaluate(t).unwrap().distance_to(p).unwrap() < 1e-10);
        assert!(
            actual
                .evaluate(t)
                .unwrap()
                .distance_to(expected.evaluate(t).unwrap())
                .unwrap()
                < 1e-10
        );
    }
    assert_eq!(*source, before);
}

#[test]
fn native_isocurves_preserve_complete_bases_loci_charts_signed_gauges_and_sources() {
    let fixture = fixture();
    assert_eq!(fixture["operations"].as_array().unwrap().len(), 15);
    for op in fixture["operations"].as_array().unwrap() {
        let original = source(&op["surface"]);
        let direction = if op["direction"] == "u" {
            SurfaceKnotDirection::U
        } else {
            SurfaceKnotDirection::V
        };
        for chart in [2_f64.powi(-360), 1., 2_f64.powi(360)] {
            for gauge in [1e-300, 1., 1e300, -1e300] {
                let source = NurbsSurface::try_new_rational(
                    original.degree_u(),
                    original.degree_v(),
                    original.control_point_count_u(),
                    original.control_point_count_v(),
                    original
                        .control_points()
                        .iter()
                        .map(|p| WeightedPoint3::try_new(p.point(), p.weight() * gauge).unwrap())
                        .collect(),
                    original.knots_u().iter().map(|k| k * chart).collect(),
                    original.knots_v().iter().map(|k| k * chart).collect(),
                )
                .unwrap();
                compare(
                    &source,
                    direction,
                    op["parameter"].as_f64().unwrap() * chart,
                );
            }
        }
    }
}

#[test]
fn full_order_fixed_knots_and_inactive_boundaries_copy_the_correct_one_sided_row_exactly() {
    for knots in [
        vec![0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1.],
        vec![-2., 0., 0., 0., 0.5, 1., 2., 3.],
        vec![-3., -2., -1., -0.5, 0., 0., 0., 2.],
    ] {
        let n = knots.len() - 3;
        let source = NurbsSurface::try_new_rational(
            1,
            2,
            2,
            n,
            (0..n)
                .flat_map(|j| {
                    (0..2).map(move |i| {
                        WeightedPoint3::try_new(
                            Point3::try_new(i as f64, j as f64, 0.).unwrap(),
                            0.7 + j as f64 / 4.,
                        )
                        .unwrap()
                    })
                })
                .collect(),
            vec![0., 0., 1., 1.],
            knots,
        )
        .unwrap();
        for source in [source.clone(), source.try_swapped_uv().unwrap()] {
            let direction = if source.degree_v() == 2 {
                SurfaceKnotDirection::U
            } else {
                SurfaceKnotDirection::V
            };
            let domain = if direction == SurfaceKnotDirection::U {
                source.domain_v()
            } else {
                source.domain_u()
            };
            for t in [
                *domain.start(),
                0.5_f64.clamp(*domain.start(), *domain.end()),
                *domain.end(),
            ] {
                compare(&source, direction, t);
            }
        }
    }
}

#[test]
fn subnormal_fixed_parameters_and_knots_keep_a_reversible_chart() {
    let tiny = f64::from_bits(1);
    for start in [0., tiny] {
        let source = NurbsSurface::try_new(
            1,
            1,
            2,
            2,
            vec![
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(1., 0., 0.).unwrap(),
                Point3::try_new(0., 10., 0.).unwrap(),
                Point3::try_new(1., 10., 0.).unwrap(),
            ],
            vec![0., 0., 1., 1.],
            vec![start, start, 10., 10.],
        )
        .unwrap();
        for t in [tiny * 2., 5.] {
            compare(&source, SurfaceKnotDirection::U, t);
        }
    }
}

#[test]
fn clamped_spherical_poles_retain_exact_payload_and_invalid_requests_fail_before_native_math() {
    let frame = Frame3::try_from_normal(
        Point3::try_new(3., -0.031, 5.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let source = NurbsSurface::try_sphere(frame, 0.8).unwrap();
    for (v, row) in [
        (*source.domain_v().start(), 0),
        (*source.domain_v().end(), source.control_point_count_v() - 1),
    ] {
        let actual = surface_isocurve(&source, SurfaceKnotDirection::U, v).unwrap();
        assert_eq!(
            actual.control_points(),
            &source.control_points()
                [row * source.control_point_count_u()..(row + 1) * source.control_point_count_u()]
        );
    }
    for parameter in [f64::NAN, -2., 2.] {
        assert!(matches!(
            surface_isocurve(&source, SurfaceKnotDirection::U, parameter),
            Err(Error::Geometry(_))
        ));
    }
    assert!(matches!(
        surface_isocurve(&source, SurfaceKnotDirection::Both, 0.),
        Err(Error::Geometry(GeometryError::InvalidControlNet { .. }))
    ));
}

#[path = "support/nurbs.rs"]
#[allow(dead_code)]
mod support;
