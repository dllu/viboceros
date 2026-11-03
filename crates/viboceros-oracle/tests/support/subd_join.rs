use serde_json::Value;
pub fn cases_for(family: &str) -> Vec<(Value, Value)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|path| path.join("tools/rhino_oracle").is_dir())
        .unwrap();
    let path = root.join("tools/rhino_oracle");
    let query: Value = serde_json::from_str(
        &std::fs::read_to_string(path.join("fixtures").join(format!("{family}.json"))).unwrap(),
    )
    .unwrap();
    let file =
        std::fs::File::open(path.join("observations").join(format!("{family}.json.gz"))).unwrap();
    let native: Value = serde_json::from_reader(flate2::read::GzDecoder::new(file)).unwrap();
    query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .cloned()
        .zip(
            native["results"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["value"].clone()),
        )
        .collect()
}
pub fn compare(a: &Value, b: &Value, id: &str) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => assert!(
            (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-10,
            "{id}: {a} != {b}"
        ),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{id}: array sizes");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{id}[{i}]"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{id}: object fields");
            for (k, a) in a {
                compare(a, &b[k], &format!("{id}.{k}"));
            }
        }
        _ => assert_eq!(a, b, "{id}"),
    }
}
