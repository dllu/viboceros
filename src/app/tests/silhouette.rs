use super::*;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn source(app: &mut VibocerosApp) -> ObjectId {
    app.document
        .add_geometry(Geometry::Brep(
            viboceros_geometry::Brep::try_box(
                viboceros_command::CommandContext::default().construction_plane,
                [[0., 2.], [0., 3.], [0., 4.]],
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap()
}

#[test]
fn perspective_silhouette_declines_without_reinterpreting_the_view_or_changing_geometry() {
    let mut app = test_app();
    let id = source(&mut app);
    app.active_viewport = 1;
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "Silhouette");
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("parallel viewport")
    );
    assert_eq!(format!("{:?}", app.document), before);
    // An explicit direction describes a parallel projection even in this view.
    assert!(app.silhouette_input("Silhouette Direction=0,0,1").is_ok());
}

#[test]
fn silhouette_source_picker_retains_picks_when_perspective_cannot_be_accepted() {
    let mut app = test_app();
    let id = source(&mut app);
    app.active_viewport = 1;
    enter(&mut app, "Silhouette");
    assert!(app.object_prompt.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    let before = format!("{:?}", app.document);
    enter(&mut app, "");
    assert!(app.object_prompt.is_some());
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("parallel viewport")
    );
    assert_eq!(format!("{:?}", app.document), before);
}

#[cfg(feature = "native-smlib")]
#[test]
fn silhouette_picker_uses_camera_direction_independently_of_cplane_and_replays_history() {
    let mut app = test_app();
    let id = source(&mut app);
    app.active_viewport = 0;
    let front = app.viewports[2].construction_plane();
    app.viewports[0].set_construction_plane(front);
    let original = app.document.object(id).unwrap().clone();
    enter(&mut app, "Silhouette InternalEdges=No");
    assert!(app.object_prompt.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().count(), 5);
    assert_eq!(app.document.object(id).unwrap(), &original);
    for object in app.document.selected_objects() {
        let Geometry::NurbsCurve(curve) = object.geometry() else {
            panic!()
        };
        assert!(
            curve
                .control_points()
                .iter()
                .all(|c| (c.point().z() - 4.).abs() < 1e-8)
        );
    }
    assert_eq!(app.document.undo_label(), Some("Silhouette"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), 1);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().count(), 5);
}
