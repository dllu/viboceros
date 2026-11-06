use super::command_file::replay;
use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/line_reference.rs"]
#[allow(dead_code)]
pub(super) mod inputs;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn line_reference_getters_match_native_selection_cancellation_geometry_history_and_archives() {
    let sources = inputs::sources();
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in inputs::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let v = &row["value"];
            let source = &sources[id];
            let mut app = test_app();
            app.document = inputs::setup(v, source);
            app.viewports[0].set_construction_plane(native::frame(v));
            let parents = app.document.objects().cloned().collect::<Vec<_>>();
            let trace = &v["input_trace"][0];
            assert_eq!(trace["completed"], trace["expected"]);
            assert_eq!(trace["errors"], serde_json::json!([]));
            enter(&mut app, "Line");
            for action in source["actions"].as_array().unwrap() {
                assert!(app.line_input.is_some(), "{id} {:?}", app.command_log);
                if action.is_object() {
                    assert!(app.picking_line_curve());
                    let object =
                        inputs::guide(&app.document, action["curve"].as_u64().unwrap() as usize);
                    assert!(app.pick_line_reference_at(
                        object,
                        None,
                        native::point(&action["pick"])
                    ));
                } else {
                    let token = inputs::token(action);
                    let token = if action.is_array() {
                        format!("w{token}")
                    } else {
                        token
                    };
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
                }
                if app.active_command.is_some() {
                    assert_eq!(app.document.objects().len(), parents.len());
                }
            }
            assert!(app.active_command.is_none(), "{id} {:?}", app.command_log);
            let mut error = replay::check(&app.document, &v["after"], false);
            let mut archives = 0;
            assert_eq!(app.document.can_undo(), v["success"] == true);
            if app.document.can_undo() {
                assert_eq!(app.document.undo_label(), Some("Line"));
                app.document.undo().unwrap();
                error = error.max(replay::check(&app.document, &v["undo"], false));
                assert!(!app.document.can_undo());
                app.document.redo().unwrap();
                error = error.max(replay::check(&app.document, &v["redo"], false));
                let path = directory.path().join("extension.3dm");
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
            if v["pending_redo"].is_array() {
                assert!(app.document.can_redo());
                app.document.redo().unwrap();
                error = error.max(replay::check(&app.document, &v["pending_redo"], false));
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
                "LINE_REFERENCE_UI {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"archives":archives})
            );
            assert!(error <= 1e-6, "{id}: {error:e}");
            count += 1;
        }
    }
    assert_eq!(count, 36);
}

#[test]
fn physical_viewport_clicks_pick_the_reference_end_and_commit_the_same_preview() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    enter(&mut app, "Line 0,0,0 10,0,0");
    let parent = app.document.objects().next().unwrap().clone();
    app.document.clear_history().unwrap();
    let context = egui::Context::default();
    let frame = |app: &mut VibocerosApp, events: Vec<egui::Event>| {
        let mut output = ViewportOutput::default();
        let mut rect = egui::Rect::NOTHING;
        let picking = app.picking_line_curve();
        let input = ViewportInput {
            drafting: app.viewport_drafting_input(false, false),
            object_filter: app.viewport_object_filter(),
            line_input: app.line_input,
            translation_constraint: app.line_axis_constraint(),
            edge_pick: picking,
            curve_or_edge_pick: picking,
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
                    rect = ui.available_rect_before_wrap();
                    output = app.viewports[0].show(ui, &app.document, input, &[], 0, true);
                },
            )
            .drop_without_applying_deltas();
        (output, rect)
    };
    let click = |app: &mut VibocerosApp, pointer: egui::Pos2| {
        frame(app, vec![egui::Event::PointerMoved(pointer)]);
        frame(
            app,
            vec![egui::Event::PointerButton {
                pos: pointer,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        frame(
            app,
            vec![egui::Event::PointerButton {
                pos: pointer,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        )
        .0
    };
    let (_, rect) = frame(&mut app, vec![]);
    app.viewports[0].zoom_out(0.5).unwrap();
    enter(&mut app, "Line Extension");
    assert!(app.picking_line_curve());
    assert!(!app.viewport_drafting_input(false, false).active);
    let pointer = app.viewports[0].project(point(9., 0., 0.), rect).unwrap();
    let output = click(&mut app, pointer);
    assert!(output.selection_click.is_some());
    app.handle_viewport_action(output);
    assert!(!app.picking_line_curve());
    assert_eq!(app.line_input.unwrap().anchor(), Some(point(10., 0., 0.)));
    assert!(app.viewport_drafting_input(false, false).active);
    let pointer = app.viewports[0].project(point(14., 6., 0.), rect).unwrap();
    let (hover, _) = frame(&mut app, vec![egui::Event::PointerMoved(pointer)]);
    let preview = app
        .line_input
        .unwrap()
        .preview(hover.drafting_hover.unwrap())
        .unwrap();
    assert_eq!(app.document.objects().len(), 1);
    let output = click(&mut app, pointer);
    assert!(output.picked_point.is_some());
    app.handle_viewport_action(output);
    let Geometry::Line(actual) = app.document.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(*actual, preview.line);
    assert!(actual.end().distance_to(point(14., 0., 0.)).unwrap() < 1e-5);
    assert_eq!(app.document.object(parent.id()).unwrap(), &parent);
    assert!(
        parent.geometry_snapshot().shares_storage_with(
            app.document
                .object(parent.id())
                .unwrap()
                .geometry_snapshot()
        )
    );
    app.document.undo().unwrap();
    assert!(!app.document.can_undo());
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "Line Extension");
    let source_pointer = app.viewports[0].project(point(9., 0., 0.), rect).unwrap();
    let output = click(&mut app, source_pointer);
    app.handle_viewport_action(output);
    enter(&mut app, "5");
    let (hover, _) = frame(&mut app, vec![egui::Event::PointerMoved(pointer)]);
    app.handle_viewport_action(hover);
    let preview = app.line_input.unwrap().enter().unwrap();
    enter(&mut app, "");
    let Geometry::Line(actual) = app.document.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(*actual, preview.line);
    assert!(actual.end().distance_to(point(15., 0., 0.)).unwrap() < 1e-5);
    assert_eq!(app.last_point, Some(preview.accepted_point));
}

#[test]
fn reference_replacement_and_macro_input_preserve_atomic_geometry() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 10,0,0");
    let id = app.document.objects().next().unwrap().id();
    app.document.clear_history().unwrap();
    enter(&mut app, "Point 9,9,9");
    app.document.undo().unwrap();
    enter(&mut app, "Line Extension");
    assert!(app.pick_line_reference_at(id, None, point(9., 0., 0.)));
    app.document
        .replace_object_geometries([(
            id,
            Geometry::Line(
                viboceros_geometry::LineSegment::try_new(
                    point(0., 0., 0.),
                    point(20., 0., 0.),
                    Tolerance::NUMERICAL_VALIDATION,
                )
                .unwrap(),
            ),
        )])
        .unwrap();
    assert!(!app.accept_drafting_point(point(15., 0., 0.)));
    assert!(app.picking_line_curve());
    assert_eq!(app.document.objects().len(), 1);
    assert!(app.line_reference.as_ref().unwrap().source.is_none());
    enter(&mut app, "Cancel");
    app.document.clear_history().unwrap();
    let parent = app.document.object(id).unwrap().clone();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Reference input.txt");
    std::fs::write(
        &path,
        format!("_Line _Extension Curve={id} w19,0,0 5 w25,8,0\n_Point w7,8,9\n"),
    )
    .unwrap();
    enter(&mut app, &format!("ReadCommandFile \"{}\"", path.display()));
    for _ in 0..100 {
        if !app.drain_command_file() {
            break;
        }
    }
    assert_eq!(
        app.last_command_file_result,
        Some(true),
        "{:?}",
        app.command_log
    );
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.undo_label(), Some("ReadCommandFile"));
    let Geometry::Line(line) = app.document.objects().nth(1).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(line.start(), point(20., 0., 0.));
    assert_eq!(line.end(), point(25., 0., 0.));
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
    assert_eq!(app.document.object(id).unwrap(), &parent);
    app.document.redo().unwrap();
    assert_eq!(app.document.objects().len(), 3);
}

#[test]
fn transparent_cplane_input_keeps_reference_picking_and_numeric_enter_cursor_separate() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 10,0,0");
    let id = app.document.objects().next().unwrap().id();
    app.document.clear_history().unwrap();
    enter(&mut app, "Line Extension");
    enter(&mut app, "CPlane 3Point");
    assert!(app.plane_prompt.is_some());
    assert!(!app.picking_line_curve());
    assert!(app.viewport_drafting_input(false, false).active);
    for text in ["w0,0,0", "w1,0,0", "w0,1,0"] {
        enter(&mut app, text);
    }
    assert!(app.picking_line_curve());
    assert!(app.pick_line_reference_at(id, None, point(9., 0., 0.)));
    enter(&mut app, "5");
    enter(&mut app, "CPlane 3Point");
    assert!(app.line_axis_constraint().is_none());
    app.handle_viewport_action(ViewportOutput {
        drafting_hover: Some(point(20., 5., 0.)),
        ..Default::default()
    });
    for text in ["w10,20,30", "w10,21,30", "w10,20,31"] {
        enter(&mut app, text);
    }
    assert!(app.plane_prompt.is_none());
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    assert!(app.line_accepted_point.is_none());
    let Geometry::Line(line) = app.document.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(line.start(), point(10., 0., 0.));
    assert_eq!(line.end(), point(5., 0., 0.));
    assert_eq!(app.last_point, Some(point(5., 0., 0.)));
    app.document.undo().unwrap();
    assert!(!app.document.can_undo());
    assert_eq!(app.document.objects().len(), 1);
}

#[test]
fn constrained_lengths_parse_model_units_and_calculator_before_axis_projection() {
    for value in ["2+3", "0.5cm", "5mm"] {
        let mut app = test_app();
        enter(&mut app, "Units Millimeters");
        enter(&mut app, "Line 0,0,0 10,0,0");
        let id = app.document.objects().next().unwrap().id();
        app.document.clear_history().unwrap();
        enter(&mut app, "Line Extension");
        assert!(app.pick_line_reference_at(id, None, point(9., 0., 0.)));
        enter(&mut app, value);
        assert!(app.point_constraint.is_none());
        enter(&mut app, "w30,10,0");
        let Geometry::Line(line) = app.document.objects().last().unwrap().geometry() else {
            panic!()
        };
        assert_eq!(line.start(), point(10., 0., 0.));
        assert_eq!(line.end(), point(15., 0., 0.), "{value}");
        assert_eq!(app.document.undo_label(), Some("Line"));
    }
    let mut app = test_app();
    enter(&mut app, "Line Angled");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w10,0,0");
    enter(&mut app, "30+15");
    assert!(app.point_constraint.is_none());
    assert!(!app.line_input.unwrap().requests_angle());
    enter(&mut app, "w10,10,0");
    let Geometry::Line(line) = app.document.objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(line.end().distance_to(point(10., 10., 0.)).unwrap() < 1e-12);
}
