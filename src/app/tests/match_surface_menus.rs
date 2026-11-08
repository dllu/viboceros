use super::boolean_intersection::enter;
use super::merge_surfaces::{captured, workflow};
use super::*;

fn contact(app: &mut VibocerosApp, action: &serde_json::Value) {
    let name = format!("match-source-{}", action["curve"]);
    let object = app
        .document
        .objects()
        .find(|o| o.attributes().name() == Some(&name))
        .unwrap()
        .id();
    app.accept_match_contacts(vec![(
        crate::viewport::EdgePick {
            object,
            edge: action["edge"].as_u64().unwrap() as usize,
        },
        captured::point(&action["pick"]),
    )]);
}

#[test]
fn enum_named_curvature_remains_in_the_match_surface_getter() {
    let data = captured::read("match_surface_command_tolerance_getters");
    let row = data["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "match_tol_complete_menus")
        .unwrap();
    let v = &row["value"];
    let trace = &v["input_trace"][0];
    assert_eq!(trace["completed"], trace["expected"]);
    assert_eq!(trace["errors"], serde_json::json!([]));
    let mut app = test_app();
    app.document = workflow::setup_named(v, "match", "Match");
    let state = workflow::State::capture(&app.document);
    let original = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "_-MatchSrf");
    for action in v["prescribed_case"]["actions"].as_array().unwrap() {
        if action.get("curve").is_some() {
            contact(&mut app, action);
        } else {
            let text = action.as_str().unwrap();
            enter(&mut app, if text == "Enter" { "" } else { text });
        }
        if app.edge_prompt.is_some() {
            assert_eq!(
                app.document.objects().cloned().collect::<Vec<_>>(),
                original
            );
            assert!(!app.document.can_undo());
            assert!(app.document.can_redo());
        }
        if action == "Curvature" {
            assert_eq!(app.edge_prompt.as_ref().unwrap().name(), "MatchSrf");
        }
    }
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    let mut record = state.verify_command(&mut app.document, v, "MatchSrf");
    record["id"] = row["id"].clone();
    println!("MATCH_SURFACE_MENU_UI {record}");
}

#[test]
fn curvature_mode_choice_accepts_english_prefix_and_new_commands_still_cancel() {
    let data = captured::read("match_surface_command_pilot");
    let v = &data["results"][0]["value"];
    let mut app = test_app();
    app.document = workflow::setup_named(v, "match", "Match");
    enter(&mut app, "MatchSrf");
    for action in &v["prescribed_case"]["actions"].as_array().unwrap()[..2] {
        contact(&mut app, action);
    }
    enter(&mut app, "Mode");
    enter(&mut app, "_Curvature");
    assert_eq!(app.edge_prompt.as_ref().unwrap().name(), "MatchSrf");
    assert_eq!(
        app.commands.match_surface_options().policy.continuity,
        viboceros_geometry::SurfaceBlendContinuity::Curvature
    );
    assert!(!app.document.can_undo());
    assert!(app.document.can_redo());
    enter(&mut app, "Line");
    assert!(app.edge_prompt.is_none());
    assert!(!app.document.can_undo());
    assert!(app.document.can_redo());
    app.cancel_current_prompt_or_selection();
}
