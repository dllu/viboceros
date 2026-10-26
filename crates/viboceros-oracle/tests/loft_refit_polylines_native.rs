//! Full native definitions, domains and sampled surfaces, retaining gaps too.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_rebuild.rs"]
mod comparison;

#[test]
fn polygon_refit_matches_native_and_retains_unimplemented_smooth_cases() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut matched = 0;
    let mut pending = 0;
    let mut maximum = 0_f64;
    for family in [
        "loft_refit",
        "loft_refit_polylines",
        "loft_refit_polyline_angles",
        "loft_refit_polyline_ends",
    ] {
        let read = |folder| -> Value {
            serde_json::from_slice(
                &std::fs::read(root.join(format!("{folder}/{family}.json"))).unwrap(),
            )
            .unwrap()
        };
        let query = read("fixtures");
        let native = read("observations");
        for (op, row) in query["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            let id = op["id"].as_str().unwrap();
            if family == "loft_refit"
                && (!id.starts_with("loft-refit-polygon-") || op["refit_tolerance"] == 0.)
            {
                continue;
            }
            assert_eq!(op["id"], row["id"]);
            let smooth = id.contains("-small_kink-")
                || id
                    .strip_prefix("loft-refit-corner-angle-")
                    .is_some_and(|s| s.split('-').next().unwrap().parse::<f64>().unwrap() <= 1.);
            let empty = row["value"]["surfaces"].as_array().unwrap().is_empty();
            let mut request = query.clone();
            request["operations"] = json!([op]);
            let result = run_request(&serde_json::from_value::<ProbeRequest>(request).unwrap());
            let subdivided = row["value"]["surfaces"][0]["control_count"][1]
                .as_u64()
                .is_some_and(|n| n > 4);
            if (smooth && subdivided) || empty {
                let error = result.unwrap_err().to_string();
                assert!(
                    error.contains(if family == "loft_refit_polyline_ends" {
                        "Uniform"
                    } else {
                        "Loft Refit currently requires"
                    }),
                    "{id}: {error}"
                );
                pending += 1;
                continue;
            }
            let value = result.unwrap_or_else(|e| panic!("{id}: {e}")).results[0]
                .value
                .clone();
            let mut expected = row["value"].clone();
            expected.as_object_mut().unwrap().remove("refitted");
            let error =
                comparison::difference(&value, &expected).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert!(error <= 1e-9, "{id}: {error}");
            assert_eq!(value["before"], value["after"]);
            maximum = maximum.max(error);
            matched += 1;
        }
    }
    assert_eq!((matched, pending), (158, 28));
    println!("LOFT_REFIT_POLYLINES matched={matched} pending={pending} maximum={maximum}");
}
