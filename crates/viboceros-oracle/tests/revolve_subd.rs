#[path = "support/subd_direction.rs"]
mod compare;
#[path = "support/observation.rs"]
mod observation;
#[path = "support/revolve_subd.rs"]
mod topology;

#[test]
fn subd_revolutions_match_native_control_topology_limit_meshes_and_history() {
    family("revolve_subd_pilot", 6);
    family("revolve_subd_holdouts", 43);
    family("revolve_subd_edges", 21);
    family("revolve_subd_policy", 10);
}
#[test]
fn subd_friendly_profiles_match_independent_public_sdk() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut count = 0;
    for family in ["revolve_subd_profiles", "revolve_subd_holdouts"] {
        let mut query: serde_json::Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        query["operations"]
            .as_array_mut()
            .unwrap()
            .retain(|op| op["op"] == "revolve_subd_profile");
        let request = serde_json::from_value(query).unwrap();
        let actual =
            serde_json::to_value(viboceros_oracle::run_request(&request).unwrap()).unwrap();
        let native = observation::read(&base, family);
        for row in actual["results"].as_array().unwrap() {
            count += 1;
            let expected = native["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == row["id"])
                .unwrap();
            compare::compare(
                &row["value"]["friendly"],
                &expected["value"]["friendly"],
                &row["id"].to_string(),
            );
        }
    }
    assert_eq!(count, 17);
}
fn family(family: &str, count: usize) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut query: serde_json::Value = serde_json::from_slice(
        &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
    )
    .unwrap();
    query["operations"]
        .as_array_mut()
        .unwrap()
        .retain(|op| op["op"] == "revolve_subd");
    let request = serde_json::from_value(query).unwrap();
    let actual = serde_json::to_value(viboceros_oracle::run_request(&request).unwrap()).unwrap();
    assert_eq!(actual["results"].as_array().unwrap().len(), count);
    let native = observation::read(&base, family);
    for row in actual["results"].as_array().unwrap() {
        let expected = native["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == row["id"])
            .unwrap();
        for phase in ["before", "after", "undo", "redo"] {
            topology::compare(
                &row["value"][phase],
                &expected["value"][phase],
                &format!("{}/{phase}", row["id"]),
                topology::epsilon(row["id"].as_str().unwrap()),
            );
        }
    }
}
