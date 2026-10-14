#![cfg(feature = "native")]
use serde_json::Value;
use viboceros_geometry::{
    NurbsSurface, Point3, SurfaceExtensionEdge as E, SurfaceKnotDirection as D, WeightedPoint3,
};
fn source(op: &Value) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        op["degree_u"].as_u64().unwrap() as usize,
        op["degree_v"].as_u64().unwrap() as usize,
        op["control_point_count_u"].as_u64().unwrap() as usize,
        op["control_point_count_v"].as_u64().unwrap() as usize,
        op["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                WeightedPoint3::try_new(
                    Point3::try_from(
                        serde_json::from_value::<[f64; 3]>(p["point"].clone()).unwrap(),
                    )
                    .unwrap(),
                    p["weight"].as_f64().unwrap_or(1.),
                )
                .unwrap()
            })
            .collect(),
        serde_json::from_value(op["knots_u"].clone()).unwrap(),
        serde_json::from_value(op["knots_v"].clone()).unwrap(),
    )
    .unwrap()
}
fn compare(a: &NurbsSurface, b: &NurbsSurface) {
    assert_eq!(a.knots_u(), b.knots_u());
    assert_eq!(a.knots_v(), b.knots_v());
    for i in 0..=16 {
        for j in 0..=16 {
            let u = a.parameter_at_u(i as f64 / 16.).unwrap();
            let v = a.parameter_at_v(j as f64 / 16.).unwrap();
            let p = a.evaluate(u, v).unwrap();
            let q = b.evaluate(u, v).unwrap();
            assert!(p.distance_to(q).unwrap() < 1e-9, "u={u} v={v}");
        }
    }
}
#[test]
fn native_surface_extension_preserves_rational_domains_other_axes_and_distance_policy() {
    for fixture in [
        include_str!("../../../tools/rhino_oracle/fixtures/surface_extend.json"),
        include_str!("../../../tools/rhino_oracle/fixtures/surface_extend_length.json"),
    ] {
        let request: Value = serde_json::from_str(fixture).unwrap();
        for op in request["operations"].as_array().unwrap() {
            let source = source(op);
            let before = source.clone();
            let (actual, expected) = if op.get("domain").is_some() {
                let interval =
                    op["domain"][0].as_f64().unwrap()..=op["domain"][1].as_f64().unwrap();
                let direction = if op["direction"] == "u" { D::U } else { D::V };
                let expected = if direction == D::U {
                    source.try_extended_u(interval.clone())
                } else {
                    source.try_extended_v(interval.clone())
                }
                .unwrap();
                (
                    viboceros_smlib::extend_surface(&source, direction, interval).unwrap(),
                    expected,
                )
            } else {
                let edge = match op["edge"].as_str().unwrap() {
                    "west" => E::West,
                    "east" => E::East,
                    "south" => E::South,
                    "north" => E::North,
                    _ => unreachable!(),
                };
                let length = op["length"].as_f64().unwrap();
                (
                    viboceros_smlib::extend_surface_by_length(&source, edge, length).unwrap(),
                    source.try_extended_by_length(edge, length).unwrap(),
                )
            };
            compare(&actual, &expected);
            assert_eq!(source, before);
        }
    }
}

#[test]
fn native_surface_extension_allows_collapsed_pole_control_rows() {
    let source = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        [[0., 0., 0.], [0., 0., 0.], [0., 0., 1.], [1., 0., 1.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    for (source, axis) in [
        (source.clone(), D::U),
        (source.try_swapped_uv().unwrap(), D::V),
    ] {
        let actual = viboceros_smlib::extend_surface(&source, axis, -0.5..=1.5).unwrap();
        for i in 0..=16 {
            for j in 0..=16 {
                let u = actual.parameter_at_u(i as f64 / 16.).unwrap();
                let v = actual.parameter_at_v(j as f64 / 16.).unwrap();
                let expected =
                    Point3::try_new(u * v, 0., if axis == D::U { v } else { u }).unwrap();
                assert!(
                    actual
                        .evaluate(u, v)
                        .unwrap()
                        .distance_to(expected)
                        .unwrap()
                        < 1e-12
                );
            }
        }
    }
}

#[test]
fn native_surface_extension_accepts_degree_eighteen_in_the_untouched_axis() {
    let surface = NurbsSurface::try_new(
        2,
        18,
        3,
        19,
        (0..19)
            .flat_map(|v| {
                (0..3).map(move |u| Point3::try_new(u as f64, v as f64, (u * u) as f64).unwrap())
            })
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
        std::iter::repeat_n(0., 19)
            .chain(std::iter::repeat_n(1., 19))
            .collect(),
    )
    .unwrap();
    let before = surface.clone();
    let actual = viboceros_smlib::extend_surface(&surface, D::U, -0.25..=1.25).unwrap();
    assert_eq!(actual.degree_v(), 18);
    assert_eq!(actual.knots_v(), surface.knots_v());
    for i in 0..=16 {
        for j in 0..=16 {
            let u = -0.25 + 1.5 * i as f64 / 16.;
            let v = j as f64 / 16.;
            let expected = Point3::try_new(2. * u, 18. * v, 2. * u + 2. * u * u).unwrap();
            assert!(
                actual
                    .evaluate(u, v)
                    .unwrap()
                    .distance_to(expected)
                    .unwrap()
                    < 1e-12
            );
        }
    }
    assert_eq!(surface, before);
}
