use serde_json::Value;
use viboceros_oracle::{ProbeRequest, run_request};
fn compare(a: &Value, b: &Value, path: &str, numbers: &mut usize) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            *numbers += 1;
            let a = a.as_f64().unwrap();
            let b = b.as_f64().unwrap();
            assert!(
                (a - b).abs() <= 1e-9 + 1e-12 * b.abs(),
                "{path}: {a} != {b}"
            );
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
            for (k, v) in a {
                compare(v, &b[k], &format!("{path}.{k}"), numbers);
            }
        }
        _ => assert_eq!(a, b, "{path}"),
    }
}
#[test]
fn contour_replays_native_domains_from_edges_isocurves_plane_axes_shifted_uv_and_rational_surfaces()
{
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/contour_domains.json"
    ))
    .unwrap();
    let native: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/contour_domains.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 52);
    let mut numbers = 0;
    for result in actual.results {
        let reference = native["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == result.id)
            .unwrap();
        compare(&result.value, &reference["value"], &result.id, &mut numbers);
    }
    assert!(numbers > 2000);
}
