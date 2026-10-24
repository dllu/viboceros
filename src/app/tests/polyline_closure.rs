use super::polyline_closure_reference as reference;
use super::*;
use std::collections::BTreeSet;
use viboceros_document::{GripMode, SelectionMode};
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn native_closure_selection_and_control_display_use_the_same_seam_through_ui() {
    let data = reference::capture();
    let v = &data["results"][0]["value"];
    let mut app = test_app();
    app.document = viboceros_command::open_3dm_with_views(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tools/rhino_oracle/fixtures/polyline_closure.3dm"
    ))
    .unwrap()
    .0;
    app.document.clear_selection();
    app.document.clear_history().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    for name in ["SelClosedCrv", "SelOpenCrv"] {
        enter(&mut app, "SelNone");
        enter(&mut app, name);
        assert!(app.command_line_idle());
        let expected = v["queries"][name]["value"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            app.document
                .selected_object_ids()
                .map(|id| id.to_string())
                .collect::<BTreeSet<_>>(),
            expected,
            "{name}"
        );
        assert!(!app.document.can_undo());
    }
    for row in v["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["valid"] == true)
    {
        let name = row["recipe"]["id"].as_str().unwrap();
        let id = row["id"].as_str().unwrap().parse::<ObjectId>().unwrap();
        app.document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        enter(&mut app, "PointsOn");
        let points = app.document.control_point_locations(id).unwrap();
        assert!(
            reference::compare_points(points, &row["grips"], name, true) <= 1e-8,
            "{name}"
        );
        assert_eq!(
            app.document
                .object(id)
                .unwrap()
                .geometry()
                .grip_locations(GripMode::Control)
                .unwrap(),
            points
        );
        enter(&mut app, "PointsOff");
    }
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!app.document.can_undo());
}

#[test]
fn native_shared_seam_edits_and_noops_match_through_move_with_history() {
    let data = reference::capture();
    let rows = data["results"][0]["value"]["rows"].as_array().unwrap();
    let mut app = test_app();
    app.document = viboceros_command::open_3dm_with_views(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tools/rhino_oracle/fixtures/polyline_closure.3dm"
    ))
    .unwrap()
    .0;
    for row in rows.iter().filter(|r| r.get("move_grip").is_some()) {
        let name = row["recipe"]["id"].as_str().unwrap();
        let id = row["id"].as_str().unwrap().parse::<ObjectId>().unwrap();
        let source = app.document.object(id).unwrap().clone();
        app.document.clear_selection();
        app.document.enable_control_points([id]).unwrap();
        app.document
            .select_control_points(
                [viboceros_document::ControlPointId {
                    object: id,
                    index: row["recipe"]["grip_index"].as_u64().unwrap_or(0) as usize,
                }],
                SelectionMode::Replace,
            )
            .unwrap();
        app.document.clear_history().unwrap();
        enter(&mut app, "Move");
        enter(&mut app, "0,0,0");
        let v = row["recipe"]
            .get("move_vector")
            .cloned()
            .unwrap_or(serde_json::json!([1., 2., 3.]));
        enter(
            &mut app,
            &format!(
                "{},{},{}",
                v[0].as_f64().unwrap(),
                v[1].as_f64().unwrap(),
                v[2].as_f64().unwrap()
            ),
        );
        assert!(app.command_line_idle(), "{name}: {:?}", app.command_log);
        let expected = Geometry::from(reference::curve(&row["move_grip"]["value"]));
        assert_eq!(
            app.document.object(id).unwrap().geometry(),
            &expected,
            "{name}"
        );
        assert_eq!(
            app.document.object(id).unwrap().attributes(),
            source.attributes()
        );
        if expected == *source.geometry() {
            assert!(!app.document.can_undo(), "{name}: no-op");
        } else {
            let after = app.document.object(id).unwrap().clone();
            enter(&mut app, "Undo");
            assert_eq!(app.document.object(id), Some(&source), "{name}: undo");
            enter(&mut app, "Redo");
            assert_eq!(app.document.object(id), Some(&after), "{name}: redo");
            enter(&mut app, "Undo");
        }
        enter(&mut app, "PointsOff");
    }
}
