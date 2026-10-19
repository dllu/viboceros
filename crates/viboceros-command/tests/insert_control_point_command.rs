use serde_json::Value;
use viboceros_command::{
    CommandRegistry,
    insert_control_point::{Draft, Options},
    open_3dm_with_views,
};
use viboceros_document::{ObjectId, SelectionMode};
use viboceros_geometry::Point3;
#[path = "support/insert_kink.rs"]
mod comparison;
#[allow(dead_code)]
#[path = "support/simplify_curves.rs"]
mod native_curve_comparison;
#[path = "support/remove_knot.rs"]
mod topology;

#[test]
fn insert_control_point_native_geometry_metadata_selection_and_history() {
    matrix("command", 82);
}
#[test]
fn insert_control_point_extensions_native_geometry_and_history() {
    matrix("extensions", 24);
}
#[test]
fn insert_control_point_endpoints_seams_and_junctions() {
    matrix("endpoints", 15);
}
#[test]
fn insert_control_point_preserves_analytic_source_charts() {
    matrix("charts", 3);
}
fn matrix(matrix: &str, count: usize) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(
            root.join(format!("observations/insert_control_point_{matrix}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(capture["results"].as_array().unwrap().len(), count);
    let mut failures = Vec::new();
    for row in capture["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            replay(&root, &row["value"], matrix)
        })) {
            Ok(gap) if gap <= 1e-8 => eprintln!(
                "INSERT_CONTROL_POINT_API_AUDIT {}",
                serde_json::json!({"matrix":matrix,"id":name,"maximum_error":gap})
            ),
            _ => failures.push(name),
        }
    }
    assert!(failures.is_empty(), "Command differences: {failures:?}");
}

fn replay(root: &std::path::Path, v: &Value, matrix: &str) -> f64 {
    let recipe = &v["recipe"];
    let name = recipe["id"].as_str().unwrap();
    let (mut doc, _, _, _) = open_3dm_with_views(
        root.join(format!("fixtures/insert_control_point_{matrix}/{name}.3dm"))
            .to_str()
            .unwrap(),
    )
    .unwrap();
    let id: ObjectId = v["source_id"].as_str().unwrap().parse().unwrap();
    doc.clear_selection();
    if v["before"][0]["selected"] == true {
        doc.select_object(id, SelectionMode::Replace).unwrap();
    }
    doc.clear_history().unwrap();
    let before = doc.object(id).unwrap().clone();
    let mut draft = Draft::new(&doc, id).unwrap();
    let mut options = Options::default();
    let mut picks = v["picks"].as_array().unwrap().iter();
    for token in v["tokens"].as_array().unwrap() {
        let token = token.as_str().unwrap();
        if draft.cancelled() {
            break;
        }
        if token == "_Extend" {
            draft.extend().unwrap();
        } else if token.starts_with('w') {
            let point = Point3::try_from(
                serde_json::from_value::<[f64; 3]>(picks.next().unwrap().clone()).unwrap(),
            )
            .unwrap();
            draft.point(point, options).unwrap();
        } else if !matches!(token, "_Enter" | "!") {
            assert!(
                viboceros_command::insert_control_point::accept_option(&mut options, token)
                    .unwrap(),
                "{name}: option {token}"
            );
        }
        assert_eq!(doc.object(id), Some(&before));
        assert!(!doc.can_undo());
    }
    if recipe["cancel"] != true {
        doc.select_object(id, SelectionMode::Replace).unwrap();
        let args = v["tokens"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .filter(|s| *s != "_Enter")
            .collect::<Vec<_>>()
            .join(" ");
        let registry = CommandRegistry::with_builtins();
        let command = format!("InsertControlPoint Points {args}");
        if recipe["preselect"] == false {
            registry
                .execute_postselected(&mut doc, &command, Default::default())
                .unwrap();
        } else {
            registry.execute(&mut doc, &command).unwrap();
        }
        assert_eq!(
            doc.object(id).unwrap().geometry(),
            draft.geometry(),
            "{name}: batch and draft agree"
        );
    }
    let actual = doc.object(id).unwrap().clone();
    assert_eq!(actual.attributes(), before.attributes());
    assert_eq!(actual.group_ids(), before.group_ids());
    let gap = topology::check(&doc, &v["after"], id, name);
    if doc.can_undo() {
        assert_eq!(doc.undo_label(), Some("InsertControlPoint"));
        doc.undo().unwrap();
        assert_eq!(doc.object(id), Some(&before));
        if !v["undo"].is_null() {
            assert!(topology::check(&doc, &v["undo"], id, name) <= 1e-8);
        }
        doc.redo().unwrap();
        assert_eq!(doc.object(id), Some(&actual));
    } else {
        assert_eq!(actual, before);
        assert!(!doc.can_redo());
        assert!(v["undo"].is_null());
    }
    gap
}

#[test]
fn insert_control_point_malformed_input_and_stale_sources_are_atomic() {
    let mut doc = viboceros_document::Document::default();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Line 0,0,0 6,0,0").unwrap();
    let id = doc.objects().next().unwrap().id();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    doc.clear_history().unwrap();
    registry.execute(&mut doc, "SetObjectName Future").unwrap();
    doc.undo().unwrap();
    let before = doc.object(id).unwrap().clone();
    for command in [
        "InsertControlPoint Points 2,0,0 NaN,0,0",
        "InsertControlPoint Points Direction=Both 2,0,0",
        "InsertControlPoint Points Midpoint=Maybe 2,0,0",
        "InsertControlPoint Points Extend 2,0,0",
    ] {
        assert!(registry.execute(&mut doc, command).is_err(), "{command}");
        assert_eq!(doc.object(id), Some(&before));
        assert!(doc.can_redo());
        assert!(!doc.can_undo());
    }
    let draft = Draft::new(&doc, id).unwrap();
    registry.execute(&mut doc, "Hide").unwrap();
    assert!(draft.commit(&mut doc, false).is_err());
}

#[test]
fn insert_control_point_single_point_script_modifiers_match_native() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/insert_control_point_command.json"))
            .unwrap(),
    )
    .unwrap();
    let v = &capture["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "rational_midpoint")
        .unwrap()["value"];
    let (mut doc, _, _, _) = open_3dm_with_views(
        root.join("fixtures/insert_control_point_command/rational_midpoint.3dm")
            .to_str()
            .unwrap(),
    )
    .unwrap();
    let id: ObjectId = v["source_id"].as_str().unwrap().parse().unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    doc.clear_history().unwrap();
    let before = doc.object(id).unwrap().clone();
    let p = v["picks"][0].as_array().unwrap();
    let command = format!(
        "InsertControlPoint {:.17},{:.17},{:.17} Midpoint _Yes",
        p[0].as_f64().unwrap(),
        p[1].as_f64().unwrap(),
        p[2].as_f64().unwrap()
    );
    CommandRegistry::with_builtins()
        .execute(&mut doc, &command)
        .unwrap();
    let gap = topology::check(&doc, &v["after"], id, "rational_midpoint");
    assert!(gap <= 1e-8);
    assert_eq!(doc.undo_label(), Some("InsertControlPoint"));
    doc.undo().unwrap();
    assert_eq!(doc.object(id), Some(&before));
    eprintln!(
        "INSERT_CONTROL_POINT_SCRIPT_AUDIT {}",
        serde_json::json!({"id":"rational_midpoint","maximum_error":gap})
    );
}
