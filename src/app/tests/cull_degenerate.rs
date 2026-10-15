use super::*;
use viboceros_geometry::{MeshFace, TriangleMesh};
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn source(app: &mut VibocerosApp, all_bad: bool) -> ObjectId {
    let points = vec![
        point(0., 0., 0.),
        point(1., 0., 0.),
        point(0., 1., 0.),
        point(0., 0., 0.),
    ];
    let face = if all_bad {
        MeshFace::Triangle([0, 1, 3])
    } else {
        MeshFace::Quad([0, 1, 2, 3])
    };
    app.document
        .add_geometry(Geometry::Mesh(
            TriangleMesh::try_from_face_records(points, vec![face]).unwrap(),
        ))
        .unwrap()
}
#[test]
fn degenerate_face_getter_preselection_and_clicks_commit_once_and_undo_restore_sources() {
    for pre in [false, true] {
        let mut app = test_app();
        let id = source(&mut app, false);
        let original = app.document.object(id).unwrap().clone();
        if pre {
            app.document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
        }
        enter(&mut app, "CullDegenerateMeshFaces");
        if !pre {
            assert!(app.object_prompt.is_some());
            assert_eq!(
                app.viewport_object_filter(),
                Some(viboceros_command::ObjectSelectionFilter::Mesh)
            );
            let peer = app
                .document
                .add_geometry(Geometry::Point(point(9., 9., 9.)))
                .unwrap();
            app.apply_selection_click(SelectionClick {
                object_id: Some(peer),
                mode: SelectionMode::Replace,
            });
            assert!(!app.document.is_selected(peer));
            app.apply_selection_click(SelectionClick {
                object_id: Some(id),
                mode: SelectionMode::Replace,
            });
            enter(&mut app, "");
        }
        assert!(app.object_prompt.is_none());
        assert!(app.command_line_idle());
        assert!(
            matches!(app.document.object(id).unwrap().geometry(),Geometry::Mesh(m) if m.face_count()==1 && m.vertices().len()==3)
        );
        assert_eq!(app.document.undo_label(), Some("CullDegenerateMeshFaces"));
        let after = app.document.object(id).unwrap().clone();
        enter(&mut app, "Undo");
        assert_eq!(app.document.object(id), Some(&original));
        enter(&mut app, "Redo");
        assert_eq!(app.document.object(id), Some(&after));
    }
}
#[test]
fn bad_object_selection_can_feed_cleanup_and_empty_deletion_is_reversible() {
    let mut app = test_app();
    let id = source(&mut app, true);
    let original = app.document.object(id).unwrap().clone();
    enter(&mut app, "CullDegenerateMeshFaces");
    app.cancel_current_prompt_or_selection();
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.object(id), Some(&original));
    enter(&mut app, "SelBadObjects");
    assert!(app.document.is_selected(id));
    enter(&mut app, "CullDegenerateMeshFaces");
    assert!(app.document.object(id).is_none());
    enter(&mut app, "Undo");
    assert_eq!(app.document.object(id), Some(&original));
}
