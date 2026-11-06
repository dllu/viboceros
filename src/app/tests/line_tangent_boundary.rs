use super::command_file::replay;
use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/line_tangent_arcs.rs"]
#[allow(dead_code)]
mod inputs;
use super::line_tangent::inputs as tangent_inputs;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn bounded_tangent_getters_match_native_selection_geometry_history_and_archives() {
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
            if inputs::excluded(id) {
                continue;
            }
            assert_eq!(trace["completed"], trace["expected"]);
            assert_eq!(trace["errors"], serde_json::json!([]));
            enter(&mut app, "Line");
            app.planar = source["planar"] == true;
            for (i, action) in source["actions"].as_array().unwrap().iter().enumerate() {
                assert!(app.active_command.is_some(), "{id}: {:?}", app.command_log);
                if action.get("hover").is_some() {
                    continue;
                }
                if action.get("curve").is_some() {
                    let object =
                        inputs::guide(&app.document, action["curve"].as_u64().unwrap() as usize);
                    app.pick_curve_line_at(object, None, inputs::pick(v, i, action));
                } else if action.get("click").is_some() {
                    let p = inputs::click_point(v, i, &app.curve_line.as_ref().unwrap().input);
                    app.accept_curve_line_point(p);
                } else {
                    let value = inputs::token(action);
                    let value = if action.is_array() {
                        format!("w{value}")
                    } else {
                        value
                    };
                    enter(
                        &mut app,
                        if value == "Enter" {
                            ""
                        } else if value == "Cancel" {
                            "!"
                        } else {
                            &value
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
                "LINE_TANGENT_BOUNDARY_UI {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"archives":archives})
            );
            assert!(error <= 1e-6, "{id}: {error:e}");
            count += 1;
        }
    }
    assert_eq!(count, 24);
}
#[test]
fn endpoint_fallback_mouse_preview_commits_and_sets_the_relative_origin() {
    use super::line_perpendicular_viewport::{click, frame};
    let rows = native::read("line_tangent_arcs_ends");
    let row = rows["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "end_arc_top")
        .unwrap();
    let mut app = test_app();
    app.document = inputs::setup(&row["value"], &inputs::sources()["end_arc_top"]);
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.grid_snap = false;
    app.osnap = false;
    app.smart_track = false;
    let id = inputs::guide(&app.document, 0);
    let parent = app.document.object(id).unwrap().clone();
    let context = egui::Context::default();
    frame(&mut app, &context, vec![]);
    app.viewports[0].zoom_out(0.5).unwrap();
    let (_, rect) = frame(&mut app, &context, vec![]);
    enter(&mut app, "Line Tangent");
    let p = app.viewports[0]
        .project(point(3.5355339059327378, 3.5355339059327378, 0.), rect)
        .unwrap();
    let output = click(&mut app, &context, p);
    app.handle_viewport_action(output);
    let p = app.viewports[0].project(point(-8., 2., 0.), rect).unwrap();
    let (hover, _) = frame(&mut app, &context, vec![egui::Event::PointerMoved(p)]);
    let preview = app
        .curve_line
        .as_ref()
        .unwrap()
        .input
        .preview(hover.drafting_hover.unwrap())
        .unwrap();
    assert!(preview.line.start().distance_to(point(0., 5., 0.)).unwrap() < 1e-8);
    assert!((preview.line.end().y() - 5.).abs() < 1e-8);
    let output = click(&mut app, &context, p);
    app.handle_viewport_action(output);
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert_eq!(
        app.document.objects().last().unwrap().geometry(),
        &Geometry::Line(preview.line)
    );
    assert_eq!(app.document.object(id).unwrap(), &parent);
    assert!(
        parent
            .geometry_snapshot()
            .shares_storage_with(app.document.object(id).unwrap().geometry_snapshot())
    );
    assert_eq!(app.last_point, Some(preview.accepted_point));
    enter(&mut app, "Point");
    enter(&mut app, "r1,2,3");
    let Geometry::Point(p) = app.document.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert!(
        p.distance_to(
            preview
                .accepted_point
                .translated(viboceros_geometry::Vector3::try_from([1., 2., 3.]).unwrap())
                .unwrap()
        )
        .unwrap()
            < 1e-10
    );
}
