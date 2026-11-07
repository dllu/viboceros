use super::blend_curve::blend;
use super::blend_curve_points::points;
use super::blend_curve_symmetry::measurements;
use super::line_reference::inputs as reference_inputs;
use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/blend_curve_empty.rs"]
pub(super) mod empty;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn empty_retained_intervals_ui_match_native_geometry_ownership_and_history() {
    let mut count = 0;
    for family in empty::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let value = &row["value"];
            let mut app = test_app();
            app.document = reference_inputs::setup(value, &value["prescribed_case"]);
            let before = empty::State::capture(&app.document, value);
            app.commands
                .remember_blend_curve_preferences(points::preferences(value));
            enter(&mut app, "BlendCrv");
            for (index, action) in value["prescribed_case"]["actions"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                if let Some(i) = action["curve"].as_u64() {
                    let input = &app.blend_curve_session.as_ref().unwrap().input;
                    let point = blend::source_pick(value, index, action, input);
                    app.pick_blend_curve_at(
                        reference_inputs::guide(&app.document, i as usize),
                        None,
                        point,
                    );
                } else if action.get("click").is_some() {
                    let input = &app.blend_curve_session.as_ref().unwrap().input;
                    let handle = blend::handle_at_click(input, value, index);
                    app.select_blend_curve_handle(handle);
                } else {
                    let token = reference_inputs::token(action);
                    let token = if action.is_array() {
                        format!("w{token}")
                    } else {
                        token
                    };
                    enter(&mut app, if token == "Enter" { "" } else { &token });
                }
                if app.active_command.is_some() {
                    assert!(
                        !app.document.can_undo(),
                        "{}: {:?}",
                        row["id"],
                        app.command_log
                    );
                }
            }
            assert!(
                app.active_command.is_none(),
                "{}: {:?}",
                row["id"],
                app.command_log
            );
            let mut metrics = before.verify(&mut app.document, value);
            metrics["id"] = serde_json::json!(format!("{family}/{}", row["id"].as_str().unwrap()));
            println!("BLEND_CURVE_EMPTY_UI {metrics}");
            count += 1;
        }
    }
    assert_eq!(count, empty::CASES);
}
