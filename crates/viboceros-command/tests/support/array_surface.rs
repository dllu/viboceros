//! ArraySrf native scene replay shares exact cloud/group/selection comparison.
pub use crate::array_curve_surface_test_support::{Scene, plane, point, setup, token};
use serde_json::{Value, json};
use viboceros_command::{CommandContext, CommandRegistry};
use viboceros_document::Document;
pub fn rows() -> Vec<(Value, Value)> {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/array_surface.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/array_surface.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 46);
    assert_eq!(r["results"].as_array().unwrap().len(), 46);
    q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
        .map(|(op, row)| {
            assert_eq!(op["id"], row["id"]);
            assert!(row["error"].is_null());
            assert_eq!(row["value"]["input_trace"]["errors"], json!([]));
            (op.clone(), row["value"].clone())
        })
        .collect()
}
pub fn execute(
    scene: &Scene,
    doc: &mut Document,
    commands: &CommandRegistry,
    op: &Value,
) -> Result<(), viboceros_command::CommandError> {
    if op["finish"] != "Enter" {
        if op["postselect"] == true {
            doc.clear_selection();
        }
        return Ok(());
    }
    let mut counts: [usize; 2] = serde_json::from_value(op["counts"].clone()).unwrap();
    let mut mode = op["mode"].as_str().unwrap();
    for edit in op["edits"].as_array().unwrap() {
        if let Some(n) = edit["UNumber"].as_u64() {
            counts[0] = n as usize;
        }
        if let Some(n) = edit["VNumber"].as_u64() {
            counts[1] = n as usize;
        }
        if let Some(m) = edit["Mode"].as_str() {
            mode = m;
        }
    }
    let base = point(&op["base_point"]);
    let up = if op["up_point"].is_null() {
        String::new()
    } else {
        format!(
            " Up={}",
            base.vector_to(point(&op["up_point"]))
                .unwrap()
                .to_array()
                .map(|x| x.to_string())
                .join(",")
        )
    };
    commands.execute_in_context(
        doc,
        &format!(
            "ArraySrf {} {} BasePoint={} Surface={} Mode={mode}{up} {}={}",
            counts[0],
            counts[1],
            token(base),
            scene.target,
            if op["postselect"] == true {
                "PickedSources"
            } else {
                "Sources"
            },
            scene
                .sources
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ),
        CommandContext {
            construction_plane: plane(op),
        },
    )?;
    Ok(())
}
