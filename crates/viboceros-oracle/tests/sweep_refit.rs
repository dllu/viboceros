use serde_json::Value;
use viboceros_oracle::{ProbeRequest, run_request};
#[path = "../../viboceros-geometry/tests/support/nurbs_surface.rs"]
mod nurbs_surface;
#[path = "../../viboceros-geometry/tests/support/profile_rows.rs"]
mod profile_rows;
#[path = "../../viboceros-geometry/tests/support/sweep_atlas.rs"]
mod sweep_atlas;

#[test]
fn section_refits_match_complete_captured_images() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut gaps = Vec::new();
    let mut checked = 0;
    for family in [
        "sweep1_refit_sdk",
        "sweep1_refit_commands",
        "sweep1_refit_sdk_holdouts",
        "sweep1_refit_command_holdouts",
        "sweep1_refit_contexts",
    ] {
        let input: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let observed: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(
            input["operations"].as_array().unwrap().len(),
            observed["results"].as_array().unwrap().len()
        );
        for (op, row) in input["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(observed["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            let mut single = input.clone();
            single["operations"] = serde_json::json!([op]);
            let request: ProbeRequest = serde_json::from_value(single).unwrap();
            checked += 1;
            let response = match run_request(&request) {
                Ok(r) => r,
                Err(e) => {
                    gaps.push(format!("{family} {}: {e}", op["id"]));
                    eprintln!("REFIT_CONSTRUCTION_GAP {}: {e}", op["id"]);
                    continue;
                }
            };
            if let Some(directory) = std::env::var_os("VIBOCEROS_REFIT_RECORDS") {
                let directory = std::path::PathBuf::from(directory);
                std::fs::create_dir_all(&directory).unwrap();
                let name = format!("{family}-{}.json", op["id"].as_str().unwrap());
                std::fs::write(
                    directory.join(name),
                    serde_json::to_vec(&serde_json::json!({
                        "operation": op,
                        "actual": response.results[0].value,
                        "native": row["value"],
                    }))
                    .unwrap(),
                )
                .unwrap();
            }
            let a = sweep_atlas::join_v(
                response.results[0]
                    .value
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| nurbs_surface::from_definition(&v["definition"]))
                    .collect(),
            );
            let b = sweep_atlas::join_v(
                row["value"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| nurbs_surface::from_definition(&v["definition"]))
                    .collect(),
            );
            let (bound, separation) = profile_rows::complete_comparison(&a, &b, 1e-7);
            println!(
                "SWEEP_REFIT_RESULT {}",
                serde_json::json!({"family":family,"id":op["id"],"complete_image_bound":bound,
                    "complete_image_separation_lower_bound":separation,"epsilon":1e-7,
                    "actual_controls":a.control_point_count_v(),"native_controls":b.control_point_count_v()})
            );
            if bound.is_none() {
                gaps.push(format!("{family} {} whole image", op["id"]));
            }
        }
    }
    assert_eq!(checked, 70);
    assert!(gaps.is_empty(), "Refit gaps: {gaps:#?}");
}
