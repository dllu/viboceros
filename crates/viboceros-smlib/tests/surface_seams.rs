#![cfg(feature = "native")]
use serde_json::Value;
use viboceros_geometry::{
    GeometryError, NurbsSurface, Point3, SurfaceKnotDirection, WeightedPoint3,
};
use viboceros_smlib::{Error, change_surface_seam};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/surface_change_seam.json"
    ))
    .unwrap()
}

fn source(op: &Value, chart: f64, gauge: f64) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        op["degree_u"].as_u64().unwrap() as usize,
        op["degree_v"].as_u64().unwrap() as usize,
        op["control_point_count_u"].as_u64().unwrap() as usize,
        op["control_point_count_v"].as_u64().unwrap() as usize,
        support::points(&op["control_points"])
            .into_iter()
            .map(|p| WeightedPoint3::try_new(p.point(), p.weight() * gauge).unwrap())
            .collect(),
        support::knots(&op["knots_u"])
            .into_iter()
            .map(|k| k * chart)
            .collect(),
        support::knots(&op["knots_v"])
            .into_iter()
            .map(|k| k * chart)
            .collect(),
    )
    .unwrap()
}

fn direction(op: &Value) -> SurfaceKnotDirection {
    match op["direction"].as_str().unwrap() {
        "u" => SurfaceKnotDirection::U,
        "v" => SurfaceKnotDirection::V,
        "both" => SurfaceKnotDirection::Both,
        _ => unreachable!(),
    }
}

#[test]
fn native_surface_seams_match_all_recorded_rhino_definitions() {
    let fixture = fixture();
    let reference: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/surface_change_seam.json"
    ))
    .unwrap();
    assert_eq!(reference["results"].as_array().unwrap().len(), 8);
    for (op, record) in fixture["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(reference["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], record["id"]);
        let actual = change_surface_seam(
            &source(op, 1., 1.),
            direction(op),
            op["parameter"][0].as_f64().unwrap(),
            op["parameter"][1].as_f64().unwrap(),
        )
        .unwrap();
        support::compare(
            &support::surface_value(&actual),
            &record["value"],
            op["id"].as_str().unwrap(),
        );
    }
}

fn compare(source: &NurbsSurface, direction: SurfaceKnotDirection, u: f64, v: f64) {
    let before = source.clone();
    let actual = change_surface_seam(source, direction, u, v).unwrap();
    let expected = source.try_change_closed_seam(direction, u, v).unwrap();
    assert_eq!(actual.degree_u(), expected.degree_u());
    assert_eq!(actual.degree_v(), expected.degree_v());
    assert_eq!(actual.knots_u(), expected.knots_u());
    assert_eq!(actual.knots_v(), expected.knots_v());
    assert_eq!(
        actual.control_point_count_u(),
        expected.control_point_count_u()
    );
    assert_eq!(
        actual.control_point_count_v(),
        expected.control_point_count_v()
    );
    assert_eq!(actual.is_periodic_u(), expected.is_periodic_u());
    assert_eq!(actual.is_periodic_v(), expected.is_periodic_v());
    for (a, e) in actual
        .control_points()
        .iter()
        .zip(expected.control_points())
    {
        assert!(a.point().distance_to(e.point()).unwrap() < 1e-10);
        assert!((a.weight() / e.weight() - 1.).abs() < 1e-12);
    }
    fn wrap(t: f64, domain: std::ops::RangeInclusive<f64>) -> f64 {
        if domain.contains(&t) {
            t
        } else {
            (t - domain.start()).rem_euclid(domain.end() - domain.start()) + domain.start()
        }
    }
    for i in 0..=8 {
        for j in 0..=8 {
            let u = actual.parameter_at_u(i as f64 / 8.).unwrap();
            let v = actual.parameter_at_v(j as f64 / 8.).unwrap();
            let a = actual.evaluate(u, v).unwrap();
            assert!(a.distance_to(expected.evaluate(u, v).unwrap()).unwrap() < 1e-10);
            assert!(
                a.distance_to(
                    source
                        .evaluate(wrap(u, source.domain_u()), wrap(v, source.domain_v()))
                        .unwrap()
                )
                .unwrap()
                    < 1e-10
            );
        }
    }
    assert_eq!(*source, before);
}

#[test]
fn native_surface_seams_preserve_tensor_definitions_charts_gauges_and_loci() {
    let fixture = fixture();
    assert_eq!(fixture["operations"].as_array().unwrap().len(), 8);
    for op in fixture["operations"].as_array().unwrap() {
        let direction = direction(op);
        for chart in [2_f64.powi(-360), 1., 2_f64.powi(360)] {
            for gauge in [1., 1e100, -1e100] {
                let source = source(op, chart, gauge);
                let (u, v) = (
                    op["parameter"][0].as_f64().unwrap() * chart,
                    op["parameter"][1].as_f64().unwrap() * chart,
                );
                compare(&source, direction, u, v);
                if direction == SurfaceKnotDirection::U {
                    compare(&source, direction, *source.domain_u().start(), f64::NAN);
                    compare(&source, direction, *source.domain_u().end(), f64::NAN);
                }
            }
        }
    }
}

#[test]
fn constant_pole_rows_use_whole_net_closure_and_preserve_the_unselected_basis() {
    let fixture = fixture();
    let op = &fixture["operations"][0];
    let mut points = support::points(&op["control_points"]);
    for p in &mut points[..7] {
        *p = WeightedPoint3::try_new(Point3::try_new(0., 0., 5.).unwrap(), 1.).unwrap();
    }
    let source = NurbsSurface::try_new_rational(
        3,
        1,
        7,
        2,
        points,
        support::knots(&op["knots_u"]),
        vec![-3., 0., 4., 8.],
    )
    .unwrap();
    for source in [
        source.clone(),
        source
            .try_make_non_periodic(SurfaceKnotDirection::U)
            .unwrap(),
    ] {
        for surface in [source.clone(), source.try_swapped_uv().unwrap()] {
            let direction = if surface.degree_u() == 3 {
                SurfaceKnotDirection::U
            } else {
                SurfaceKnotDirection::V
            };
            for parameter in [3.25, 3. + 1e-9] {
                compare(&surface, direction, parameter, parameter);
            }
        }
    }
}

#[test]
fn both_seam_orders_preserve_the_same_polynomial_and_rational_tensor() {
    let fixture = fixture();
    let polynomial = source(&fixture["operations"][3], 1., 1.);
    let rational = NurbsSurface::try_new_rational(
        2,
        2,
        5,
        5,
        polynomial
            .control_points()
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let (u, v) = (i % 5, i / 5);
                WeightedPoint3::try_new(p.point(), 1. + (u % 3) as f64 / 4. + (v % 3) as f64 / 5.)
                    .unwrap()
            })
            .collect(),
        polynomial.knots_u().to_vec(),
        polynomial.knots_v().to_vec(),
    )
    .unwrap();
    for source in [polynomial, rational] {
        let u_then_v = change_surface_seam(&source, SurfaceKnotDirection::Both, 1.25, 0.6).unwrap();
        let v_then_u = change_surface_seam(
            &source.try_swapped_uv().unwrap(),
            SurfaceKnotDirection::Both,
            0.6,
            1.25,
        )
        .unwrap()
        .try_swapped_uv()
        .unwrap();
        support::compare(
            &support::surface_value(&u_then_v),
            &support::surface_value(&v_then_u),
            "seam order",
        );
        compare(&source, SurfaceKnotDirection::Both, 1.25, 0.6);
    }
}

#[test]
fn native_seam_validation_preserves_open_tiny_gauge_and_invalid_parameter_policy() {
    let fixture = fixture();
    let surface = source(&fixture["operations"][0], 1., 1.);
    for parameter in [f64::NAN, 1., 7.] {
        assert!(matches!(
            change_surface_seam(&surface, SurfaceKnotDirection::U, parameter, f64::NAN),
            Err(Error::Geometry(_))
        ));
    }
    let tiny = source(&fixture["operations"][0], 1., 1e-300);
    assert!(matches!(
        change_surface_seam(&tiny, SurfaceKnotDirection::U, 3.25, f64::NAN),
        Err(Error::Geometry(
            GeometryError::SurfaceSeamDirectionMustBeClosed { direction: "U" }
        ))
    ));
    assert_eq!(
        change_surface_seam(&surface, SurfaceKnotDirection::U, 2., f64::NAN).unwrap(),
        surface
    );
    let open = NurbsSurface::try_bilinear([
        Point3::try_new(0., 0., 0.).unwrap(),
        Point3::try_new(2., 0., 0.).unwrap(),
        Point3::try_new(2., 2., 0.).unwrap(),
        Point3::try_new(0., 2., 0.).unwrap(),
    ])
    .unwrap();
    assert!(matches!(
        change_surface_seam(&open, SurfaceKnotDirection::U, 0.5, f64::NAN),
        Err(Error::Geometry(
            GeometryError::SurfaceSeamDirectionMustBeClosed { direction: "U" }
        ))
    ));
}

#[path = "support/nurbs.rs"]
#[allow(dead_code)]
mod support;
