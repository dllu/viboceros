use super::*;
use viboceros_document::SelectionMode;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn source(app: &mut VibocerosApp) -> viboceros_document::ObjectId {
    use viboceros_geometry::{Brep, TriangleMesh};
    let t = app.document.tolerance();
    let mesh = TriangleMesh::try_new(
        vec![
            point(0., 0., 0.),
            point(4., 0., 0.),
            point(4., 3., 0.),
            point(0., 3., 0.),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
        t,
    )
    .unwrap();
    app.document
        .add_geometry(Geometry::Brep(Brep::try_from_mesh(&mesh, true, t).unwrap()))
        .unwrap()
}
#[test]
fn coplanar_getter_filters_picks_rejects_options_and_cancels() {
    let mut app = test_app();
    let id = source(&mut app);
    let peer = app
        .document
        .add_geometry(Geometry::Point(point(9., 0., 0.)))
        .unwrap();
    let before = app.document.object(id).unwrap().clone();
    enter(&mut app, "MergeAllCoplanarFaces");
    assert!(app.object_prompt.is_some());
    assert_eq!(
        app.viewport_object_filter(),
        Some(viboceros_command::ObjectSelectionFilter::SurfaceComponents)
    );
    for object_id in [peer, id] {
        app.apply_selection_click(SelectionClick {
            object_id: Some(object_id),
            mode: SelectionMode::Replace,
        });
    }
    assert!(!app.document.is_selected(peer));
    assert!(app.document.is_selected(id));
    enter(&mut app, "Tolerance=1");
    assert!(app.object_prompt.is_some());
    assert_eq!(app.document.object(id), Some(&before));
    enter(&mut app, "Cancel");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.object(id), Some(&before));
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn coplanar_getter_retains_picks_when_native_kernel_is_unavailable() {
    let mut app = test_app();
    let id = source(&mut app);
    enter(&mut app, "MergeAllCoplanarFaces");
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    let before = format!("{:?}", app.document);
    enter(&mut app, "");
    assert!(app.object_prompt.is_some());
    assert_eq!(format!("{:?}", app.document), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn coplanar_getter_commits_once_for_postselection_and_preselection() {
    for pre in [false, true] {
        let mut app = test_app();
        let id = source(&mut app);
        let before = app.document.object(id).unwrap().clone();
        if pre {
            app.document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
        }
        enter(&mut app, "MergeAllCoplanarFaces");
        if !pre {
            app.apply_selection_click(SelectionClick {
                object_id: Some(id),
                mode: SelectionMode::Replace,
            });
            assert_eq!(app.document.object(id), Some(&before));
            enter(&mut app, "");
        }
        assert!(app.object_prompt.is_none());
        let after = app.document.object(id).unwrap().clone();
        assert!(matches!(after.geometry(),Geometry::Brep(b) if b.faces().len()==1));
        assert_eq!(app.document.undo_label(), Some("MergeAllCoplanarFaces"));
        enter(&mut app, "Undo");
        assert_eq!(app.document.object(id), Some(&before));
        enter(&mut app, "Redo");
        assert_eq!(app.document.object(id), Some(&after));
    }
}
