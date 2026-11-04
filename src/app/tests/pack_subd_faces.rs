use super::*;
use viboceros_document::SelectionMode;
use viboceros_geometry::SubD;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn fixture() -> (VibocerosApp, viboceros_document::ObjectId) {
    let mut app = test_app();
    let subd = SubD::try_from_polygons(
        vec![
            point(0., 0., 0.),
            point(2., 0., 1.),
            point(4., 0., 0.),
            point(0., 3., 0.),
            point(2., 3., 2.),
            point(4., 3., 0.),
        ],
        vec![vec![0, 1, 4, 3], vec![1, 2, 5, 4]],
    )
    .unwrap()
    .try_with_vertex_tags(&[(0, 3), (2, 3), (3, 3), (5, 3)])
    .unwrap();
    let id = app.document.add_geometry(Geometry::SubD(subd)).unwrap();
    app.document.clear_history().unwrap();
    (app, id)
}
#[test]
fn pack_subd_faces_filters_picks_cancels_and_replays_one_edit() {
    let (mut app, id) = fixture();
    let other = app
        .document
        .add_geometry(Geometry::Point(point(9., 8., 7.)))
        .unwrap();
    app.document.clear_history().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "PackSubDFaces");
    assert!(app.object_prompt.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(other),
        mode: SelectionMode::Replace,
    });
    assert!(!app.document.is_selected(other));
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    app.cancel_interactive_command(true);
    assert_eq!(app.document.selected_object_count(), 0);
    assert!(!app.document.can_undo());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "PackSubDFaces");
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.undo_label(), Some("PackSubDFaces"));
    let after = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), after);
}
#[test]
fn pack_subd_faces_preselection_executes_immediately_and_keeps_mixed_selection() {
    let (mut app, id) = fixture();
    let other = app
        .document
        .add_geometry(Geometry::Point(point(9., 8., 7.)))
        .unwrap();
    app.document.clear_history().unwrap();
    app.document
        .select_objects_direct([id, other], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "_PackSubDFaces");
    assert!(app.object_prompt.is_none());
    assert!(app.document.is_selected(id));
    assert!(app.document.is_selected(other));
    let Geometry::SubD(s) = app.document.object(id).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(
        s.face_packs()
            .unwrap()
            .iter()
            .map(|p| p.unwrap().id)
            .collect::<Vec<_>>(),
        vec![1, 1]
    );
    assert_eq!(app.document.undo_label(), Some("PackSubDFaces"));
    enter(&mut app, "Undo");
    assert!(app.document.is_selected(id));
    assert!(app.document.is_selected(other));
    let history = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "PackSubDFaces invalid");
    assert_eq!(app.document.undo_label(), history.as_deref());
    assert!(app.document.can_redo());
}
