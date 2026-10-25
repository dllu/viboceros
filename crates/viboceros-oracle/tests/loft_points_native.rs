use serde_json::Value;
use std::path::Path;
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_points.rs"]
mod comparison;

#[test]
fn public_point_ended_api_matches_native_poles_geometry_and_source_purity() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let (mut count, mut maximum) = (0, 0_f64);
    for family in ["loft_points", "loft_points_more_sections"] {
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
            let surfaces = row["value"]["surfaces"].as_array().unwrap();
            if surfaces.len() != 1 || !comparison::honors_point_ends(&surfaces[0], op) {
                continue;
            }
            assert_eq!(op["id"], row["id"]);
            let mut request = query.clone();
            request["operations"] = serde_json::json!([op]);
            let request: ProbeRequest = serde_json::from_value(request).unwrap();
            let actual = run_request(&request).unwrap();
            assert_eq!(actual.results.len(), 1);
            maximum = maximum.max(
                comparison::compare(&actual.results[0].value, &row["value"])
                    .unwrap_or_else(|e| panic!("{}: {e}", op["id"])),
            );
            count += 1;
        }
    }
    assert_eq!(count, 96);
    println!("LOFT_POINT_API cases={count} maximum_error={maximum}");
}
