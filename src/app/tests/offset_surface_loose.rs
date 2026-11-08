use super::boolean_intersection::{enter, pick};
use super::offset_surface::inputs;
use super::{brep_comparison, plane_surface_native_support as native, *};
#[path = "../../../crates/viboceros-command/tests/support/offset_surface_loose.rs"]
#[allow(dead_code)]
pub(super) mod loose;

#[test]
fn loose_offsets_replay_native_getters_previews_cancel_and_admission() {
    let mut count = 0;
    for (id, v) in loose::rows() {
        if loose::diagnostic(&id, &v) {
            continue;
        }
        println!("LOOSE_OFFSET_UI_CASE {id}");
        let mut app = test_app();
        app.document = loose::setup(&v);
        app.commands
            .remember_offset_surface_options(inputs::initial(&v))
            .unwrap();
        let state = inputs::State::capture(&app.document);
        let before = loose::check(&app.document, &v["before"], false);
        let original = app.document.objects().cloned().collect::<Vec<_>>();
        enter(&mut app, "OffsetSrf");
        for action in v["prescribed_case"]["actions"].as_array().unwrap() {
            if action == "Cancel" {
                app.cancel_current_prompt_or_selection();
            } else if action == "Enter" {
                enter(&mut app, "");
            } else if let Some(i) = action.get("curve").and_then(serde_json::Value::as_u64) {
                let id = inputs::guide(&app.document, i as usize);
                pick(&mut app, id);
            } else if let Some(n) = action.as_f64() {
                enter(&mut app, &n.to_string());
            } else if let Some(n) = action.get("number").and_then(serde_json::Value::as_str) {
                enter(&mut app, n);
            } else {
                enter(&mut app, action.as_str().unwrap());
            }
            if !app.command_line_idle() {
                assert_eq!(
                    app.document.objects().cloned().collect::<Vec<_>>(),
                    original,
                    "{id}: preview mutated the document"
                );
                assert!(!app.document.can_undo(), "{id}");
                assert!(app.document.can_redo(), "{id}");
            }
        }
        assert!(app.command_line_idle(), "{id}: {:?}", app.command_log);
        let mut record = state.verify_with(&mut app.document, &v, loose::check);
        record["id"] = id.into();
        record["maximum_geometry_error"] = before
            .max(record["maximum_geometry_error"].as_f64().unwrap())
            .into();
        println!("LOOSE_OFFSET_UI {record}");
        count += 1;
    }
    println!("LOOSE_OFFSET_UI_COUNT {count}");
}
