use super::diagnostic_comparison;
use serde_json::Value;
use std::path::Path;

pub fn cases(sdk: bool) -> Vec<(Value, Value)> {
    family_cases(
        if sdk {
            "brep_direction_sdk"
        } else {
            "brep_direction_command"
        },
        if sdk { 30 } else { 52 },
    )
}

pub fn session_cases() -> Vec<(Value, Value)> {
    let mut cases = family_cases("brep_direction_sessions", 36);
    cases.extend(family_cases("brep_direction_modes_seeded", 32));
    cases
}

fn family_cases(family: &str, count: usize) -> Vec<(Value, Value)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/fixtures").is_dir())
        .unwrap()
        .join("tools/rhino_oracle");
    let query: Value = serde_json::from_slice(
        &std::fs::read(root.join(format!("fixtures/{family}.json"))).unwrap(),
    )
    .unwrap();
    let file = std::fs::File::open(root.join(format!("observations/{family}.json.gz"))).unwrap();
    let native: Value = serde_json::from_reader(flate2::read::GzDecoder::new(file)).unwrap();
    let ops = query["operations"].as_array().unwrap();
    let rows = native["results"].as_array().unwrap();
    assert_eq!(ops.len(), count);
    assert_eq!(rows.len(), ops.len());
    ops.iter().zip(rows).map(|(op,row)| {
        assert_eq!(op["id"],row["id"]);
        let native=&row["value"];
        let events = native["events"].as_array().unwrap().iter().filter(|event| event["name"] == "Dir").collect::<Vec<_>>();
        let changed = native["before"] != native["after"];
        let success = if op["pair"] == true { op["cancel"] != true } else { op.get("sequence").is_none_or(|_| changed) };
        assert_eq!(native["success"], success);
        if op["sdk"] != true {
            assert!(!native["history"].as_str().unwrap().contains("Unknown command:"));
            assert_eq!(events.len(), 1);
            assert_eq!(events[0]["result"], if success { "Success" } else if op["pair"] == true { "Cancel" } else { "Nothing" });
        } else { assert!(events.is_empty()); }
        let value=serde_json::json!({"before":native["before"],"after":native["after"],"undo":native["undo"],"redo":native["redo"]});
        (op.clone(),value)
    }).collect()
}

pub fn compare(actual: &Value, native: &Value) -> f64 {
    let maximum = diagnostic_comparison::compare(actual, native);
    assert!(maximum <= 1e-9, "Dir maximum error: {maximum}");
    maximum
}
