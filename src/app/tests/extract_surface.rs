use super::*;
use crate::viewport::{ComponentClick, ComponentPick, ComponentWindow};
use viboceros_command::ComponentSelectionKind;
use viboceros_document::{ObjectId, SelectionMode};
use viboceros_geometry::Brep;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn source(app: &mut VibocerosApp) -> ObjectId {
    app.document
        .add_geometry(Geometry::Brep(
            Brep::try_box(
                viboceros_command::CommandContext::default().construction_plane,
                [[0., 2.], [0., 3.], [0., 4.]],
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap()
}
fn pick(id: ObjectId, index: usize) -> ComponentPick {
    ComponentPick {
        object: id,
        index,
        kind: ComponentSelectionKind::BrepFace,
    }
}
fn click(app: &mut VibocerosApp, id: ObjectId, index: usize, ctrl: bool) {
    app.accept_component_click(ComponentClick {
        picks: vec![pick(id, index)],
        preselection: false,
        modifiers: egui::Modifiers {
            ctrl,
            ..Default::default()
        },
    });
}

#[test]
fn extract_faces_accumulate_remove_and_rectangle_pick_across_objects_before_one_edit() {
    let mut app = test_app();
    let a = source(&mut app);
    let b = source(&mut app);
    let group = app.document.add_group(None, [a, b]).unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "ExtractSrf");
    assert!(app.picking_extract_faces());
    enter(&mut app, "");
    assert!(app.picking_extract_faces());
    assert!(!app.document.can_undo());
    click(&mut app, a, 0, false);
    click(&mut app, b, 2, false);
    click(&mut app, a, 0, true);
    assert_eq!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap(),
        [pick(b, 2)]
    );
    app.accept_component_window(ComponentWindow {
        picks: vec![pick(a, 1), pick(b, 4)],
        preselection: false,
        modifiers: Default::default(),
        crossing: false,
        inverted: false,
    });
    assert_eq!(app.document.objects().len(), 2);
    assert!(!app.document.can_undo());
    enter(&mut app, "");
    assert!(!app.picking_extract_faces());
    assert_eq!(app.document.objects().len(), 5);
    assert_eq!(app.document.selected_object_count(), 3);
    assert_eq!(
        app.document
            .group(group)
            .unwrap()
            .members()
            .collect::<std::collections::BTreeSet<_>>(),
        [a, b].into_iter().collect()
    );
    for object in app.document.selected_objects() {
        assert!(object.group_ids().is_empty());
    }
    assert_eq!(app.document.undo_label(), Some("ExtractSrf"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 2);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 5);
}

#[test]
fn extract_face_preselection_can_change_copy_and_layer_before_enter() {
    let mut app = test_app();
    let id = source(&mut app);
    let before = app.document.object(id).unwrap().geometry().clone();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    app.accept_component_click(ComponentClick {
        picks: vec![pick(id, 3)],
        preselection: true,
        modifiers: egui::Modifiers {
            ctrl: true,
            shift: true,
            ..Default::default()
        },
    });
    let layer = app
        .document
        .add_layer("output", viboceros_document::ColorRgb::new(10, 20, 30))
        .unwrap();
    app.document.set_current_layer(layer).unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "ExtractSrf");
    assert!(app.picking_extract_faces());
    assert!(!app.document.can_undo());
    enter(&mut app, "Copy _Yes OutputLayer=_Current");
    enter(&mut app, "Copy=Maybe");
    assert!(app.picking_extract_faces());
    assert_eq!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap(),
        [pick(id, 3)]
    );
    enter(&mut app, "");
    assert_eq!(app.document.object(id).unwrap().geometry(), &before);
    assert_eq!(app.document.selected_object_count(), 1);
    assert_eq!(
        app.document
            .selected_objects()
            .next()
            .unwrap()
            .attributes()
            .layer_id(),
        layer
    );
}

#[test]
fn extraction_cancel_none_and_stale_geometry_never_extract_partial_faces() {
    let mut app = test_app();
    let id = source(&mut app);
    app.document.clear_history().unwrap();
    enter(&mut app, "ExtractSrf");
    click(&mut app, id, 0, false);
    enter(&mut app, "None");
    assert!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .is_empty()
    );
    click(&mut app, id, 1, false);
    app.cancel_current_prompt_or_selection();
    assert!(!app.picking_extract_faces());
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
    enter(&mut app, "ExtractSrf");
    click(&mut app, id, 0, false);
    let Geometry::Brep(brep) = app.document.object(id).unwrap().geometry() else {
        panic!()
    };
    let changed = brep
        .sub_brep(&[0, 1, 2, 3, 4], app.document.tolerance())
        .unwrap();
    app.document
        .replace_object_geometries([(id, Geometry::Brep(changed))])
        .unwrap();
    enter(&mut app, "");
    assert!(app.picking_extract_faces());
    assert_eq!(app.document.objects().len(), 1);
    assert!(app.command_log.back().unwrap().contains("source changed"));
    enter(&mut app, "Undo");
    assert!(!app.picking_extract_faces());
    assert_eq!(app.document.objects().len(), 1);
}
