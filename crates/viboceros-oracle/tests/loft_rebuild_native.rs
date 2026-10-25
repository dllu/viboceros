use serde_json::{Value, json};
use std::path::Path;
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_rebuild.rs"]
mod comparison;

#[test]
#[ignore = "Retained native command face-sense and crease differences; full raw comparison"]
fn rebuilt_loft_commands_match_native_geometry_topology_and_source_state() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let query: Value = serde_json::from_slice(
        &std::fs::read(root.join("fixtures/loft_rebuild_commands.json")).unwrap(),
    )
    .unwrap();
    let native: Value = serde_json::from_slice(
        &std::fs::read(root.join("observations/loft_rebuild_commands.json")).unwrap(),
    )
    .unwrap();
    let mut failures = Vec::new();
    let mut matched = 0;
    let mut maximum = 0_f64;
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let mut request = query.clone();
        request["operations"] = json!([op]);
        let request: ProbeRequest = serde_json::from_value(request).unwrap();
        let actual = match run_request(&request) {
            Ok(r) => r.results[0].value.clone(),
            Err(e) => {
                failures.push(format!("{}: {e:?}", op["id"]));
                continue;
            }
        };
        let error = comparison::difference(&actual, &row["value"]);
        match error {
            Ok(e) if e <= 1e-6 => {
                matched += 1;
                maximum = maximum.max(e)
            }
            _ => {
                failures.push(format!("{}: {:?}", op["id"], error));
                println!(
                    "LOFT_REBUILD_COMMAND_DIFFERENCE {}",
                    json!({"id":op["id"],"actual":actual,"expected":row["value"]})
                );
            }
        }
    }
    println!(
        "LOFT_REBUILD_COMMAND matched={matched} max={maximum} failures={}",
        failures.len()
    );
    assert!(failures.is_empty(), "{failures:?}");
    assert_eq!(matched, 140);
}

#[test]
fn public_rebuilt_loft_api_matches_all_native_surfaces_and_retains_inputs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut matched = 0;
    let mut empty = 0;
    let mut maximum = 0_f64;
    for family in ["loft_rebuild_smoke", "loft_rebuild", "loft_rebuild_bounds"] {
        let query: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let native: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        for (op, row) in query["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            let mut request = query.clone();
            request["operations"] = json!([op]);
            let request: ProbeRequest = serde_json::from_value(request).unwrap();
            let result = run_request(&request);
            if row["value"]["surfaces"].as_array().unwrap().is_empty() {
                assert!(result.is_err(), "{}", op["id"]);
                empty += 1;
                continue;
            }
            let actual = result
                .unwrap_or_else(|e| panic!("{}: {e:?}", op["id"]))
                .results[0]
                .value
                .clone();
            let mut expected = row["value"].clone();
            expected.as_object_mut().unwrap().remove("rebuilt");
            let error = comparison::difference(&actual, &expected)
                .unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
            assert!(error <= 1e-6, "{}: {error}", op["id"]);
            maximum = maximum.max(error);
            matched += 1;
        }
    }
    assert_eq!((matched, empty), (397, 36));
    println!("LOFT_REBUILD_API matched={matched} empty={empty} maximum={maximum}");
}
