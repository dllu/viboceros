use super::*;
use crate::app::subd_native_tests as subd_native;
use crate::app::tests::test_app;
use crate::viewport::{ComponentClick, ComponentPick};
#[allow(dead_code)]
#[path = "../../../crates/viboceros-command/tests/support/subdivide.rs"]
mod native;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command_input();
}
fn face(app: &mut VibocerosApp, object: ObjectId, index: usize) {
    let kind = match app.document.object(object).unwrap().geometry() {
        Geometry::SubD(_) => ComponentSelectionKind::SubDFace,
        Geometry::Mesh(_) => ComponentSelectionKind::MeshFace,
        _ => panic!(),
    };
    app.accept_component_click(ComponentClick {
        picks: vec![ComponentPick {
            object,
            kind,
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
fn subdivision_getters_replay_native_whole_objects_components_and_history() {
    for row in native::rows()
        .into_iter()
        .filter(|r| native::getter_eligible(&r["value"]))
    {
        let value = &row["value"];
        let mut scene = native::setup(value);
        let mut app = test_app();
        std::mem::swap(&mut scene.doc, &mut app.document);
        let mut components = Vec::new();
        for (i, source) in value["input"]["sources"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            if let Some(faces) = source["selected_faces"].as_array() {
                for face in faces {
                    components.push((scene.ids[i], face.as_u64().unwrap() as usize));
                }
            }
        }
        for (id, index) in &mut components {
            if let Geometry::Mesh(mesh) = app.document.object(*id).unwrap().geometry()
                && let Some(ngon) = mesh
                    .ngons()
                    .iter()
                    .find(|n| n.faces().contains(&(*index as u32)))
            {
                *index = *ngon.faces().iter().min().unwrap() as usize;
            }
        }
        components.sort();
        components.dedup();
        for &(id, index) in &components {
            face(&mut app, id, index);
        }
        for step in value["steps"].as_array().unwrap() {
            enter(&mut app, "SubDivide");
            if components.is_empty() && value["input"]["preselected"] != true {
                assert!(
                    app.picking_subdivide(),
                    "{} {:?}",
                    row["id"],
                    app.command_log
                );
                app.document
                    .select_objects_direct(scene.ids.iter().copied(), SelectionMode::Replace)
                    .unwrap();
                if value["input"]["cancel"] == true {
                    app.cancel_interactive_command(false);
                } else {
                    enter(&mut app, "");
                }
            }
            assert!(
                app.object_prompt.is_none(),
                "{} {:?}",
                row["id"],
                app.command_log
            );
            std::mem::swap(&mut scene.doc, &mut app.document);
            native::check(&scene, &step["after"]);
            let children = app
                .component_selection
                .checked_picks(&scene.doc)
                .unwrap()
                .into_iter()
                .filter(|p| p.kind == ComponentSelectionKind::SubDFace)
                .map(|p| (p.object, p.index))
                .collect::<Vec<_>>();
            native::check_children(&scene, &children, &step["after"]);
            std::mem::swap(&mut scene.doc, &mut app.document);
        }
        app.component_selection.clear();
        app.document.clear_selection();
        std::mem::swap(&mut scene.doc, &mut app.document);
        let check = native::check(&scene, &value["after_script"]);
        if value["input"]["cancel"] != true {
            assert_eq!(scene.doc.undo_label(), Some("SubDivide"));
            scene.doc.undo().unwrap();
            native::check(&scene, &value["undo"]);
            scene.doc.redo().unwrap();
            native::check(&scene, &value["redo"]);
        } else {
            assert_eq!(scene.doc.undo_label(), None);
        }
        println!(
            "SUBDIVIDE_GETTER {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}
#[test]
fn subdivision_prompt_selects_faces_after_start_and_cancellation_preserves_redo() {
    let row = native::rows()
        .into_iter()
        .find(|r| r["id"] == "subdivide_subd_strip_local")
        .unwrap();
    let value = &row["value"];
    let mut scene = native::setup(value);
    let mut app = test_app();
    std::mem::swap(&mut app.document, &mut scene.doc);
    enter(&mut app, "Point 9,9,9");
    enter(&mut app, "Undo");
    let redo = app.document.redo_label().map(str::to_owned);
    enter(&mut app, "SubDivide");
    assert!(app.component_preselection_available());
    face(&mut app, scene.ids[0], 0);
    assert_eq!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .len(),
        1
    );
    enter(&mut app, "SelNone");
    assert!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .is_empty()
    );
    face(&mut app, scene.ids[0], 0);
    app.cancel_interactive_command(false);
    assert!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .is_empty()
    );
    assert_eq!(app.document.redo_label(), redo.as_deref());
    enter(&mut app, "SubDivide");
    face(&mut app, scene.ids[0], 0);
    enter(&mut app, "");
    let picks = app
        .component_selection
        .checked_picks(&app.document)
        .unwrap();
    assert_eq!(picks.len(), 4);
    assert!(
        picks
            .iter()
            .all(|p| p.kind == ComponentSelectionKind::SubDFace)
    );
    std::mem::swap(&mut scene.doc, &mut app.document);
    native::check(&scene, &value["steps"][0]["after"]);
}
#[test]
fn subdivision_prompt_rejects_geometry_and_visibility_changes_instead_of_using_stale_faces() {
    for hidden in [false, true] {
        let row = native::rows()
            .into_iter()
            .find(|r| r["id"] == "subdivide_subd_strip_local")
            .unwrap();
        let mut scene = native::setup(&row["value"]);
        let mut app = test_app();
        std::mem::swap(&mut scene.doc, &mut app.document);
        enter(&mut app, "SubDivide");
        face(&mut app, scene.ids[0], 0);
        if hidden {
            app.document
                .set_objects_visibility([scene.ids[0]], false)
                .unwrap();
        } else {
            let Geometry::SubD(s) = app.document.object(scene.ids[0]).unwrap().geometry() else {
                panic!()
            };
            let changed = s.subdivided(1).unwrap();
            app.document
                .replace_object_geometries([(scene.ids[0], Geometry::SubD(changed))])
                .unwrap();
        }
        let before = format!("{:?}", app.document);
        enter(&mut app, "");
        assert!(app.picking_subdivide());
        assert_eq!(format!("{:?}", app.document), before);
        assert!(
            app.command_log
                .back()
                .unwrap()
                .contains("source or tolerance changed")
        );
        app.cancel_interactive_command(false);
    }
}
