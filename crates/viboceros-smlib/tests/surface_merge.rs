#![cfg(feature = "native")]
//! Authored inputs only; retained public SDK outputs are expectations.
use serde_json::{Value, json};
use std::{fs::File, path::Path};
use viboceros_geometry::*;
use viboceros_smlib::{SurfaceMergeOptions, merge_surface_supports};

fn rows() -> Vec<Value> {
    ["pilot", "holdouts", "final"]
        .into_iter()
        .flat_map(|family| {
            let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                "../../tools/rhino_oracle/observations/merge_surfaces_{family}.json.gz"
            ));
            let data: Value =
                serde_json::from_reader(flate2::read::GzDecoder::new(File::open(path).unwrap()))
                    .unwrap();
            assert_eq!(data["engine"], "rhino");
            data["results"].as_array().unwrap().clone()
        })
        .collect()
}

fn numbers(v: &Value) -> Vec<f64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect()
}

fn surface(v: &Value) -> NurbsSurface {
    let native = v.get("degree").is_some();
    let count = if native {
        numbers(&v["control_count"])
    } else {
        vec![
            v["control_point_count_u"].as_f64().unwrap(),
            v["control_point_count_v"].as_f64().unwrap(),
        ]
    };
    let degree = if native {
        numbers(&v["degree"])
    } else {
        vec![
            v["degree_u"].as_f64().unwrap(),
            v["degree_v"].as_f64().unwrap(),
        ]
    };
    let controls = v["control_points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let q = numbers(&p["point"]);
            WeightedPoint3::try_new(
                Point3::try_new(q[0], q[1], q[2]).unwrap(),
                p["weight"].as_f64().unwrap(),
            )
            .unwrap()
        })
        .collect();
    let mut result = NurbsSurface::try_new_rational(
        degree[0] as usize,
        degree[1] as usize,
        count[0] as usize,
        count[1] as usize,
        controls,
        numbers(&v["knots_u"]),
        numbers(&v["knots_v"]),
    )
    .unwrap();
    if !native && v.get("domain_u").is_some() {
        let u = numbers(&v["domain_u"]);
        let w = numbers(&v["domain_v"]);
        result = result
            .try_reparameterized(u[0]..=u[1], w[0]..=w[1])
            .unwrap();
    }
    result
}

fn options(recipe: &Value) -> SurfaceMergeOptions {
    SurfaceMergeOptions {
        tolerance: recipe["merge_tolerance"].as_f64().unwrap(),
        smooth: recipe["smooth"].as_bool().unwrap(),
        roundness: recipe["roundness"].as_f64().unwrap(),
    }
}

#[test]
fn oriented_supports_match_public_sdk_without_borrowed_input_mutation() {
    let rows = rows();
    assert_eq!(rows.len(), 30);
    let mut ids = std::collections::BTreeSet::new();
    for row in rows {
        let id = row["id"].as_str().unwrap();
        assert!(ids.insert(id.to_owned()));
        let value = &row["value"];
        let recipe = &value["prescribed_case"];
        assert_eq!(value["source_unchanged"], true);
        let first = surface(&recipe["sources"][0]);
        let original_second = surface(&recipe["sources"][1]);
        // This primitive takes oriented supports. Apply only the independently
        // prescribed input-direction edits, never native output coefficients.
        let second = match id {
            "merge_second_u" => original_second.try_reversed_u().unwrap(),
            "merge_second_v" => original_second.try_reversed_v().unwrap(),
            "merge_second_transpose" => original_second.try_swapped_uv().unwrap(),
            _ => original_second.clone(),
        };
        let before = (first.clone(), second.clone());
        let result =
            merge_surface_supports(&first, &second, SurfaceKnotDirection::U, options(recipe));
        assert_eq!((first, second), before, "{id}");
        if !value["success"].as_bool().unwrap() {
            assert!(
                result.is_err(),
                "{id}: native declined but product accepted"
            );
            println!("MERGE_SURFACE_SUPPORT {}", json!({"id":id,"success":false}));
            continue;
        }
        let actual = result.unwrap_or_else(|e| panic!("{id}: {e}"));
        let expected = surface(&value["output"]["surfaces"][0]);
        assert_eq!(
            (actual.degree_u(), actual.degree_v()),
            (expected.degree_u(), expected.degree_v()),
            "{id}"
        );
        assert_eq!(
            (
                actual.control_point_count_u(),
                actual.control_point_count_v()
            ),
            (
                expected.control_point_count_u(),
                expected.control_point_count_v()
            ),
            "{id}"
        );
        for (a, b) in [
            (actual.knots_u(), expected.knots_u()),
            (actual.knots_v(), expected.knots_v()),
        ] {
            assert_eq!(a.len(), b.len(), "{id}");
            for (a, b) in a.iter().zip(b) {
                assert!((a - b).abs() <= 1e-12, "{id}: knot {a} vs {b}");
            }
        }
        let mut maximum_error = 0_f64;
        for i in 0..=16 {
            for j in 0..=16 {
                let u = expected.parameter_at_u(i as f64 / 16.).unwrap();
                let v = expected.parameter_at_v(j as f64 / 16.).unwrap();
                maximum_error = maximum_error.max(
                    actual
                        .evaluate(u, v)
                        .unwrap()
                        .distance_to(expected.evaluate(u, v).unwrap())
                        .unwrap(),
                );
            }
        }
        assert!(maximum_error <= 1e-9, "{id}: {maximum_error}");
        println!(
            "MERGE_SURFACE_SUPPORT {}",
            json!({"id":id,"success":true,"maximum_geometry_error":maximum_error})
        );
    }
}

#[test]
fn transposed_supports_merge_along_v_with_the_same_surface_image() {
    let row = rows()
        .into_iter()
        .find(|r| r["id"] == "merge_nonunit_smooth")
        .unwrap();
    let value = &row["value"];
    let recipe = &value["prescribed_case"];
    let first = surface(&recipe["sources"][0]).try_swapped_uv().unwrap();
    let second = surface(&recipe["sources"][1]).try_swapped_uv().unwrap();
    let actual =
        merge_surface_supports(&first, &second, SurfaceKnotDirection::V, options(recipe)).unwrap();
    let expected = surface(&value["output"]["surfaces"][0])
        .try_swapped_uv()
        .unwrap();
    assert_eq!(actual.knots_u(), expected.knots_u());
    assert_eq!(actual.knots_v(), expected.knots_v());
    for i in 0..=16 {
        for j in 0..=16 {
            let u = expected.parameter_at_u(i as f64 / 16.).unwrap();
            let v = expected.parameter_at_v(j as f64 / 16.).unwrap();
            assert!(
                actual
                    .evaluate(u, v)
                    .unwrap()
                    .distance_to(expected.evaluate(u, v).unwrap())
                    .unwrap()
                    <= 1e-9
            );
        }
    }
}

#[test]
fn unequal_degrees_keep_the_first_nonunit_transverse_clock() {
    let row = rows()
        .into_iter()
        .find(|r| r["id"] == "merge_along_degrees")
        .unwrap();
    let value = &row["value"];
    let recipe = &value["prescribed_case"];
    let first = surface(&recipe["sources"][0])
        .try_reparameterized(2. ..=3., -3. ..=5.)
        .unwrap();
    let second = surface(&recipe["sources"][1])
        .try_reparameterized(7. ..=8., 10. ..=20.)
        .unwrap();
    let actual =
        merge_surface_supports(&first, &second, SurfaceKnotDirection::U, options(recipe)).unwrap();
    let expected = surface(&value["output"]["surfaces"][0])
        .try_reparameterized(2. ..=4., -3. ..=5.)
        .unwrap();
    assert_eq!(actual.domain_u(), 2. ..=4.);
    assert_eq!(actual.domain_v(), -3. ..=5.);
    for i in 0..=16 {
        for j in 0..=16 {
            let u = expected.parameter_at_u(i as f64 / 16.).unwrap();
            let v = expected.parameter_at_v(j as f64 / 16.).unwrap();
            assert!(
                actual
                    .evaluate(u, v)
                    .unwrap()
                    .distance_to(expected.evaluate(u, v).unwrap())
                    .unwrap()
                    <= 1e-9
            );
        }
    }
}

#[test]
fn invalid_options_and_directions_leave_sources_usable() {
    let row = rows().remove(0);
    let recipe = &row["value"]["prescribed_case"];
    let first = surface(&recipe["sources"][0]);
    let second = surface(&recipe["sources"][1]);
    let before = (first.clone(), second.clone());
    for tolerance in [-1., f64::NAN, f64::INFINITY] {
        assert!(
            merge_surface_supports(
                &first,
                &second,
                SurfaceKnotDirection::U,
                SurfaceMergeOptions {
                    tolerance,
                    ..options(recipe)
                }
            )
            .is_err()
        );
    }
    for roundness in [-1., 1.01, f64::NAN, f64::INFINITY] {
        assert!(
            merge_surface_supports(
                &first,
                &second,
                SurfaceKnotDirection::U,
                SurfaceMergeOptions {
                    roundness,
                    ..options(recipe)
                }
            )
            .is_err()
        );
    }
    assert!(
        merge_surface_supports(&first, &second, SurfaceKnotDirection::Both, options(recipe))
            .is_err()
    );
    assert_eq!((&first, &second), (&before.0, &before.1));
    assert!(
        merge_surface_supports(&first, &second, SurfaceKnotDirection::U, options(recipe)).is_ok()
    );
}
