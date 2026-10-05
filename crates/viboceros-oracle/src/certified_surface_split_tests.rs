use super::*;

fn compare(actual: &Value, native: &Value, path: &str) -> f64 {
    match (actual, native) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            let error = (a - b).abs();
            assert!(
                error <= 1e-9 + 1e-10 * a.abs().max(b.abs()),
                "{path}: {a} differs from native {b}"
            );
            error
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            a.iter()
                .zip(b)
                .enumerate()
                .map(|(i, (a, b))| compare(a, b, &format!("{path}[{i}]")))
                .fold(0_f64, f64::max)
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{path}"
            );
            a.iter()
                .map(|(key, a)| compare(a, &b[key], &format!("{path}.{key}")))
                .fold(0_f64, f64::max)
        }
        _ => {
            assert_eq!(actual, native, "{path}");
            0.
        }
    }
}

#[test]
fn certified_nonaffine_surface_splits_replay_complete_native_outputs() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/surface_split_nonaffine_trimmed.json"
    ))
    .unwrap();
    let native: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/certified_surface_split.json"
    ))
    .unwrap();
    assert_eq!(native["engine_version"], "8.32.26160.13001");
    let result = run_request(&request).unwrap();
    assert_eq!(result.results.len(), 2);
    assert_eq!(native["results"].as_array().unwrap().len(), 2);
    for (actual, native) in result
        .results
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(actual.id, native["id"].as_str().unwrap());
        let error = compare(&actual.value, &native["value"], &actual.id);
        assert!(
            error <= 1e-9,
            "{} maximum absolute error {error}",
            actual.id
        );
    }
}
