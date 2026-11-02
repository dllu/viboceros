use serde_json::{Value, json};
use std::path::Path;

pub fn cases() -> Vec<(Value, Value)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle").is_dir())
        .unwrap()
        .join("tools/rhino_oracle");
    let q: Value = serde_json::from_slice(
        &std::fs::read(root.join("fixtures/draft_angle_analysis_initial.json")).unwrap(),
    )
    .unwrap();
    let n: Value = serde_json::from_reader(flate2::read::GzDecoder::new(
        std::fs::File::open(root.join("observations/draft_angle_analysis_initial.json.gz"))
            .unwrap(),
    ))
    .unwrap();
    let ops = q["operations"].as_array().unwrap();
    let rows = n["results"].as_array().unwrap();
    assert_eq!(ops.len(), 22);
    assert_eq!(rows.len(), 22);
    ops.iter()
        .zip(rows)
        .map(|(op, row)| {
            assert_eq!(op["id"], row["id"]);
            assert!(row.get("error").is_none());
            (op.clone(), row["value"].clone())
        })
        .collect()
}

pub fn expected(op: &Value, native: &Value) -> Value {
    let mut enabled = false;
    let mut settings = vec![Value::Null];
    for (step, t) in op["steps"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["traces"].as_array().unwrap())
    {
        assert_eq!(t["command"], *step);
        assert_eq!(t["success"], true);
        match step.as_str().unwrap() {
            "DraftAngleAnalysis" => enabled = true,
            "DraftAngleAnalysisOff" => enabled = false,
            _ => {}
        }
        let s = &t["settings"];
        settings.push(if enabled { json!({"range":s["range"].as_array().unwrap().iter().map(|n|n.as_f64().unwrap().to_degrees()).collect::<Vec<_>>(),"up":s["up"],"wires":s["wires"]}) } else { Value::Null });
    }
    json!({"states":native["states"],"settings":settings})
}
