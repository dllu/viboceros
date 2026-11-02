use super::diagnostic_comparison;
use serde_json::Value;
use std::path::Path;

pub fn cases(sdk: bool) -> Vec<(Value, Value)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/fixtures").is_dir())
        .unwrap()
        .join("tools/rhino_oracle");
    let family = if sdk {
        "brep_direction_sdk"
    } else {
        "brep_direction_command"
    };
    let query: Value = serde_json::from_slice(
        &std::fs::read(root.join(format!("fixtures/{family}.json"))).unwrap(),
    )
    .unwrap();
    let file = std::fs::File::open(root.join(format!("observations/{family}.json.gz"))).unwrap();
    let native: Value = serde_json::from_reader(flate2::read::GzDecoder::new(file)).unwrap();
    let ops = query["operations"].as_array().unwrap();
    let rows = native["results"].as_array().unwrap();
    assert_eq!(ops.len(), if sdk { 30 } else { 52 });
    assert_eq!(rows.len(), ops.len());
    ops.iter().zip(rows).map(|(op,row)| {
        assert_eq!(op["id"],row["id"]);assert_eq!(row["value"]["success"],true);
        let native=&row["value"];
        if !sdk {assert!(!native["history"].as_str().unwrap().contains("Unknown command:"));}
        let value=serde_json::json!({"before":native["before"],"after":native["after"],"undo":native["undo"],"redo":native["redo"]});
        (op.clone(),value)
    }).collect()
}

pub fn compare(actual: &Value, native: &Value) -> f64 {
    let maximum = diagnostic_comparison::compare(actual, native);
    assert!(maximum <= 1e-9, "Dir maximum error: {maximum}");
    maximum
}
