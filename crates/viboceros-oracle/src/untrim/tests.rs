use super::*;

fn same(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => assert!(
            (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-9,
            "{path}: {a} != {b}"
        ),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                same(a, b, &format!("{path}[{i}]"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{path}"
            );
            for (key, value) in a {
                same(value, &b[key], &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn full_surface_and_retained_trim_definitions_match_owned_rhino_commands() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/untrim_all.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/untrim_all.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 72);
    let mut successes = 0;
    let mut complete_definitions = 0;
    for (actual, expected) in actual
        .results
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        assert_eq!(actual.id, expected["id"]);
        let (a, b) = (&actual.value, &expected["value"]);
        assert_eq!(a["succeeded"], b["succeeded"], "{}", actual.id);
        successes += usize::from(a["succeeded"] == true);
        if actual.id.starts_with("box-") {
            // The two independent box factories have different face/edge
            // tables. This case proves whole-polysurface rejection, unchanged
            // geometry in each engine, and identical identity/attribute state.
            for (old, new) in a["before"]
                .as_array()
                .unwrap()
                .iter()
                .zip(a["after"].as_array().unwrap())
            {
                assert_eq!(old["geometry"], new["geometry"]);
            }
            for (old, new) in b["before"]
                .as_array()
                .unwrap()
                .iter()
                .zip(b["after"].as_array().unwrap())
            {
                assert_eq!(old["geometry"], new["geometry"]);
            }
            let strip = |list: &Value| {
                list.as_array()
                    .unwrap()
                    .iter()
                    .map(|object| {
                        let mut object = object.clone();
                        object.as_object_mut().unwrap().remove("geometry");
                        object
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(strip(&a["after"]), strip(&b["after"]));
        } else {
            for field in ["constructed", "before", "after"] {
                same(&a[field], &b[field], &format!("{}.{}", actual.id, field));
            }
            complete_definitions += 1;
        }
    }
    assert_eq!(successes, 64);
    assert_eq!(complete_definitions, 68);
}

#[test]
fn untrim_rejects_ambiguous_iterations_and_empty_sources() {
    let mut request: ProbeRequest = serde_json::from_value(json!({"protocol_version":1,"iterations":2,
        "operations":[{"op":"untrim_all_command","id":"empty","sources":[],"keep_trim_objects":false}]})).unwrap();
    assert!(run_request(&request).is_err());
    request.iterations = 1;
    assert!(run_request(&request).is_err());
}
