use serde_json::Value;
use viboceros_oracle::{ProbeRequest, run_request};
#[path = "../../viboceros-geometry/tests/support/nurbs_surface.rs"]
mod nurbs_surface;
#[path = "support/parallel_pairs.rs"]
mod parallel_pairs;
#[path = "../../viboceros-geometry/tests/support/profile_rows.rs"]
mod profile_rows;
#[path = "../../viboceros-geometry/tests/support/sweep_atlas.rs"]
mod sweep_atlas;

#[test]
fn common_handle_profiles_match_complete_native_images() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let family = "sweep1_refit_common_handles";
    let input: Value = serde_json::from_slice(
        &std::fs::read(root.join(format!("fixtures/{family}.json"))).unwrap(),
    )
    .unwrap();
    let observed: Value = serde_json::from_slice(
        &std::fs::read(root.join(format!("observations/{family}.json"))).unwrap(),
    )
    .unwrap();
    assert_eq!(input["operations"].as_array().unwrap().len(), 16);
    assert_eq!(observed["results"].as_array().unwrap().len(), 16);
    let operations = input["operations"].as_array().unwrap();
    let count = operations.len();
    let gaps = parallel_pairs::compare(
        operations,
        observed["results"].as_array().unwrap(),
        |op, native| {
            assert_eq!(op["id"], native["id"]);
            let mut single = input.clone();
            single["operations"] = serde_json::json!([op]);
            let request: ProbeRequest = serde_json::from_value(single).unwrap();
            let response = run_request(&request).unwrap();
            let actual = &response.results[0];
            assert_eq!(actual.id, native["id"].as_str().unwrap());
            if let Some(directory) = std::env::var_os("VIBOCEROS_REFIT_COMMON_RECORDS") {
                let directory = std::path::PathBuf::from(directory);
                std::fs::create_dir_all(&directory).unwrap();
                std::fs::write(directory.join(format!("{}.json", actual.id)),
                    serde_json::to_vec(&serde_json::json!({"actual":actual.value,"native":native["value"],"operation":op})).unwrap()).unwrap();
            }
            let join = |values: &Value| {
                sweep_atlas::join_v(
                    values
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| nurbs_surface::from_definition(&v["definition"]))
                        .collect(),
                )
            };
            let a = join(&actual.value);
            let b = join(&native["value"]);
            let (bound, separation) = profile_rows::complete_comparison(&a, &b, 1e-7);
            println!(
                "SWEEP_COMMON_REFIT_RESULT {}",
                serde_json::json!({"id":actual.id,"family":family,"complete_image_bound":bound,
                    "complete_image_separation_lower_bound":separation,"comparison_epsilon":1e-7})
            );
            bound.is_none().then(|| actual.id.clone())
        },
    );
    assert_eq!(count, 16);
    assert!(
        gaps.is_empty(),
        "Common-handle images remain unproved: {gaps:#?}"
    );
}
