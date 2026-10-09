use serde_json::Value;
use viboceros_oracle::{ProbeRequest, run_request};

fn compare(actual: &Value, expected: &Value, path: &str, numbers: &mut usize) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => {
            *numbers += 1;
            if let (Some(a), Some(b)) = (a.as_i64(), b.as_i64()) {
                assert_eq!(a, b, "{path}");
            } else {
                let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
                assert!(
                    (a - b).abs() <= 1e-9 + 1e-12 * b.abs(),
                    "{path}: {a} != {b}"
                );
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{path}[{i}]"), numbers);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{path}"
            );
            for (key, a) in a {
                compare(a, &b[key], &format!("{path}.{key}"), numbers);
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn replay_native_block_commands_graphs_geometry_and_metadata() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/block_workflow_commands.json"
    ))
    .unwrap();
    let native: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/block_workflow_commands.json"
    ))
    .unwrap();
    let result = run_request(&request).unwrap();
    assert_eq!(result.results.len(), 20);
    let expected = native["results"].as_array().unwrap();
    assert_eq!(expected.len(), result.results.len());
    assert_eq!(native["error"], Value::Null);
    let mut numbers = 0;
    for actual in result.results {
        let expected = expected.iter().find(|r| r["id"] == actual.id).unwrap();
        compare(&actual.value, &expected["value"], &actual.id, &mut numbers);
    }
    assert!(numbers > 10_000, "only {numbers} numeric witnesses");
}

#[test]
fn native_block_creation_and_both_explosion_group_policies_match_full_graphs() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/block_groups_commands.json"
    ))
    .unwrap();
    let native: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/block_groups_commands.json"
    ))
    .unwrap();
    let result = run_request(&request).unwrap();
    assert_eq!(result.results.len(), 16);
    let references = native["results"].as_array().unwrap();
    assert_eq!(references.len(), 16);
    let mut numbers = 0;
    for actual in result.results {
        let expected = references.iter().find(|r| r["id"] == actual.id).unwrap();
        compare(&actual.value, &expected["value"], &actual.id, &mut numbers);
    }
    assert!(numbers > 1000);
}

#[test]
fn root_text_stays_separate_and_sdk_color_policy_remains_a_diagnostic() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/block_workflow_metadata.json"
    ))
    .unwrap();
    let native: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/block_workflow_metadata.json"
    ))
    .unwrap();
    let result = run_request(&request).unwrap();
    assert_eq!(result.results.len(), 4);
    let expected = native["results"].as_array().unwrap();
    let mut numbers = 0;
    for actual in result.results {
        let reference = expected.iter().find(|r| r["id"] == actual.id).unwrap();
        if actual.id.contains("-command-") {
            compare(&actual.value, &reference["value"], &actual.id, &mut numbers);
        } else {
            // This protocol selects a native reference entrypoint; it does not
            // change the local document command's metadata contract.
            let local = &actual.value["states"][3]["objects"][1]["attributes"];
            let sdk = &reference["value"]["states"][3]["objects"][1]["attributes"];
            assert_eq!(local["color_source"], "parent");
            assert_eq!(sdk["color_source"], "object");
            assert_eq!(local["user_text"], sdk["user_text"]);
            assert_eq!(local["geometry_user_text"], sdk["geometry_user_text"]);
            assert_eq!(sdk["user_text"]["Part"], "member");
            assert!(sdk["user_text"].get("RootOnly").is_none());
        }
    }
    assert!(numbers > 100);
}
