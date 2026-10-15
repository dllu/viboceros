use super::*;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn surface(app: &mut VibocerosApp) -> ObjectId {
    app.document
        .add_geometry(Geometry::NurbsSurface(
            viboceros_geometry::NurbsSurface::try_new(
                1,
                1,
                2,
                2,
                vec![
                    point(0., 0., 0.),
                    point(2., 0., 0.),
                    point(0., 3., 0.),
                    point(2., 3., 1.),
                ],
                vec![0., 0., 1., 1.],
                vec![0., 0., 1., 1.],
            )
            .unwrap(),
        ))
        .unwrap()
}
#[test]
fn surface_extrusion_source_picker_options_and_cancel_preserve_model_geometry() {
    let mut app = test_app();
    let id = surface(&mut app);
    let before = app.document.object(id).unwrap().clone();
    enter(&mut app, "ExtrudeSrf BothSides=Yes Solid=No");
    assert!(app.object_prompt.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    let Some(InteractiveCommand::ExtrudeSurface { options, .. }) = app.active_command else {
        panic!()
    };
    assert!(options.both_sides);
    assert!(!options.solid);
    enter(&mut app, "Solid=Yes DeleteInput=Yes");
    let Some(InteractiveCommand::ExtrudeSurface { options, .. }) = app.active_command else {
        panic!()
    };
    assert!(options.solid && options.delete_input);
    enter(&mut app, "Cancel");
    assert!(app.active_command.is_none());
    assert!(app.surface_extrusion_sources.is_none());
    assert_eq!(app.document.object(id), Some(&before));
}
#[test]
fn surface_extrusion_failure_and_stale_sources_keep_the_getter() {
    let mut app = test_app();
    let id = surface(&mut app);
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ExtrudeSrf");
    let active = app.active_command;
    let before = format!("{:?}", app.document);
    enter(&mut app, "0");
    assert_eq!(app.active_command, active);
    assert_eq!(format!("{:?}", app.document), before);
    app.document
        .replace_object_geometries([(id, Geometry::Point(point(5., 5., 5.)))])
        .unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "2");
    assert!(app.active_command.is_some());
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.command_log.back().unwrap().contains("source changed"));
}
#[cfg(feature = "native-smlib")]
#[test]
fn surface_extrusion_distance_and_direction_points_commit_from_frozen_faces() {
    for numeric in [false, true] {
        let mut app = test_app();
        let id = surface(&mut app);
        app.document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        enter(&mut app, "ExtrudeSrf BothSides=Yes DeleteInput=Yes");
        if numeric {
            enter(&mut app, "2");
        } else {
            assert!(app.accept_drafting_point(point(1., 2., 3.)));
            assert!(app.accept_drafting_point(point(1., 2., 5.)));
        }
        assert!(app.active_command.is_none());
        assert!(app.document.object(id).is_none());
        assert_eq!(app.document.objects().count(), 1);
        let Geometry::Brep(b) = app.document.selected_objects().next().unwrap().geometry() else {
            panic!()
        };
        assert!(b.is_solid());
        assert!((b.bounds().min().z() + 2.).abs() < 1e-8);
        assert!((b.bounds().max().z() - 3.).abs() < 1e-8);
        enter(&mut app, "Undo");
        assert!(app.document.object(id).is_some());
        enter(&mut app, "Redo");
        assert!(app.document.object(id).is_none());
    }
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_extrusion_component_preselection_uses_original_face_indices() {
    let mut app = test_app();
    let brep = viboceros_geometry::Brep::try_box(
        viboceros_command::CommandContext::default().construction_plane,
        [[0., 2.], [0., 3.], [0., 4.]],
        app.document.tolerance(),
    )
    .unwrap();
    let id = app.document.add_geometry(Geometry::Brep(brep)).unwrap();
    app.accept_component_click(crate::viewport::ComponentClick {
        picks: vec![crate::viewport::ComponentPick {
            object: id,
            kind: viboceros_command::ComponentSelectionKind::BrepFace,
            index: 0,
        }],
        preselection: true,
        modifiers: egui::Modifiers::default(),
    });
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "ExtrudeSrf DeleteInput=Yes");
    assert!(app.object_prompt.is_none());
    assert!(app.active_command.is_some());
    enter(&mut app, "2");
    assert!(app.active_command.is_none());
    let Geometry::Brep(remainder) = app.document.object(id).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(remainder.faces().len(), 5);
    assert_eq!(app.document.selected_object_count(), 1);
    assert!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .is_empty()
    );
}
