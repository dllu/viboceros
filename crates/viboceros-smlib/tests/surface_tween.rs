#![cfg(feature = "native")]
use serde_json::Value;
use viboceros_geometry::{
    NurbsSurface, Point3, WeightedPoint3, try_tween_nurbs_surfaces,
    try_tween_nurbs_surfaces_refitted, try_tween_nurbs_surfaces_sampled,
};
use viboceros_smlib::{
    interpolate_open_point_grid, surface_tweens, surface_tweens_refitted, surface_tweens_sampled,
};

fn surface(v: &Value) -> NurbsSurface {
    let d = &v["degree"];
    let c = &v["control_count"];
    NurbsSurface::try_new_rational(
        d[0].as_u64().unwrap() as usize,
        d[1].as_u64().unwrap() as usize,
        c[0].as_u64().unwrap() as usize,
        c[1].as_u64().unwrap() as usize,
        support::points(&v["control_points"]),
        support::knots(&v["knots_u"]),
        support::knots(&v["knots_v"]),
    )
    .unwrap()
}
fn compare(a: &NurbsSurface, b: &NurbsSurface, epsilon: f64) {
    assert_eq!((a.degree_u(), a.degree_v()), (b.degree_u(), b.degree_v()));
    assert_eq!(a.knots_u().len(), b.knots_u().len());
    assert_eq!(a.knots_v().len(), b.knots_v().len());
    for (a, b) in a
        .knots_u()
        .iter()
        .chain(a.knots_v())
        .zip(b.knots_u().iter().chain(b.knots_v()))
    {
        assert!((a - b).abs() < 1e-8);
    }
    assert_eq!(a.control_points().len(), b.control_points().len());
    for (a, b) in a.control_points().iter().zip(b.control_points()) {
        assert!((a.weight() - b.weight()).abs() < 1e-12);
        for (a, b) in a.point().to_array().into_iter().zip(b.point().to_array()) {
            assert!(
                (a - b).abs() < epsilon + 1e-12 * a.abs().max(b.abs()),
                "{a} != {b}"
            );
        }
    }
    for j in 0..=8 {
        for i in 0..=8 {
            let u = a.parameter_at_u(i as f64 / 8.).unwrap();
            let v = a.parameter_at_v(j as f64 / 8.).unwrap();
            assert!(
                a.evaluate(u, v)
                    .unwrap()
                    .distance_to(
                        b.evaluate(
                            b.parameter_at_u(i as f64 / 8.).unwrap(),
                            b.parameter_at_v(j as f64 / 8.).unwrap()
                        )
                        .unwrap()
                    )
                    .unwrap()
                    < epsilon * 4.
            );
        }
    }
}

#[test]
fn native_tweens_replay_retained_control_refit_and_sampling_nets_without_source_mutation() {
    let mut cases = 0;
    for text in [
        include_str!("../../../tools/rhino_oracle/observations/tween_surfaces_command.json"),
        include_str!(
            "../../../tools/rhino_oracle/observations/tween_surfaces_control_initial.json"
        ),
        include_str!(
            "../../../tools/rhino_oracle/observations/tween_surfaces_control_followup.json"
        ),
        include_str!("../../../tools/rhino_oracle/observations/tween_surfaces_refit.json"),
        include_str!("../../../tools/rhino_oracle/observations/tween_surfaces_sampling.json"),
    ] {
        let d: Value = serde_json::from_str(text).unwrap();
        for row in d["results"].as_array().unwrap() {
            let v = &row["value"];
            let a = surface(&v["before"][0]["definition"]);
            let b = surface(&v["before"][1]["definition"]);
            let before = (a.clone(), b.clone());
            let n = v["spec"]["number"].as_u64().unwrap() as usize;
            let sample = v["spec"]["sample"].as_u64().unwrap() as usize;
            let (actual, portable) = match v["spec"]["method"].as_str().unwrap() {
                "None" => (
                    surface_tweens(&a, &b, n).unwrap(),
                    try_tween_nurbs_surfaces(&a, &b, n).unwrap(),
                ),
                "Refit" => (
                    surface_tweens_refitted(&a, &b, n).unwrap(),
                    try_tween_nurbs_surfaces_refitted(&a, &b, n).unwrap(),
                ),
                "SamplePoints" => (
                    surface_tweens_sampled(&a, &b, n, sample).unwrap(),
                    try_tween_nurbs_surfaces_sampled(&a, &b, n, sample).unwrap(),
                ),
                _ => unreachable!(),
            };
            let records = v["command"]["after_script"].as_array().unwrap();
            assert_eq!(actual.len(), n);
            // Early option-change captures include extra source-shaped copies.
            // Requested tweens are the final n objects; raw counts remain in
            // the qualification report rather than being claimed as parity.
            for ((a, p), r) in actual
                .iter()
                .zip(&portable)
                .zip(&records[records.len() - n..])
            {
                assert_eq!(a.knots_u(), p.knots_u());
                assert_eq!(a.knots_v(), p.knots_v());
                compare(a, p, 1e-8);
                compare(a, &surface(&r["definition"]), 1e-6);
            }
            assert_eq!((a, b), before);
            cases += 1;
        }
    }
    assert_eq!(cases, 104);
}

#[test]
fn native_open_grids_cover_all_degrees_nonuniform_chords_and_independent_grid_stations() {
    let points = (0..7)
        .flat_map(|j| {
            (0..13)
                .map(move |i| Point3::try_new(i as f64, j as f64, (i * j) as f64 * 0.125).unwrap())
        })
        .collect::<Vec<_>>();
    for degree in 0..=13 {
        let a = interpolate_open_point_grid(&points, [13, 7], [degree; 2]).unwrap();
        let p = NurbsSurface::try_through_point_grid(&points, [13, 7], [degree; 2], [false; 2])
            .unwrap();
        compare(&a, &p, 1e-8);
    }
    let points = (0..256)
        .flat_map(|j| {
            (0..256)
                .map(move |i| Point3::try_new(i as f64, j as f64, (i * j) as f64 * 0.001).unwrap())
        })
        .collect::<Vec<_>>();
    let a = interpolate_open_point_grid(&points, [256, 256], [3; 2]).unwrap();
    assert_eq!(a.control_points().len(), points.len());
    assert!(a.control_points().iter().all(|p| p.weight() == 1.));
    for j in [0, 17, 128, 255] {
        for i in [0, 17, 128, 255] {
            let p = a
                .evaluate(
                    *a.domain_u().end() * i as f64 / 255.,
                    *a.domain_v().end() * j as f64 / 255.,
                )
                .unwrap();
            assert!(p.distance_to(points[j * 256 + i]).unwrap() < 1e-8);
        }
    }
}

#[test]
fn native_refit_retains_full_order_degree_elevation_and_distinct_adjacent_knots() {
    let a = NurbsSurface::try_new(
        2,
        1,
        6,
        2,
        (0..2)
            .flat_map(|j| {
                [0., 1., 2., 4., 5., 6.]
                    .into_iter()
                    .map(move |x| Point3::try_new(x, j as f64, 0.).unwrap())
            })
            .collect(),
        vec![0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let b = NurbsSurface::try_bilinear([
        Point3::try_new(0., 0., 2.).unwrap(),
        Point3::try_new(6., 0., 2.).unwrap(),
        Point3::try_new(6., 1., 2.).unwrap(),
        Point3::try_new(0., 1., 2.).unwrap(),
    ])
    .unwrap()
    .try_change_degree(3, 1, false)
    .unwrap();
    let before = (a.clone(), b.clone());
    assert!(try_tween_nurbs_surfaces_refitted(&a, &b, 2).is_err());
    assert!(surface_tweens_refitted(&a, &b, 2).is_err());
    assert_eq!((a, b), before);
    let near = |knot| {
        NurbsSurface::try_new(
            1,
            1,
            3,
            2,
            (0..2)
                .flat_map(|j| (0..3).map(move |i| Point3::try_new(i as f64, j as f64, 0.).unwrap()))
                .collect(),
            vec![0., 0., knot, 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap()
    };
    let (a, b) = (near(0.3), near(0.30000000000000004));
    let actual = surface_tweens_refitted(&a, &b, 1).unwrap();
    let p = try_tween_nurbs_surfaces_refitted(&a, &b, 1).unwrap();
    compare(&actual[0], &p[0], 1e-9);
    assert_eq!(
        actual[0].knots_u(),
        &[0., 0., 0.3, 0.30000000000000004, 1., 1.]
    );
}

#[test]
fn native_tween_sampling_keeps_tiny_channels_and_rejects_invalid_budgets() {
    let plane = |z| {
        NurbsSurface::try_bilinear([
            Point3::try_new(0., 0., z).unwrap(),
            Point3::try_new(4., 0., z).unwrap(),
            Point3::try_new(4., 6., z).unwrap(),
            Point3::try_new(0., 6., z).unwrap(),
        ])
        .unwrap()
    };
    for tiny in [1e-18, 1e-180, 1e-280] {
        let a = plane(0.);
        let b = plane(tiny);
        let result = surface_tweens_sampled(&a, &b, 3, 4).unwrap();
        for (i, s) in result.iter().enumerate() {
            for p in s.control_points() {
                assert!((p.point().z() / tiny - (i + 1) as f64 / 4.).abs() < 1e-12);
            }
        }
    }
    let a = plane(0.);
    assert!(surface_tweens_sampled(&a, &a, 16, 255).is_err());
    assert!(surface_tweens(&a, &a, 0).is_err());
    let bad = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        a.control_points()
            .iter()
            .map(|p| WeightedPoint3::try_new(p.point(), -1.).unwrap())
            .collect(),
        a.knots_u().to_vec(),
        a.knots_v().to_vec(),
    )
    .unwrap();
    assert!(surface_tweens(&bad, &a, 1).is_err());
}

#[path = "support/nurbs.rs"]
#[allow(dead_code)]
mod support;
