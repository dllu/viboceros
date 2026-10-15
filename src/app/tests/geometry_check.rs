use super::*;
use viboceros_geometry::{MeshFace, TriangleMesh};
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn bad_source(app: &mut VibocerosApp) -> ObjectId {
    let mesh = TriangleMesh::try_from_face_records(
        vec![point(0., 0., 0.), point(1., 0., 0.), point(0., 0., 0.)],
        vec![MeshFace::Triangle([0, 1, 2])],
    )
    .unwrap();
    app.document.add_geometry(Geometry::Mesh(mesh)).unwrap()
}
#[test]
fn native_geometry_check_getter_accepts_preselection_or_picks_and_leaves_redo() {
    for preselected in [false, true] {
        let mut app = test_app();
        let id = bad_source(&mut app);
        app.document
            .add_geometry(Geometry::Point(point(9., 9., 9.)))
            .unwrap();
        app.document.undo().unwrap();
        let original = app.document.object(id).unwrap().clone();
        if preselected {
            app.document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
        }
        enter(&mut app, "Check");
        if !preselected {
            assert!(app.object_prompt.is_some());
            assert_eq!(
                app.viewport_object_filter(),
                Some(viboceros_command::ObjectSelectionFilter::Any)
            );
            app.apply_selection_click(SelectionClick {
                object_id: Some(id),
                mode: SelectionMode::Replace,
            });
            let before = format!("{:?}", app.document);
            enter(&mut app, "");
            assert_eq!(format!("{:?}", app.document), before);
        }
        assert!(app.object_prompt.is_none());
        assert!(app.command_line_idle());
        assert!(
            app.command_log
                .iter()
                .any(|s| s.contains("Check: 0 valid, 1 invalid, 0 not checked"))
        );
        assert_eq!(app.document.object(id), Some(&original));
        assert!(app.document.can_redo());
    }
}
#[test]
fn native_geometry_check_cancel_and_bad_object_selection_share_existing_ui_flows() {
    let mut app = test_app();
    let id = bad_source(&mut app);
    enter(&mut app, "Check");
    app.cancel_current_prompt_or_selection();
    assert!(app.object_prompt.is_none());
    assert!(app.command_line_idle());
    enter(&mut app, "SelBadObjects");
    assert!(app.document.is_selected(id));
    assert!(
        app.command_log
            .iter()
            .any(|s| s.contains("Selected 1 bad object(s)"))
    );
}
