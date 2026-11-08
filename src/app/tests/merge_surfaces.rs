use super::boolean_intersection::{enter, pick};
use super::*;
use crate::app::object_selection::ObjectPromptPhase;
#[path = "../../../crates/viboceros-command/tests/support/merge_surfaces.rs"]
#[allow(dead_code)]
pub(super) mod captured;
#[path = "../../../crates/viboceros-command/tests/support/merge_surface_command.rs"]
pub(super) mod workflow;

#[test]
fn merge_surfaces_selection_getters_cancellation_and_history_match_native() {
    let mut count = 0;
    for family in workflow::families() {
        let mut app = test_app();
        for row in family {
            let id = row["id"].as_str().unwrap();
            let v = &row["value"];
            println!("MERGE_SURFACE_UI_CASE {id}");
            app.document = workflow::setup(v);
            let state = workflow::State::capture(&app.document);
            workflow::check(&app.document, &v["before"], false);
            let original = app.document.objects().cloned().collect::<Vec<_>>();
            let expected_options =
                workflow::options(v, app.commands.merge_surface_options(), &app.document);
            enter(&mut app, "_-MergeSrf");
            for (index, action) in v["prescribed_case"]["actions"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                let prompt = v["input_trace"][0]["prompts"][index].as_str().unwrap();
                let pending = app
                    .object_prompt
                    .as_ref()
                    .unwrap_or_else(|| panic!("{id}: {prompt}: {:?}", app.command_log));
                let field = if prompt.starts_with("Joining tolerance") {
                    Some("Tolerance")
                } else if prompt.starts_with("Roundness") {
                    Some("Roundness")
                } else {
                    None
                };
                assert_eq!(
                    matches!(pending.phase, ObjectPromptPhase::MergeSurfaceValue(_)),
                    field.is_some(),
                    "{id}: {prompt}"
                );
                let options = app.commands.merge_surface_options();
                if let Some(field) = field {
                    assert_eq!(pending.phase, ObjectPromptPhase::MergeSurfaceValue(field));
                    let default = prompt
                        .split('<')
                        .nth(1)
                        .unwrap()
                        .split('>')
                        .next()
                        .unwrap()
                        .parse::<f64>()
                        .unwrap();
                    assert_eq!(
                        default,
                        if field == "Tolerance" {
                            options
                                .effective_tolerance(app.document.tolerance())
                                .unwrap()
                        } else {
                            options.roundness
                        },
                        "{id}"
                    );
                } else {
                    assert_eq!(pending.phase, ObjectPromptPhase::Selecting);
                    let native_value = |name: &str| {
                        prompt
                            .split_whitespace()
                            .find_map(|token| token.strip_prefix(&format!("{name}=")))
                            .unwrap()
                    };
                    assert_eq!(
                        native_value("Smooth"),
                        if options.smooth { "Yes" } else { "No" }
                    );
                    assert_eq!(
                        native_value("Tolerance").parse::<f64>().unwrap(),
                        options
                            .effective_tolerance(app.document.tolerance())
                            .unwrap(),
                        "{id}"
                    );
                    assert_eq!(
                        native_value("Roundness").parse::<f64>().unwrap(),
                        options.roundness,
                        "{id}"
                    );
                }
                if action == "Cancel" {
                    app.cancel_current_prompt_or_selection();
                } else if action == "Enter" {
                    enter(&mut app, "");
                } else if let Some(i) = action["curve"].as_u64() {
                    let id = workflow::source(&app.document, i as usize);
                    pick(&mut app, id);
                } else if let Some(s) = action["number"].as_str() {
                    enter(&mut app, s);
                } else if let Some(s) = action.as_str() {
                    enter(&mut app, s);
                } else {
                    enter(&mut app, &action.to_string());
                }
                if !app.command_line_idle() {
                    assert_eq!(
                        app.document.objects().cloned().collect::<Vec<_>>(),
                        original,
                        "{id}: premature edit"
                    );
                    assert!(!app.document.can_undo());
                    assert!(app.document.can_redo());
                }
            }
            assert!(app.command_line_idle(), "{id}: {:?}", app.command_log);
            assert_eq!(
                app.commands.merge_surface_options(),
                expected_options,
                "{id}"
            );
            let mut record = state.verify(&mut app.document, v);
            record["id"] = id.into();
            println!("MERGE_SURFACE_UI {record}");
            count += 1;
        }
    }
    assert_eq!(count, 20);
}

#[test]
fn merge_surface_scalar_getter_owns_clicks_and_empty_enter_keeps_value() {
    let mut app = test_app();
    app.document = workflow::setup(&workflow::families()[0][1]["value"]);
    enter(&mut app, "MergeSrf");
    enter(&mut app, "Roundness");
    let id = workflow::source(&app.document, 0);
    pick(&mut app, id);
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(
        app.object_prompt.as_ref().unwrap().phase,
        ObjectPromptPhase::MergeSurfaceValue("Roundness")
    );
    enter(&mut app, "");
    assert_eq!(app.commands.merge_surface_options().roundness, 1.);
    enter(&mut app, "Roundness=1/4");
    enter(&mut app, "Tolerance");
    for invalid in ["NaN", "-1", "0", "0.0000005"] {
        enter(&mut app, invalid);
        assert_eq!(
            app.object_prompt.as_ref().unwrap().phase,
            ObjectPromptPhase::MergeSurfaceValue("Tolerance")
        );
    }
    app.cancel_current_prompt_or_selection();
    assert!(app.command_line_idle());
    assert_eq!(app.commands.merge_surface_options().roundness, 0.25);
    assert!(!app.document.can_undo());
    assert!(app.document.can_redo());
}
