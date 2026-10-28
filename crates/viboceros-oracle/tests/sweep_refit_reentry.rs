//! Re-enter independently constructed C1 curves as ordinary public Sweep inputs.
use serde_json::Value;
use viboceros_oracle::{ProbeRequest, run_request};
#[path = "../../viboceros-geometry/tests/support/nurbs_surface.rs"]
mod nurbs_surface;
#[path = "../../viboceros-geometry/tests/support/profile_rows.rs"]
mod profile_rows;
#[path = "../../viboceros-geometry/tests/support/sweep_atlas.rs"]
mod sweep_atlas;

fn captured() -> (Value, Value) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |directory: &str| {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{directory}/sweep1_refit_stage_reentry.json")))
                .unwrap(),
        )
        .unwrap()
    };
    (read("fixtures"), read("observations"))
}

fn compare(input: &Value, operation: &Value, native: &Value) -> Result<Option<f64>, String> {
    let mut single = input.clone();
    single["operations"] = serde_json::json!([operation]);
    let request: ProbeRequest = serde_json::from_value(single).unwrap();
    let response = run_request(&request).map_err(|error| error.to_string())?;
    let actual = &response.results[0].value;
    if let Some(directory) = std::env::var_os("VIBOCEROS_REFIT_REENTRY_RECORDS") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join(format!("{}.json", operation["id"].as_str().unwrap())),
            serde_json::to_vec(&serde_json::json!({
                "operation":operation, "actual":actual, "native":native,
            }))
            .unwrap(),
        )
        .unwrap();
    }
    let surface = |value: &Value| {
        sweep_atlas::join_v(
            value
                .as_array()
                .unwrap()
                .iter()
                .map(|v| nurbs_surface::from_definition(&v["definition"]))
                .collect(),
        )
    };
    let (a, b) = (surface(actual), surface(native));
    let bound = profile_rows::complete_bound(&a, &b, 1e-7);
    println!(
        "SWEEP_REENTRY_REFIT_RESULT {}",
        serde_json::json!({"family":"sweep1_refit_stage_reentry","id":operation["id"],
            "complete_image_bound":bound,"epsilon":1e-7,
            "actual_controls":a.control_point_count_v(),"native_controls":b.control_point_count_v()})
    );
    Ok(bound)
}

#[test]
fn coarse_c1_profiles_retain_complete_native_boundaries() {
    let (input, observed) = captured();
    for id in [
        "closed-cubic-0.001-c1-single",
        "closed-cubic-0.01-c1-single",
    ] {
        let operation = input["operations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|op| op["id"] == id)
            .unwrap();
        let native = observed["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap();
        assert!(
            compare(&input, operation, &native["value"])
                .unwrap()
                .is_some(),
            "{id}"
        );
    }
}

#[test]
fn all_reentered_profiles_match_complete_native_images() {
    let (input, observed) = captured();
    let operations = input["operations"].as_array().unwrap();
    let rows = observed["results"].as_array().unwrap();
    assert_eq!(operations.len(), 12);
    assert_eq!(rows.len(), 12);
    let mut gaps = Vec::new();
    for (operation, row) in operations.iter().zip(rows) {
        assert_eq!(operation["id"], row["id"]);
        match compare(&input, operation, &row["value"]) {
            Ok(Some(_)) => {}
            Ok(None) => gaps.push(format!("{} whole image", operation["id"])),
            Err(error) => {
                eprintln!("REENTRY_CONSTRUCTION_GAP {}: {error}", operation["id"]);
                gaps.push(format!("{}: {error}", operation["id"]));
            }
        }
    }
    assert!(gaps.is_empty(), "Re-entered C1 Refit gaps: {gaps:#?}");
}
