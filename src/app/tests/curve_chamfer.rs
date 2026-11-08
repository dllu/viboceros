use super::curve_fillet::{enter, inputs};
use super::line_reference::inputs as reference_inputs;
use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/chamfer_getter.rs"]
mod getters;

#[test]
fn captured_getters_preserve_distances_across_cancel_and_match_native_documents() {
    assert_eq!(replay(&getters::FAMILIES, "CHAMFER"), 30);
}
pub(super) fn replay(families: &[&str], marker: &str) -> usize {
    let mut count = 0;
    for family in families {
        let rows = native::read(family);
        let mut app = test_app();
        app.commands
            .remember_chamfer_preferences(getters::initial(&rows["results"][0]["value"]))
            .unwrap();
        for row in rows["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let v = &row["value"];
            println!("{marker}_UI_CASE {family}/{id}");
            assert_eq!(
                app.commands.chamfer_preferences(),
                getters::initial(v),
                "{id}"
            );
            app.document = inputs::setup(v);
            if getters::unsupported(id) && !app.document.can_redo() {
                app.commands
                    .execute(&mut app.document, "Point 90,91,92")
                    .unwrap();
                app.document.undo().unwrap();
            }
            let state = inputs::State::capture(&app.document);
            let before = app
                .document
                .objects()
                .map(|o| (o.id(), o.geometry().clone()))
                .collect::<Vec<_>>();
            enter(&mut app, "Chamfer");
            for (index, a) in v["prescribed_case"]["actions"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                if let Some(i) = a["curve"].as_u64() {
                    let point = if let Some(p) =
                        v["prescribed_case"]["source_seed_points"].get(i as usize)
                    {
                        native::point(p)
                    } else {
                        let click = v["input_trace"][0]["clicks"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|c| c["action"].as_u64() == Some(index as u64))
                            .unwrap();
                        inputs::input_point(click)
                    };
                    app.pick_curve_chamfer_at(
                        reference_inputs::guide(&app.document, i as usize),
                        point,
                    );
                } else {
                    let token = a["number"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| reference_inputs::token(a));
                    let token = if a.is_array() {
                        format!("w{token}")
                    } else {
                        token
                    };
                    enter(&mut app, if token == "Enter" { "" } else { &token });
                }
                if app.curve_chamfer_session.is_some() {
                    assert!(!app.document.can_undo(), "{id}");
                    assert_eq!(
                        before,
                        app.document
                            .objects()
                            .map(|o| (o.id(), o.geometry().clone()))
                            .collect::<Vec<_>>(),
                        "{id}"
                    );
                }
            }
            assert!(
                app.curve_chamfer_session.is_none(),
                "{id}: {:?}",
                app.command_log
            );
            if inputs::success(v) {
                assert_eq!(
                    app.commands.chamfer_preferences(),
                    getters::final_options(v)
                );
            }
            if getters::unsupported(id) {
                assert_eq!(
                    before,
                    app.document
                        .objects()
                        .map(|o| (o.id(), o.geometry().clone()))
                        .collect::<Vec<_>>()
                );
                assert!(!app.document.can_undo());
                assert!(app.document.can_redo());
                assert!(
                    app.command_log
                        .back()
                        .unwrap()
                        .starts_with("Unable to chamfer")
                );
                println!(
                    "{marker}_UI_REJECTION {{\"id\":\"{family}/{id}\",\"redo_preserved\":true}}"
                );
            } else {
                let mut r = state.verify(&mut app.document, v);
                r["id"] = serde_json::json!(format!("{family}/{id}"));
                println!("{marker}_UI {r}");
                count += 1;
            }
        }
    }
    count
}

#[test]
fn mouse_picks_and_distance_points_commit_once_and_stale_sources_cancel() {
    let mut app = test_app();
    for s in ["Line 0,0,0 4,0,0", "Line 4,0,0 4,4,0"] {
        app.commands.execute(&mut app.document, s).unwrap();
    }
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    enter(&mut app, "Chamfer");
    enter(&mut app, "Distances");
    for point in [[1., 2., 0.], [1., 2.5, 0.], [1., 3., 0.]] {
        app.handle_viewport_action(ViewportOutput {
            picked_point: Some(Point3::try_from(point).unwrap()),
            ..Default::default()
        });
    }
    assert_eq!(app.commands.chamfer_preferences().distances, [0.5, 1.]);
    assert!(app.picking_curve_chamfer());
    assert!(!app.viewport_drafting_input(false, false).active);
    let ids: Vec<_> = app.document.objects().map(|o| o.id()).collect();
    let context = egui::Context::default();
    let (_, rect) = super::curve_fillet::mouse_frame(&mut app, &context, vec![]);
    for (index, point) in [[2.7, 0., 0.], [4., 1.7, 0.]].into_iter().enumerate() {
        let pos = app.viewports[0]
            .project(Point3::try_from(point).unwrap(), rect)
            .unwrap();
        super::curve_fillet::mouse_frame(&mut app, &context, vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            let (output, _) = super::curve_fillet::mouse_frame(
                &mut app,
                &context,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            if !pressed {
                assert_eq!(
                    output.selection_click.as_ref().unwrap().object_id,
                    Some(ids[index])
                );
            }
            app.handle_viewport_action(output);
        }
    }
    assert!(app.curve_chamfer_session.is_none());
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.undo_label(), Some("Chamfer"));
    app.document.undo().unwrap();
    assert!(!app.document.can_undo());
    enter(&mut app, "Chamfer");
    app.pick_curve_chamfer_at(ids[0], Point3::try_new(3., 0., 0.).unwrap());
    app.commands
        .execute(&mut app.document, "Point 8,9,10")
        .unwrap();
    app.commands
        .execute(&mut app.document, &format!("SelID {}", ids[0]))
        .unwrap();
    app.commands
        .execute(&mut app.document, "Move 0,0,0 1,0,0")
        .unwrap();
    app.validate_curve_chamfer();
    assert!(app.curve_chamfer_session.is_none());
    println!("CHAMFER_MOUSE_SOURCE_PICKS 2");
}
