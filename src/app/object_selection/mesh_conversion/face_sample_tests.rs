use super::*;
use crate::app::crease_native_tests::fixture as native;
use crate::app::tests::test_app;
use crate::viewport::{
    ComponentPickFilter, DisplayMode, ViewKind, Viewport, ViewportInput, ViewportOutput,
    ZoomExtentsBorders,
};
use egui::{Pos2, Rect, Vec2};
use serde_json::Value;
#[path = "../../../../crates/viboceros-command/tests/support/nonplanar_sample.rs"]
mod fixtures;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn frame(
    context: &egui::Context,
    view: &mut Viewport,
    document: &Document,
    events: Vec<egui::Event>,
) -> (ViewportOutput, Rect) {
    let mut output = ViewportOutput::default();
    let mut rect = Rect::NOTHING;
    context
        .run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                events,
                ..Default::default()
            },
            |ui| {
                rect = ui.available_rect_before_wrap();
                output = view.show(
                    ui,
                    document,
                    ViewportInput {
                        component_pick: Some(ComponentPickFilter::RawMeshFaces),
                        ..Default::default()
                    },
                    &[],
                    0,
                    true,
                );
            },
        )
        .drop_without_applying_deltas();
    (output, rect)
}
fn click(document: &Document, target: viboceros_geometry::Point3) -> ViewportOutput {
    let mut view = Viewport::new(ViewKind::Top);
    view.display_mode = DisplayMode::Shaded;
    let context = egui::Context::default();
    let (_, rect) = frame(&context, &mut view, document, vec![]);
    view.zoom_extents(document, ZoomExtentsBorders::default())
        .unwrap();
    let pointer = view.project(target, rect).unwrap();
    let event = |pressed| egui::Event::PointerButton {
        pos: pointer,
        pressed,
        button: egui::PointerButton::Primary,
        modifiers: egui::Modifiers::NONE,
    };
    frame(
        &context,
        &mut view,
        document,
        vec![egui::Event::PointerMoved(pointer), event(true)],
    );
    frame(&context, &mut view, document, vec![event(false)]).0
}

#[test]
fn native_face_sampling_matches_staged_values_geometry_selection_and_history() {
    for (family, rows) in fixtures::groups() {
        let mut app = test_app();
        for row in rows {
            eprintln!("Face sampler {}", row["id"]);
            let value = &row["value"];
            let (doc, ids) = native::setup_from_family(&row, family);
            app.document = doc;
            let defaults = app
                .commands
                .triangulate_nonplanar_quads_options(&[])
                .unwrap();
            let expected = fixtures::native_options(value, defaults);
            let sources = ids
                .iter()
                .map(|id| {
                    app.document
                        .object(*id)
                        .unwrap()
                        .geometry_snapshot()
                        .clone()
                })
                .collect::<Vec<_>>();
            enter(&mut app, "TriangulateNonPlanarQuads");
            assert_eq!(app.document.undo_label(), Some("Undo witness"));
            for (id, source) in ids.iter().zip(&sources) {
                assert!(
                    app.document
                        .object(*id)
                        .unwrap()
                        .geometry_snapshot()
                        .shares_storage_with(source)
                );
            }
            let actions = value["input"]["actions"].as_array().unwrap();
            if actions.is_empty()
                && let Some(arguments) = value["input"]["command_arguments"]
                    .as_str()
                    .filter(|s| !s.is_empty())
            {
                enter(&mut app, arguments);
            }
            let (mut distance_error, mut angle_error) = (0f64, 0f64);
            for (index, action) in actions.iter().enumerate() {
                let final_action = index + 1 == actions.len();
                if final_action {
                    let Settings::Nonplanar(options) = app
                        .object_prompt
                        .as_ref()
                        .unwrap()
                        .mesh_conversion
                        .as_ref()
                        .unwrap()
                        .options
                    else {
                        panic!()
                    };
                    (distance_error, angle_error) = fixtures::option_errors(options, expected);
                }
                if action.is_object() {
                    assert!(app.picking_nonplanar_threshold());
                    let output = click(&app.document, fixtures::pointer(value, index));
                    let picks = &output
                        .component_click
                        .as_ref()
                        .expect("quad face click")
                        .picks;
                    assert_eq!(picks.len(), 1);
                    assert_eq!(
                        picks[0].object,
                        ids[value["input"]["sample_source"].as_u64().unwrap_or(0) as usize]
                    );
                    assert_eq!(
                        picks[0].index,
                        value["input"]["sample_face"].as_u64().unwrap() as usize
                    );
                    app.handle_viewport_action(output);
                } else if let Some(text) = action.as_str() {
                    if text == "Cancel" {
                        app.cancel_current_prompt_or_selection();
                    } else {
                        enter(&mut app, if text == "Enter" { "" } else { text });
                    }
                } else {
                    enter(&mut app, &action.as_f64().unwrap().to_string());
                }
                if !final_action {
                    assert!(app.object_prompt.is_some(), "{:?}", app.command_log);
                    assert_eq!(app.document.undo_label(), Some("Undo witness"));
                    for (id, source) in ids.iter().zip(&sources) {
                        assert!(
                            app.document
                                .object(*id)
                                .unwrap()
                                .geometry_snapshot()
                                .shares_storage_with(source)
                        );
                    }
                }
            }
            if actions.is_empty() {
                enter(&mut app, "");
            }
            assert!(app.object_prompt.is_none(), "{:?}", app.command_log);
            let actual = app
                .commands
                .triangulate_nonplanar_quads_options(&[])
                .unwrap();
            let errors = fixtures::option_errors(
                actual,
                if value["success"] == true {
                    expected
                } else {
                    defaults
                },
            );
            distance_error = distance_error.max(errors.0);
            angle_error = angle_error.max(errors.1);
            let mut maximum = native::mesh_command::check(&app.document, &ids, value, "after");
            app.document.undo().unwrap();
            maximum = maximum.max(native::mesh_command::check(
                &app.document,
                &ids,
                value,
                "undo",
            ));
            app.document.redo().unwrap();
            maximum = maximum.max(native::mesh_command::check(
                &app.document,
                &ids,
                value,
                "redo",
            ));
            println!(
                "FACE_SAMPLER {}",
                fixtures::report(&row, actual, maximum, distance_error, angle_error)
            );
        }
    }
}

#[test]
fn face_sampler_rejects_stale_parents_and_preserves_redo_on_nested_escape() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let data: Value = serde_json::from_str(
        &std::fs::read_to_string(
            root.join("tools/rhino_oracle/observations/nonplanar_sample_shaded.json"),
        )
        .unwrap(),
    )
    .unwrap();
    for stale in [false, true] {
        let row = &data["results"][0];
        let (doc, ids) = native::setup_from_family(row, "nonplanar_sample_shaded");
        let mut app = test_app();
        app.document = doc;
        app.commands
            .execute(&mut app.document, "Point 9,9,9")
            .unwrap();
        app.document.undo().unwrap();
        enter(&mut app, "TriangulateNonPlanarQuads");
        enter(&mut app, "Distance");
        if stale {
            app.document
                .set_tolerance(Tolerance::try_new(0.01, 1e-12, 1e-6).unwrap());
        }
        let redo = app.document.redo_label().map(str::to_owned);
        let undo = app.document.undo_label().map(str::to_owned);
        let output = click(&app.document, fixtures::pointer(&row["value"], 8));
        app.handle_viewport_action(output);
        if stale {
            assert!(app.object_prompt.is_none());
        } else {
            assert!(app.object_prompt.is_some());
            enter(&mut app, "Angle");
            app.cancel_current_prompt_or_selection();
            assert!(app.object_prompt.is_some());
            assert!(!app.picking_nonplanar_threshold());
            app.cancel_current_prompt_or_selection();
            assert!(app.object_prompt.is_none());
        }
        assert_eq!(app.document.redo_label(), redo.as_deref());
        assert_eq!(app.document.undo_label(), undo.as_deref());
        assert!(app.document.object(ids[0]).is_some());
    }
}
