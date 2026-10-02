//! Replay real viewport events from independent inputs against native mouse runs.
use super::*;
use crate::viewport::{
    ComponentPickFilter, DisplayMode, FacePickMode, ViewKind, Viewport, ViewportInput,
    ViewportOutput,
};
use egui::{PointerButton, Pos2, Rect, Vec2};
use serde_json::{Value, json};
use viboceros_oracle::{ExtractFixture, observe_component_document};

#[path = "../../../../crates/viboceros-oracle/src/test_json.rs"]
mod test_json;

fn frame(
    app: &VibocerosApp,
    view: &mut Viewport,
    context: &egui::Context,
    time: &mut f64,
    mut events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
) -> (ViewportOutput, Rect) {
    events.insert(0, egui::Event::ModifiersChanged(modifiers));
    let mut output = ViewportOutput::default();
    let mut area = Rect::NOTHING;
    *time += 0.25;
    context
        .run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200., 800.))),
                time: Some(*time),
                events,
                ..Default::default()
            },
            |ui| {
                area = ui.available_rect_before_wrap();
                output = view.show(
                    ui,
                    &app.document,
                    ViewportInput {
                        component_pick: app
                            .picking_extract_faces()
                            .then_some(ComponentPickFilter::Faces),
                        face_pick: app
                            .picking_extract_faces()
                            .then_some(FacePickMode::SurfaceAndBrepAny),
                        ..Default::default()
                    },
                    &[],
                    0,
                    true,
                );
            },
        )
        .drop_without_applying_deltas();
    (output, area)
}

fn component_record(app: &VibocerosApp, sources: &[ObjectId]) -> Value {
    let mut picks = app
        .component_selection
        .checked_picks(&app.document)
        .unwrap();
    picks.sort_by_key(|pick| {
        (
            sources.iter().position(|id| *id == pick.object).unwrap(),
            pick.index,
        )
    });
    json!(
        picks
            .into_iter()
            .map(|pick| json!([
                sources.iter().position(|id| *id == pick.object).unwrap(),
                "face",
                pick.index
            ]))
            .collect::<Vec<_>>()
    )
}

#[test]
fn extract_surface_viewport_sequences_geometry_metadata_and_history_match_native_mouse_runs() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/extract_srf_picking.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/extract_srf_picking.json"
    ))
    .unwrap();
    replay_viewport_sequences(request, observed, 32);
}

#[test]
fn extract_surface_curved_viewport_sequences_match_native_mouse_runs() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/extract_srf_curved_picking.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/extract_srf_curved_picking.json"
    ))
    .unwrap();
    replay_viewport_sequences(request, observed, 38);
}

fn replay_viewport_sequences(request: Value, observed: Value, count: usize) {
    let operations = request["operations"].as_array().unwrap();
    let results = observed["results"].as_array().unwrap();
    assert_eq!(operations.len(), count);
    assert_eq!(results.len(), operations.len());
    assert_eq!(observed["engine"], "rhino");
    for (operation, row) in operations.iter().zip(results) {
        let label = operation["id"].as_str().unwrap();
        assert_eq!(row["id"], label);
        let expected = &row["value"];
        let fixture: ExtractFixture = serde_json::from_value(operation.clone()).unwrap();
        let mut app = test_app();
        let prepared = fixture.prepare_document(app.document.tolerance()).unwrap();
        let sources = prepared.sources;
        let groups = prepared.groups;
        app.document = prepared.document;
        app.document.clear_history().unwrap();
        let snapshot = |app: &VibocerosApp| {
            json!(observe_component_document(&app.document, &sources, &groups).unwrap())
        };
        test_json::close(
            &json!(prepared.constructed),
            &expected["constructed"],
            label,
            1e-9,
            0.,
        );
        test_json::close(&snapshot(&app), &expected["before"], label, 1e-9, 0.);
        assert_eq!(
            component_record(&app, &sources),
            expected["component_selection"]["before"],
            "{label}"
        );
        enter(
            &mut app,
            &format!(
                "ExtractSrf Copy={} OutputLayer={}",
                if operation["copy"].as_bool().unwrap() {
                    "Yes"
                } else {
                    "No"
                },
                if operation["output_current"].as_bool().unwrap() {
                    "Current"
                } else {
                    "Input"
                }
            ),
        );
        assert!(app.picking_extract_faces(), "{label}");
        let view_kind = match operation["view"].as_str().unwrap_or("Top") {
            "Top" => ViewKind::Top,
            "Bottom" => ViewKind::Bottom,
            "Front" => ViewKind::Front,
            "Back" => ViewKind::Back,
            "Left" => ViewKind::Left,
            "Right" => ViewKind::Right,
            other => panic!("{label}: unsupported view {other}"),
        };
        let mut view = Viewport::new(view_kind);
        view.display_mode = match operation["display"].as_str().unwrap_or("Shaded") {
            "Shaded" => DisplayMode::Shaded,
            "Ghosted" => DisplayMode::Ghosted,
            "Wireframe" => DisplayMode::Wireframe,
            other => panic!("{label}: unsupported display {other}"),
        };
        let context = egui::Context::default();
        let mut time = 0.;
        frame(
            &app,
            &mut view,
            &context,
            &mut time,
            vec![],
            egui::Modifiers::NONE,
        );
        view.zoom_extents(&app.document, Default::default())
            .unwrap();
        let (_, area) = frame(
            &app,
            &mut view,
            &context,
            &mut time,
            vec![],
            egui::Modifiers::NONE,
        );
        let steps = operation["steps"].as_array().unwrap();
        assert_eq!(
            steps.len(),
            expected["selection_steps"].as_array().unwrap().len(),
            "{label}"
        );
        for (index, step) in steps.iter().enumerate() {
            if step["kind"] == "key" {
                enter(&mut app, step["value"].as_str().unwrap());
            } else {
                let modifier = step["modifiers"].as_str().unwrap();
                let modifiers = egui::Modifiers {
                    ctrl: matches!(modifier, "ctrl" | "sub"),
                    shift: matches!(modifier, "shift" | "sub"),
                    ..Default::default()
                };
                let points = if step["kind"] == "click" {
                    let source = step["component"][0].as_u64().unwrap() as usize;
                    let index = step["component"][1].as_u64().unwrap() as usize;
                    let Geometry::Brep(brep) =
                        app.document.object(sources[source]).unwrap().geometry()
                    else {
                        panic!()
                    };
                    let face = &brep.faces()[index];
                    let surface = face.surface();
                    let (u, v) = (surface.domain_u(), surface.domain_v());
                    let fraction = step["fraction"]
                        .as_array()
                        .map(|values| [values[0].as_f64().unwrap(), values[1].as_f64().unwrap()])
                        .unwrap_or([0.3, 0.3]);
                    let (u, v) = (
                        *u.start() + fraction[0] * (*u.end() - *u.start()),
                        *v.start() + fraction[1] * (*v.end() - *v.start()),
                    );
                    assert!(
                        face.contains_parameters(u, v, app.document.tolerance())
                            .unwrap()
                    );
                    vec![surface.evaluate(u, v).unwrap()]
                } else {
                    step["corners"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|point| {
                            Point3::try_from([
                                point[0].as_f64().unwrap(),
                                point[1].as_f64().unwrap(),
                                point[2].as_f64().unwrap(),
                            ])
                            .unwrap()
                        })
                        .collect()
                };
                let pixels = points
                    .into_iter()
                    .map(|point| view.project(point, area).unwrap())
                    .collect::<Vec<_>>();
                let button = |pos, pressed, modifiers| egui::Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers,
                };
                let (output, _) = frame(
                    &app,
                    &mut view,
                    &context,
                    &mut time,
                    vec![
                        egui::Event::PointerMoved(pixels[0]),
                        button(pixels[0], true, modifiers),
                    ],
                    modifiers,
                );
                assert!(output.component_click.is_none() && output.component_window.is_none());
                let end = *pixels.last().unwrap();
                if pixels.len() == 2 {
                    frame(
                        &app,
                        &mut view,
                        &context,
                        &mut time,
                        vec![egui::Event::PointerMoved(end)],
                        egui::Modifiers::NONE,
                    );
                }
                let (output, _) = frame(
                    &app,
                    &mut view,
                    &context,
                    &mut time,
                    vec![button(end, false, egui::Modifiers::NONE)],
                    egui::Modifiers::NONE,
                );
                assert!(
                    output.component_click.is_some() || output.component_window.is_some(),
                    "{label} step {index}"
                );
                app.handle_viewport_action(output);
            }
            assert_eq!(
                component_record(&app, &sources),
                expected["selection_steps"][index],
                "{label} step {index}"
            );
            assert!(
                !app.document.can_undo(),
                "{label}: geometry edited while picking"
            );
            // Compare complete geometry/metadata after every input, with no
            // expected pick injected into the application or viewport.
            test_json::close(&snapshot(&app), &expected["before"], label, 1e-9, 0.);
        }
        if steps.last().unwrap()["value"] == "None" {
            assert!(
                !app.picking_extract_faces(),
                "{label}: None must leave selection"
            );
            assert_eq!(expected["input_completion"], json!(["None"]));
        } else if operation["finish"] == "Cancel" {
            app.cancel_current_prompt_or_selection();
        } else {
            enter(&mut app, "");
        }
        assert!(!app.picking_extract_faces(), "{label}");
        assert_eq!(
            app.document.can_undo(),
            expected["succeeded"].as_bool().unwrap(),
            "{label}"
        );
        test_json::close(&snapshot(&app), &expected["after"], label, 1e-9, 0.);
        assert_eq!(
            component_record(&app, &sources),
            expected["component_selection"]["after"],
            "{label}"
        );
        if app.document.can_undo() {
            assert_eq!(app.document.undo_label(), Some("ExtractSrf"));
            enter(&mut app, "Undo");
            test_json::close(&snapshot(&app), &expected["undo"], label, 1e-9, 0.);
            assert_eq!(
                component_record(&app, &sources),
                expected["component_selection"]["undo"],
                "{label}"
            );
            assert!(!app.document.can_undo());
            enter(&mut app, "Redo");
            test_json::close(&snapshot(&app), &expected["redo"], label, 1e-9, 0.);
            assert_eq!(
                component_record(&app, &sources),
                expected["component_selection"]["redo"],
                "{label}"
            );
        }
    }
}
