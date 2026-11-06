use super::command_file::replay;
use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/line_construction.rs"]
mod inputs;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

#[test]
fn line_getters_match_native_construction_cancel_history_and_archives() {
    let sources = inputs::sources();
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in inputs::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            if inputs::DIAGNOSTICS.contains(&id) {
                continue;
            }
            let v = &row["value"];
            let mut app = test_app();
            app.document = inputs::setup(v);
            app.viewports[0].set_construction_plane(native::frame(v));
            let parents = app.document.objects().cloned().collect::<Vec<_>>();
            let is_macro = sources[id]["macro"].is_string();
            if let Some(text) = sources[id]["macro"].as_str() {
                let path = directory.path().join("Line macro.txt");
                std::fs::write(&path, text).unwrap();
                enter(&mut app, &format!("ReadCommandFile \"{}\"", path.display()));
                for _ in 0..100 {
                    if !app.drain_command_file() {
                        break;
                    }
                }
                assert!(app.command_file_job.is_none());
                assert_eq!(
                    app.last_command_file_result,
                    Some(v["success"].as_bool().unwrap()),
                    "{id} {:?}",
                    app.command_log
                );
            } else {
                let trace = &v["input_trace"][0];
                assert_eq!(trace["completed"], trace["expected"]);
                assert_eq!(trace["errors"], serde_json::json!([]));
                enter(&mut app, "Line");
                for action in sources[id]["actions"].as_array().unwrap() {
                    assert!(
                        app.line_input.is_some(),
                        "{id} {action} {:?}",
                        app.command_log
                    );
                    let token = inputs::token(action, true);
                    enter(
                        &mut app,
                        if token == "Enter" {
                            ""
                        } else if token == "Cancel" {
                            "!"
                        } else {
                            &token
                        },
                    );
                    if app.active_command.is_some() {
                        assert_eq!(app.document.objects().len(), parents.len());
                    }
                }
            }
            assert!(app.active_command.is_none(), "{id} {:?}", app.command_log);
            let comparison = replay::compare(&app.document, &v["after"], false);
            assert_eq!(
                comparison.native_valid,
                !inputs::NATIVE_INVALID.contains(&id)
            );
            let check = |d: &Document, target: &serde_json::Value, archive| {
                if comparison.native_valid {
                    replay::check(d, target, archive)
                } else {
                    replay::compare(d, target, archive).error
                }
            };
            let mut error = comparison.error;
            assert_eq!(app.document.can_undo(), v["before"] != v["after"], "{id}");
            let mut archives = 0;
            if app.document.can_undo() {
                assert_eq!(
                    app.document.undo_label(),
                    Some(if is_macro { "ReadCommandFile" } else { "Line" })
                );
                app.document.undo().unwrap();
                error = error.max(check(&app.document, &v["undo"], false));
                assert!(!app.document.can_undo());
                app.document.redo().unwrap();
                error = error.max(check(&app.document, &v["redo"], false));
                if comparison.native_valid {
                    let path = directory.path().join("line.3dm");
                    app.commands
                        .execute(&mut app.document, &format!("SaveAs {}", path.display()))
                        .unwrap();
                    let mut restored = Document::new(app.document.tolerance());
                    app.commands
                        .execute(&mut restored, &format!("Open3dm {}", path.display()))
                        .unwrap();
                    error = error.max(replay::check(&restored, &v["after"], true));
                    archives = 1;
                }
            }
            for parent in parents {
                let now = app.document.object(parent.id()).unwrap();
                assert_eq!(now, &parent);
                assert!(
                    parent
                        .geometry_snapshot()
                        .shares_storage_with(now.geometry_snapshot())
                );
            }
            println!(
                "LINE_UI {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"native_valid":comparison.native_valid,"archives":archives})
            );
            assert!(error <= 1e-6, "{id}: {error:e}");
            count += 1;
        }
    }
    assert_eq!(count, 54);
}

#[test]
fn actual_viewport_click_uses_vertical_axis_with_edge_on_cplane_and_same_preview() {
    for kind in [ViewKind::Front, ViewKind::Perspective] {
        let mut app = test_app();
        app.viewports[0] = Viewport::new(kind);
        app.viewports[0]
            .set_construction_plane(viboceros_command::construction_plane::WorldPlane::Top.frame());
        enter(&mut app, "Line BothSides Vertical");
        enter(&mut app, "w1,2,3");
        enter(&mut app, "5");
        let context = egui::Context::default();
        let pointer = egui::pos2(510., 210.);
        let frame = |app: &mut VibocerosApp, events: Vec<egui::Event>| {
            let mut output = ViewportOutput::default();
            let input = ViewportInput {
                drafting: DraftingInput {
                    active: true,
                    anchor: app.line_input.unwrap().anchor(),
                    ..Default::default()
                },
                line_input: app.line_input,
                translation_constraint: app.line_axis_constraint(),
                ..Default::default()
            };
            context
                .run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(800., 600.),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        output = app.viewports[0].show(ui, &app.document, input, &[], 0, true);
                    },
                )
                .drop_without_applying_deltas();
            output
        };
        frame(&mut app, vec![]);
        let hover = frame(&mut app, vec![egui::Event::PointerMoved(pointer)])
            .drafting_hover
            .unwrap();
        let preview = app.line_input.unwrap().preview(hover).unwrap();
        assert!(app.document.objects().next().is_none());
        let press = egui::Event::PointerButton {
            pos: pointer,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&mut app, vec![press]);
        let output = frame(
            &mut app,
            vec![egui::Event::PointerButton {
                pos: pointer,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(output.picked_point.is_some(), "{kind:?}");
        app.handle_viewport_action(output);
        assert!(app.active_command.is_none());
        let Geometry::Line(actual) = app.document.objects().next().unwrap().geometry() else {
            panic!()
        };
        assert_eq!(*actual, preview.line);
        assert_eq!(app.last_point, Some(preview.accepted_point));
        assert_eq!(actual.start().x(), 1.);
        assert_eq!(actual.end().y(), 2.);
        assert!((actual.length().unwrap() - 10.).abs() < 1e-10);
        app.document.undo().unwrap();
        assert!(!app.document.can_undo());
    }
}

#[test]
fn line_projected_last_points_relative_inputs_transparent_planes_and_redo_stay_coherent() {
    let mut app = test_app();
    enter(&mut app, "Point 9,9,9");
    enter(&mut app, "Undo");
    enter(&mut app, "Line FourPoint");
    for s in ["w0,0,0", "w4,0,0", "w1,2,3"] {
        enter(&mut app, s);
    }
    assert_eq!(app.last_point, Some(point(1., 0., 0.)));
    assert!(app.document.objects().next().is_none());
    assert!(app.document.can_redo());
    enter(&mut app, "r3,6,5");
    assert_eq!(app.last_point, Some(point(4., 0., 0.)));
    let Geometry::Line(line) = app.document.objects().next().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(line.start(), point(1., 0., 0.));
    assert_eq!(line.end(), point(4., 0., 0.));
    app.document.undo().unwrap();
    enter(&mut app, "Line Vertical");
    enter(&mut app, "w1,2,3");
    enter(&mut app, "CPlane 3Point");
    for s in ["w10,20,30", "w10,21,30", "w10,20,31"] {
        enter(&mut app, s);
    }
    assert!(app.line_input.is_some());
    assert!(app.document.objects().next().is_none());
    enter(&mut app, "Cancel");
    assert!(app.line_input.is_none());
    assert!(app.document.can_redo());
    assert!(!app.document.can_undo());
}
