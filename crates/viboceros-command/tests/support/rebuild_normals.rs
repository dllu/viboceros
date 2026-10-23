//! Frozen public normal queries and saved command sources, kept distinct.
use serde_json::{Value, json};

pub fn groups(commands: bool) -> Vec<(&'static str, Vec<Value>)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/observations").is_dir())
        .unwrap();
    [
        (
            if commands {
                "rebuild_normals_command"
            } else {
                "rebuild_normals_sdk"
            },
            if commands { 18 } else { 15 },
        ),
        ("rebuild_normals_holdouts", 4),
    ]
    .into_iter()
    .map(|(family, count)| {
        let data: Value = serde_json::from_str(
            &std::fs::read_to_string(
                root.join("tools/rhino_oracle/observations")
                    .join(format!("{family}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        let rows = data["results"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| {
                (r["value"]["input"]["command"].as_str() == Some("RebuildMeshNormals")) == commands
            })
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), count);
        (family, rows)
    })
    .collect()
}

pub fn report(row: &Value, maximum: f64) -> Value {
    json!({"id":row["id"],"qualified":true,"geometry":true,"channels":true,"normals":true,"ngons":true,"identity":true,"metadata":true,"selection":true,"object_order":true,"history":true,"accepted":row["value"]["success"],"maximum_error":maximum})
}
