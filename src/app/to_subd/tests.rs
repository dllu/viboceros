use super::*;
use crate::app::tests::test_app;
use crate::viewport::{ComponentClick, ComponentPick};
#[allow(dead_code)]
#[path = "../../../crates/viboceros-command/tests/support/tosubd.rs"]
mod native;
use crate::app::{subd_native_tests as subd_native, subdivision_native_tests as geometry_native};
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command_input();
}
fn face(app: &mut VibocerosApp, object: ObjectId, index: usize) {
    app.accept_component_click(ComponentClick {
        picks: vec![ComponentPick {
            object,
            kind: ComponentSelectionKind::BrepFace,
            index,
        }],
        preselection: true,
        modifiers: egui::Modifiers {
            ctrl: true,
            shift: true,
            ..Default::default()
        },
    });
}
#[test]
fn tosubd_getters_replay_native_conversion_options_components_cancellation_and_history() {
    let mut count = 0;
    for row in native::rows() {
        eprintln!("checking getter {}", row["id"]);
        let v = &row["value"];
        // Native mesh subobject input is rejected before ToSubD begins.
        let rejected_component = v["input"]["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["kind"] == "mesh" && s.get("selected_faces").is_some());
        if rejected_component {
            continue;
        }
        let mut scene = native::setup(v);
        let mut app = test_app();
        std::mem::swap(&mut app.document, &mut scene.doc);
        for (i, s) in v["input"]["sources"].as_array().unwrap().iter().enumerate() {
            for f in s["selected_faces"].as_array().into_iter().flatten() {
                face(&mut app, scene.ids[i], f.as_u64().unwrap() as usize);
            }
        }
        enter(&mut app, "ToSubD");
        assert!(app.to_subd_pending(), "{} {:?}", row["id"], app.command_log);
        if app.picking_to_subd() && v["input"]["cancel_selection"] != true {
            let eligible = scene
                .ids
                .iter()
                .copied()
                .filter(|&id| {
                    viboceros_command::to_subd::accepts(app.document.object(id).unwrap().geometry())
                })
                .collect::<Vec<_>>();
            app.document
                .select_objects_direct(eligible, SelectionMode::Replace)
                .unwrap();
            enter(&mut app, "");
        }
        if v["input"]["cancel"] == true
            || v["input"]["cancel_selection"] == true
            || v["success"] != true
        {
            app.cancel_interactive_command(false);
        } else {
            enter(&mut app, &native::option_line(v));
            enter(&mut app, "");
            assert!(
                !app.to_subd_pending(),
                "{} {:?}",
                row["id"],
                app.command_log
            );
            assert_eq!(app.command_log.back().unwrap(), &native::message(v));
            std::mem::swap(&mut app.document, &mut scene.doc);
            native::check(&scene, &v["after"]);
            std::mem::swap(&mut app.document, &mut scene.doc);
        }
        app.document.clear_selection();
        std::mem::swap(&mut app.document, &mut scene.doc);
        let check = native::check(&scene, &v["after_script"]);
        let mutated = v["after_script"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o.get("subd").is_some());
        if mutated {
            assert_eq!(scene.doc.undo_label(), Some("ToSubD"));
            scene.doc.undo().unwrap();
            native::check(&scene, &v["undo"]);
            scene.doc.redo().unwrap();
            native::check(&scene, &v["redo"]);
        } else {
            assert_eq!(scene.doc.undo_label(), None);
        }
        println!(
            "TOSUBD_GETTER {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true,"mutated":mutated})
        );
        count += 1;
    }
    assert_eq!(count, 200);
}
#[test]
fn tosubd_face_picks_after_start_and_stale_input_preserve_document_and_redo() {
    for change in ["geometry", "hidden", "tolerance", "cancel"] {
        let row = native::rows()
            .into_iter()
            .find(|r| r["id"] == "tosubd_box_faces_01_0")
            .unwrap();
        let mut scene = native::setup(&row["value"]);
        let mut app = test_app();
        std::mem::swap(&mut scene.doc, &mut app.document);
        enter(&mut app, "Point 8,8,8");
        enter(&mut app, "Undo");

        enter(&mut app, "ToSubD");
        assert!(app.component_preselection_available());
        face(&mut app, scene.ids[0], 0);
        face(&mut app, scene.ids[0], 1);
        enter(&mut app, "");
        assert_eq!(
            app.object_prompt.as_ref().unwrap().phase,
            ObjectPromptPhase::Options
        );
        enter(&mut app, "DeleteInput=Yes UseSurface=ControlPoints");
        match change {
            "geometry" => {
                app.document
                    .replace_object_geometries([(
                        scene.ids[0],
                        Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()),
                    )])
                    .unwrap();
            }
            "hidden" => {
                app.document
                    .set_objects_visibility([scene.ids[0]], false)
                    .unwrap();
            }
            "tolerance" => {
                app.document.set_tolerance(
                    viboceros_geometry::Tolerance::try_new(1e-5, 1e-12, 1e-6).unwrap(),
                );
            }
            _ => {}
        }
        let redo = app.document.redo_label().map(str::to_owned);
        let before = format!("{:?}", app.document);
        if change != "cancel" {
            enter(&mut app, "");
            assert_eq!(format!("{:?}", app.document), before);
            assert!(app.to_subd_pending());
            assert!(
                app.command_log
                    .back()
                    .unwrap()
                    .contains("source geometry, tolerance or editability changed")
            );
        }
        app.cancel_interactive_command(false);
        assert!(
            app.component_selection
                .checked_picks(&app.document)
                .unwrap()
                .is_empty()
        );
        assert_eq!(app.document.redo_label(), redo.as_deref());
        let prompt = app
            .commands
            .object_selection_prompt("ToSubD")
            .unwrap()
            .unwrap();
        let remembered = ToSubDOptions::from_prompt(&prompt).unwrap();
        assert!(remembered.delete_input);
        assert_eq!(
            remembered.surface_method,
            viboceros_geometry::SubDFromSurfaceMethod::ControlPoints
        );
    }
}
