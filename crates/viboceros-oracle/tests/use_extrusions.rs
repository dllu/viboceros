#[path = "support/observation.rs"]
mod observation;

#[test]
fn native_application_preference_preserves_selection_geometry_and_redo() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let request =
        serde_json::from_slice(&std::fs::read(base.join("fixtures/use_extrusions.json")).unwrap())
            .unwrap();
    let actual = serde_json::to_value(viboceros_oracle::run_request(&request).unwrap()).unwrap();
    let native = observation::read(&base, "use_extrusions");
    assert_eq!(actual["results"].as_array().unwrap().len(), 8);
    for (a, b) in actual["results"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(a["id"], b["id"]);
        for field in ["before", "after", "redo", "success", "redone"] {
            assert_eq!(
                a["value"][field], b["value"][field],
                "{}/{}",
                a["id"], field
            );
        }
    }
}
