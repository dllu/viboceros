use super::blend_curve::blend;
use super::blend_curve_edges::edges;
use super::line_reference::inputs as reference_inputs;
use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/blend_curve_display.rs"]
mod display;
use super::blend_curve_empty::empty;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn transient_display_ui_matches_native_curvature_analysis_lifecycle_and_history() {
    let mut count = 0;
    for family in display::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let id = format!("{family}/{}", row["id"].as_str().unwrap());
            println!("DISPLAY_CASE {id}");
            let value = &row["value"];
            let mut app = test_app();
            app.document = display::setup(&app.commands, value);
            app.commands
                .remember_blend_curve_preferences(edges::preferences(value));
            let options = app.commands.curvature_graph_options();
            let state = empty::State::capture(&app.document, value);
            let before = display::curvature(&app.document, &value["before"], &options);
            enter(&mut app, "BlendCrv");
            for (index, action) in value["prescribed_case"]["actions"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                if action.get("view_snapshot").is_some() {
                    display::snapshot(
                        &app.blend_curve_session.as_ref().unwrap().input,
                        &app.document,
                        &app.commands,
                        value,
                        index,
                    );
                } else if action.get("wait_prompt").is_some() {
                    continue;
                } else if action.get("transparent").is_some() {
                    enter(&mut app, "'_CurvatureGraphOff");
                    assert!(app.active_command.is_some(), "{id}: {:?}", app.command_log);
                } else if let Some(i) = action["curve"].as_u64() {
                    let input = &app.blend_curve_session.as_ref().unwrap().input;
                    let point = blend::source_pick(value, index, action, input);
                    app.pick_blend_curve_at(
                        reference_inputs::guide(&app.document, i as usize),
                        blend::source_edge(value, index, action),
                        point,
                    );
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
                    assert!(!app.document.can_undo(), "{id}");
                }
            }
            assert!(app.active_command.is_none(), "{id}: {:?}", app.command_log);
            assert!(app.blend_curvature_display().is_none());
            display::completed(&app.document, &app.commands, value);
            let after = display::curvature(&app.document, &value["after"], &options);
            let mut metrics =
                display::measurements(before, after, state.verify(&mut app.document, value));
            metrics["id"] = serde_json::json!(id);
            println!("BLEND_CURVE_DISPLAY_UI {metrics}");
            count += 1;
        }
    }
    assert_eq!(count, display::CASES);
}

#[test]
fn cached_hover_combs_follow_candidates_and_cancel_restores_the_accepted_preview() {
    use std::sync::Arc;
    use viboceros_command::{blend_curve::Handle, curvature_graph::Options};
    let (mut app, ids) = super::arc_blend::scene();
    enter(&mut app, "BlendCrv");
    app.pick_blend_curve_at(ids[0], None, Point3::try_new(0., 0., 0.).unwrap());
    app.pick_blend_curve_at(ids[1], None, Point3::try_new(4., 3., 0.).unwrap());
    enter(&mut app, "DisplayCurvature");
    let before_model = format!("{:?}", app.document);
    let original = app.blend_curvature_display().unwrap();
    let context = egui::Context::default();
    let output = context.run_ui(egui::RawInput::default(), |ui| app.show_curvature_graph(ui));
    output.drop_without_applying_deltas();
    assert_eq!(format!("{:?}", app.document), before_model);
    app.select_blend_curve_handle(Handle::StartShape);
    app.hover_blend_curve_parameter(3.);
    let moved = app.blend_curvature_display().unwrap();
    assert!(!Arc::ptr_eq(&original.data, &moved.data));
    assert!(Arc::ptr_eq(
        &moved.data,
        &app.blend_curvature_display().unwrap().data
    ));
    enter(&mut app, "Cancel");
    assert!(Arc::ptr_eq(
        &original.data,
        &app.blend_curvature_display().unwrap().data
    ));
    app.commands
        .configure_curvature_graph(
            &app.document,
            Options {
                scale: 110,
                color: [3, 211, 137],
                ..Options::default()
            },
        )
        .unwrap();
    assert!(Arc::ptr_eq(
        &original.data,
        &app.blend_curvature_display().unwrap().data
    ));
    app.set_curvature_graph_display("CurvatureGraphOff");
    assert!(app.active_command.is_some());
    assert!(app.blend_curvature_display().is_none());
    enter(&mut app, "DisplayCurvature");
    assert!(Arc::ptr_eq(
        &original.data,
        &app.blend_curvature_display().unwrap().data
    ));
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    assert!(app.blend_curvature_display().is_none());
    assert_eq!(app.document.undo_label(), Some("BlendCrv"));
    app.document.undo().unwrap();
    assert!(!app.document.can_undo());
}
