use super::*;

fn equal(a: &Value, b: &Value, path: &str) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => assert!(
            (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-9,
            "{path}: {a} vs {b}"
        ),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                equal(a, b, &format!("{path}/{i}"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{path}"
            );
            for (key, a) in a {
                equal(a, &b[key], &format!("{path}/{key}"));
            }
        }
        _ => assert_eq!(a, b, "{path}"),
    }
}

fn groups(mut state: Value) -> Value {
    for group in state["groups"].as_array_mut().unwrap() {
        if group["name"] != "Owned remap group" {
            let name = group["name"].as_str().unwrap();
            assert!(
                name.strip_prefix("Group")
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
            );
            group["name"] = json!("generated");
        }
    }
    state
}

#[test]
fn authored_remap_recipes_replay_all_fifty_two_native_geometry_and_history_records() {
    let mut checked = 0;
    for (request, native) in [
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/remap_cplane.json"),
            include_str!("../../../../tools/rhino_oracle/observations/remap_cplane.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/remap_cplane_copy.json"),
            include_str!("../../../../tools/rhino_oracle/observations/remap_cplane_copy.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/remap_cplane_views.json"),
            include_str!("../../../../tools/rhino_oracle/observations/remap_cplane_views.json"),
        ),
    ] {
        let request: ProbeRequest = serde_json::from_str(request).unwrap();
        let native: Value = serde_json::from_str(native).unwrap();
        let response = run_request(&request).unwrap();
        assert_eq!(
            response.results.len(),
            native["results"].as_array().unwrap().len()
        );
        for (local, native) in response
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(local.id, native["id"]);
            for key in ["before", "after", "after_script", "undo", "redo"] {
                equal(
                    &groups(local.value[key].clone()),
                    &groups(native["value"][key].clone()),
                    &format!("{}/{key}", local.id),
                );
            }
            for key in ["success", "undo_returned", "redo_returned"] {
                equal(&local.value[key], &native["value"][key], &local.id);
            }
            if let Some(setup) = local.value.get("setup") {
                equal(
                    &groups(setup["after"].clone()),
                    &groups(native["value"]["setup"]["after"].clone()),
                    &local.id,
                );
                equal(
                    &setup["success"],
                    &native["value"]["setup"]["success"],
                    &local.id,
                );
            }
            let rejections = native["value"]["history"]
                .as_str()
                .unwrap()
                .matches("Specify a different CPlane.")
                .count();
            assert_eq!(local.value["target_rejections"], json!(rejections));
            checked += 1;
        }
    }
    assert_eq!(checked, 52);
    eprintln!("REMAP_API_NATIVE cases={checked}");
}

#[test]
fn invalid_remap_recipe_fields_are_rejected_without_becoming_successful_values() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/remap_cplane.json"
    ))
    .unwrap();
    for (key, value) in [
        ("source", json!("unknown")),
        ("mode", json!("unknown")),
        ("copy", json!("Yes")),
        ("retry_target", Value::Null),
        ("setup_copy", Value::Null),
    ] {
        let mut bad = q.clone();
        bad["operations"][0][key] = value;
        assert!(
            serde_json::from_value::<ProbeRequest>(bad).is_err(),
            "{key}"
        );
    }
    for (key, value) in [
        ("grips", json!(true)),
        ("retry_target", json!("Owned target")),
        ("unexpected", json!(true)),
    ] {
        let mut bad = q.clone();
        bad["operations"][0][key] = value;
        let request: ProbeRequest = serde_json::from_value(bad).unwrap();
        let response = run_request_audit(&request).unwrap();
        assert!(matches!(
            &response.outcomes[0],
            OperationOutcome::Failure { .. }
        ));
    }
}
