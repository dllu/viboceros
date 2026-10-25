use serde_json::{Value, json};
use std::path::Path;
use viboceros_geometry::{CurveRef, Tolerance};
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_rebuild.rs"]
mod comparison;
use comparison::definitions;

#[test]
fn open_loft_commands_match_native_order_direction_topology_and_source_state() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut matched = 0;
    let mut failures = Vec::new();
    let mut maximum = 0_f64;
    for family in [
        "loft_ordering_smoke",
        "loft_ordering",
        "loft_line_ordering",
        "loft_ordering_holdouts",
        "loft_ordering_spatial",
    ] {
        let query: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let native: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(
            query["operations"].as_array().unwrap().len(),
            native["results"].as_array().unwrap().len()
        );
        for (op, row) in query["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            assert_eq!(
                op["curves"].as_array().unwrap().len(),
                row["value"]["input_planes"].as_array().unwrap().len()
            );
            for (curve, plane) in op["curves"]
                .as_array()
                .unwrap()
                .iter()
                .zip(row["value"]["input_planes"].as_array().unwrap())
            {
                let curve = definitions::curve(curve);
                let normal = CurveRef::NurbsCurve(&curve)
                    .supporting_plane_normal(Tolerance::DEFAULT)
                    .unwrap();
                assert_eq!(normal.is_some(), plane["planar"].as_bool().unwrap());
                if let Some(normal) = normal {
                    let expected: [f64; 3] =
                        serde_json::from_value(plane["normal"].clone()).unwrap();
                    for (a, b) in normal.as_vector().to_array().into_iter().zip(expected) {
                        assert!((a - b).abs() <= 1e-12, "{}: support-plane normal", op["id"]);
                    }
                }
            }
            let mut request = query.clone();
            request["operations"] = json!([op]);
            let request: ProbeRequest = serde_json::from_value(request).unwrap();
            let actual = match run_request(&request) {
                Ok(v) => v.results[0].value.clone(),
                Err(e) => {
                    failures.push(format!("{}: {e:?}", op["id"]));
                    continue;
                }
            };
            let mut expected = row["value"].clone();
            expected.as_object_mut().unwrap().remove("input_planes");
            match comparison::difference(&actual, &expected) {
                Ok(e) if e <= 1e-9 => {
                    matched += 1;
                    maximum = maximum.max(e)
                }
                e => {
                    failures.push(format!("{}: {e:?}", op["id"]));
                    println!(
                        "LOFT_ORDER_DIFFERENCE {}",
                        json!({"id":op["id"],"actual":actual,"expected":expected})
                    );
                }
            }
        }
    }
    println!(
        "LOFT_ORDER matched={matched} maximum={maximum} failures={}",
        failures.len()
    );
    assert!(failures.is_empty(), "{failures:?}");
    assert_eq!(matched, 464);
}

#[test]
fn rebuilt_open_non_straight_commands_match_native_raw_records() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let query: Value = serde_json::from_slice(
        &std::fs::read(root.join("fixtures/loft_rebuild_commands.json")).unwrap(),
    )
    .unwrap();
    let native: Value = serde_json::from_slice(
        &std::fs::read(root.join("observations/loft_rebuild_commands.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        query["operations"].as_array().unwrap().len(),
        native["results"].as_array().unwrap().len()
    );
    let mut matched = 0;
    let mut maximum = 0_f64;
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let curves = op["curves"]
            .as_array()
            .unwrap()
            .iter()
            .map(definitions::curve)
            .collect::<Vec<_>>();
        if curves.iter().any(|c| c.is_closed().unwrap()) || op["style"] == "straight" {
            continue;
        }
        let mut request = query.clone();
        request["operations"] = json!([op]);
        let request: ProbeRequest = serde_json::from_value(request).unwrap();
        let actual = run_request(&request).unwrap_or_else(|e| panic!("{}: {e:?}", op["id"]));
        let error = comparison::difference(&actual.results[0].value, &row["value"])
            .unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
        assert!(error <= 1e-6, "{}: {error}", op["id"]);
        maximum = maximum.max(error);
        matched += 1;
    }
    assert_eq!(matched, 48);
    println!("LOFT_ORDER_REBUILD matched={matched} maximum={maximum}");
}
