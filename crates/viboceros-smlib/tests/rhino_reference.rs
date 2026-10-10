#![cfg(feature = "native")]
use viboceros_geometry::{NurbsCurve, Point3, WeightedPoint3};
use viboceros_smlib::{BooleanOperation, KernelCurve, Solid};

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn capture() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../docs/smlib-bridge-native-reference.json"
    ))
    .unwrap()
}

#[test]
fn native_solid_references_match_volume_bounds_and_manifold_status() {
    let mut qualified = 0;
    for row in capture()["results"].as_array().unwrap() {
        let case = row["id"].as_str().unwrap();
        if case.starts_with("rational_") {
            continue;
        }
        let size = if case == "perforated_plate" {
            [12., 12., 1.5]
        } else {
            [10.; 3]
        };
        let mut a = Solid::box_solid(p(0., 0., 0.), size).unwrap();
        match case {
            "box_union" | "box_intersection" | "box_difference" => {
                let b = Solid::box_solid(p(5., 2., 3.), [10.; 3]).unwrap();
                let operation = match case {
                    "box_union" => BooleanOperation::Union,
                    "box_intersection" => BooleanOperation::Intersection,
                    _ => BooleanOperation::Difference,
                };
                a = a.boolean(&b, operation).unwrap();
            }
            "cylinder_through_hole" => {
                a = a
                    .boolean(
                        &Solid::cylinder(p(5., 5., -1.), 2., 12.).unwrap(),
                        BooleanOperation::Difference,
                    )
                    .unwrap()
            }
            "sphere_cavity" | "hemisphere_pocket" => {
                a = a
                    .boolean(
                        &Solid::sphere(
                            p(5., 5., if case == "sphere_cavity" { 5. } else { 10. }),
                            2.,
                        )
                        .unwrap(),
                        BooleanOperation::Difference,
                    )
                    .unwrap()
            }
            "perforated_plate" => {
                for x in 0..4 {
                    for y in 0..4 {
                        a = a
                            .boolean(
                                &Solid::cylinder(
                                    p(1.5 + 3. * x as f64, 1.5 + 3. * y as f64, -1.),
                                    0.75,
                                    3.5,
                                )
                                .unwrap(),
                                BooleanOperation::Difference,
                            )
                            .unwrap();
                    }
                }
            }
            "box" => {}
            _ => panic!("Unknown fixture {case}"),
        }
        let actual = a.properties(1e-8).unwrap();
        let expected = &row["value"];
        if case == "sphere_cavity" {
            // The SDK returned no result. Keep that evidence separate from parity assertions.
            assert_eq!(expected["result_count"], 0);
            let volume = 1000. - 32. * std::f64::consts::PI / 3.;
            assert!((actual.volume - volume).abs() < 1e-6);
            assert!(actual.manifold);
            continue;
        }
        assert_eq!(expected["result_count"], 1);
        assert_eq!(expected["valid"], true);
        assert_eq!(actual.manifold, expected["manifold"].as_bool().unwrap());
        let volume = expected["volume"].as_f64().unwrap();
        assert!(
            (actual.volume - volume).abs() <= 2e-6 + volume.abs() * 1e-12,
            "{case}: {} != {volume}",
            actual.volume
        );
        for (actual, expected) in actual
            .bounds
            .iter()
            .zip(expected["bounds"].as_array().unwrap())
        {
            for (a, b) in actual.to_array().iter().zip(expected.as_array().unwrap()) {
                assert!((a - b.as_f64().unwrap()).abs() < 1e-4, "{case} bounds");
            }
        }
        qualified += 1;
    }
    assert_eq!(qualified, 7);
}

#[test]
fn native_rational_curves_preserve_definitions_and_all_parameter_samples() {
    let mut qualified = 0;
    for row in capture()["results"].as_array().unwrap() {
        if !row["id"].as_str().unwrap().starts_with("rational_") {
            continue;
        }
        let definition = &row["value"]["definition"];
        let controls = definition["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|cp| {
                let xyz = &cp["point"];
                WeightedPoint3::try_new(
                    p(
                        xyz[0].as_f64().unwrap(),
                        xyz[1].as_f64().unwrap(),
                        xyz[2].as_f64().unwrap(),
                    ),
                    cp["weight"].as_f64().unwrap(),
                )
                .unwrap()
            })
            .collect();
        let knots = definition["knots"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect();
        let original = NurbsCurve::try_new_rational(
            definition["degree"].as_u64().unwrap() as usize,
            controls,
            knots,
        )
        .unwrap();
        let native = KernelCurve::from_nurbs(&original).unwrap();
        assert_eq!(native.to_nurbs().unwrap(), original);
        let domain = original.domain();
        for (i, expected) in row["value"]["samples"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let actual = native
                .evaluate(domain.start() + (domain.end() - domain.start()) * i as f64 / 128.)
                .unwrap();
            let expected = p(
                expected[0].as_f64().unwrap(),
                expected[1].as_f64().unwrap(),
                expected[2].as_f64().unwrap(),
            );
            assert!(actual.distance_to(expected).unwrap() < 1e-12);
        }
        qualified += 1;
    }
    assert_eq!(qualified, 3);
}
