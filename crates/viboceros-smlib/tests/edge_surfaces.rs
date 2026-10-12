#![cfg(feature = "native")]
use serde_json::Value;
use viboceros_geometry::{
    NurbsCurve, NurbsSurface, Point3, Tolerance, WeightedPoint3, arrange_edge_surface_curves,
};
use viboceros_smlib::{KernelCurve, Shape};

fn source(value: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        value["degree"].as_u64().unwrap() as usize,
        value["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|control| {
                let p = &control["point"];
                WeightedPoint3::try_new(
                    Point3::try_new(
                        p[0].as_f64().unwrap(),
                        p[1].as_f64().unwrap(),
                        p[2].as_f64().unwrap(),
                    )
                    .unwrap(),
                    control["weight"].as_f64().unwrap(),
                )
                .unwrap()
            })
            .collect(),
        value["knots"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k.as_f64().unwrap())
            .collect(),
    )
    .unwrap()
}

fn sample_error(surface: &NurbsSurface, record: &Value) -> f64 {
    let mut maximum = 0.0_f64;
    for j in 0..=12 {
        for i in 0..=12 {
            let expected = &record["value"]["samples"][0][j * 13 + i];
            let actual = surface
                .evaluate(
                    surface.parameter_at_u(i as f64 / 12.).unwrap(),
                    surface.parameter_at_v(j as f64 / 12.).unwrap(),
                )
                .unwrap();
            for (axis, value) in actual.to_array().into_iter().enumerate() {
                let expected = expected[axis].as_f64().unwrap();
                let error = (value - expected).abs();
                assert!(
                    error <= 2e-12 + 1e-14 * value.abs().max(expected.abs()),
                    "{} at {i},{j}: {error}",
                    record["id"]
                );
                maximum = maximum.max(error);
            }
        }
    }
    maximum
}

#[test]
fn native_edge_producers_replay_every_recorded_rhino_boundary_family() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/edge_surface.json"
    ))
    .unwrap();
    let reference: Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tools/rhino_oracle/observations/edge_surface.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let t = Tolerance::DEFAULT;
    let mut failures = Vec::new();
    for (operation, record) in request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(reference["results"].as_array().unwrap())
    {
        assert_eq!(operation["id"], record["id"]);
        let input = operation["curves"]
            .as_array()
            .unwrap()
            .iter()
            .map(source)
            .collect::<Vec<_>>();
        let arranged = arrange_edge_surface_curves(&input, t).unwrap();
        let handles = arranged
            .iter()
            .map(KernelCurve::from_nurbs_for_modeling)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let before = handles
            .iter()
            .map(|curve| curve.to_nurbs().unwrap())
            .collect::<Vec<_>>();
        let result = KernelCurve::edge_surface(&handles.iter().collect::<Vec<_>>(), t);
        match result {
            Ok(surface) => {
                sample_error(&surface, record);
                match Shape::edge_surface(&handles.iter().collect::<Vec<_>>(), t)
                    .and_then(|shape| shape.to_brep(t))
                {
                    Ok(brep) => {
                        assert_eq!(brep.faces().len(), 1);
                        assert!(!brep.is_solid());
                        sample_error(brep.faces()[0].surface(), record);
                    }
                    Err(error) => failures.push(format!("{}: topology {error}", operation["id"])),
                }
            }
            Err(error) => failures.push(format!("{}: {error}", operation["id"])),
        }
        assert_eq!(
            handles
                .iter()
                .map(|curve| curve.to_nurbs().unwrap())
                .collect::<Vec<_>>(),
            before
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn ruled_sheets_retain_short_spans_and_extreme_parameter_charts() {
    for scale in [2_f64.powi(-360), 1., 2_f64.powi(360)] {
        let close = 0.5 + 1e-14;
        let a = NurbsCurve::try_new_rational(
            2,
            [
                [2., 0., 0.],
                [3., 0., 0.],
                [4., 0., 0.],
                [4., 0., 10.],
                [5., 0., 0.],
                [6., 0., 0.],
                [7., 0., 0.],
            ]
            .into_iter()
            .zip([1., 0.75, 1., 0.5, 1., 0.75, 1.])
            .map(|(p, w)| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w).unwrap())
            .collect(),
            [0., 0., 0., 0.5, 0.5, close, close, 1., 1., 1.]
                .map(|k| k * scale)
                .to_vec(),
        )
        .unwrap();
        let b = a
            .transformed(viboceros_geometry::AffineTransform3::from_translation(
                viboceros_geometry::Vector3::try_new(0., 5., 0.).unwrap(),
            ))
            .unwrap();
        let handles = [
            KernelCurve::from_nurbs_for_modeling(&a).unwrap(),
            KernelCurve::from_nurbs_for_modeling(&b).unwrap(),
        ];
        let result =
            KernelCurve::edge_surface(&handles.iter().collect::<Vec<_>>(), Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(result.knots_v(), a.knots());
        for v in [0., 0.25, 0.5, (0.5 + close) * 0.5, close, 0.75, 1.] {
            let p = a.evaluate(v * scale).unwrap();
            for u in [0., 0.25, 0.5, 1.] {
                let expected = Point3::try_new(p.x(), p.y() + 5. * u, p.z()).unwrap();
                assert!(
                    result
                        .evaluate(u, v * scale)
                        .unwrap()
                        .distance_to(expected)
                        .unwrap()
                        < 1e-10
                );
            }
        }
    }
}

#[test]
fn ruled_surface_matches_homogeneous_interpolation_and_preserves_sources() {
    let a = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(Point3::try_new(0., 0., 0.).unwrap(), 1.).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(2., 1., 2.).unwrap(), 0.5).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(4., 0., 0.).unwrap(), 1.).unwrap(),
        ],
        vec![2., 2., 2., 7., 7., 7.],
    )
    .unwrap();
    let b = a
        .transformed(viboceros_geometry::AffineTransform3::from_translation(
            viboceros_geometry::Vector3::try_new(0., 0., 5.).unwrap(),
        ))
        .unwrap();
    let handles = [
        KernelCurve::from_nurbs_for_modeling(&a).unwrap(),
        KernelCurve::from_nurbs_for_modeling(&b).unwrap(),
    ];
    let surface: NurbsSurface =
        KernelCurve::edge_surface(&handles.iter().collect::<Vec<_>>(), Tolerance::DEFAULT).unwrap();
    assert_eq!(surface.domain_v(), a.domain());
    for i in 0..=24 {
        let v = a.parameter_at(i as f64 / 24.).unwrap();
        let p = a.evaluate(v).unwrap();
        for j in 0..=16 {
            let u = j as f64 / 16.;
            let expected = Point3::try_new(p.x(), p.y(), p.z() + 5. * u).unwrap();
            assert!(
                surface
                    .evaluate(u, v)
                    .unwrap()
                    .distance_to(expected)
                    .unwrap()
                    < 1e-11
            );
        }
    }
}
