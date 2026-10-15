use super::*;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn source(app: &mut VibocerosApp) -> ObjectId {
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
fn surface_apex_source_picker_and_atomic_options_survive_cancel() {
    let mut app = test_app();
    let s = source(&mut app);
    let before = app.document.object(s).unwrap().clone();
    enter(&mut app, "ExtrudeSrfToPoint Solid=No");
    assert!(app.object_prompt.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(s),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    let Some(InteractiveCommand::ExtrudeSurfaceToPoint { options }) = app.active_command else {
        panic!()
    };
    assert!(!options.solid);
    enter(&mut app, "Solid=Yes DeleteInput=Yes");
    let active = app.active_command;
    enter(&mut app, "Solid=No DeleteInput=bad");
    assert_eq!(app.active_command, active);
    enter(&mut app, "Cancel");
    assert!(app.active_command.is_none());
    assert!(app.surface_extrusion_sources.is_none());
    assert_eq!(app.document.object(s), Some(&before));
}
#[test]
fn surface_apex_changed_face_source_keeps_point_getter() {
    let mut app = test_app();
    let s = source(&mut app);
    app.document
        .select_objects_direct([s], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ExtrudeSrfToPoint");
    app.document
        .replace_object_geometries([(s, Geometry::Point(point(1., 2., 3.)))])
        .unwrap();
    let before = format!("{:?}", app.document);
    assert!(!app.accept_drafting_point(point(1., 1.5, 6.)));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::ExtrudeSurfaceToPoint { .. })
    ));
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.command_log.back().unwrap().contains("source changed"));
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn surface_apex_portable_failure_retains_points_and_frozen_sources() {
    let mut app = test_app();
    let s = source(&mut app);
    app.document
        .select_objects_direct([s], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ExtrudeSrfToPoint");
    let before = format!("{:?}", app.document);
    assert!(!app.accept_drafting_point(point(1., 1.5, 6.)));
    assert!(app.active_command.is_some());
    assert!(app.surface_extrusion_sources.is_some());
    assert_eq!(format!("{:?}", app.document), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn surface_apex_component_source_and_typed_point_commit_one_history_entry() {
    let mut app = test_app();
    let s = source(&mut app);
    let before = app.document.object(s).unwrap().clone();
    app.accept_component_click(crate::viewport::ComponentClick {
        picks: vec![crate::viewport::ComponentPick {
            object: s,
            kind: viboceros_command::ComponentSelectionKind::BrepFace,
            index: 0,
        }],
        preselection: true,
        modifiers: egui::Modifiers::default(),
    });
    enter(&mut app, "ExtrudeSrfToPoint DeleteInput=Yes");
    assert!(app.object_prompt.is_none());
    assert!(app.active_command.is_some());
    enter(&mut app, "1,1.5,6");
    assert!(app.active_command.is_none());
    assert!(app.document.object(s).is_none());
    let Geometry::Brep(b) = app.document.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(b.is_solid());
    assert!((b.signed_volume(app.document.tolerance()).unwrap().abs() - 11.5).abs() < 1e-5);
    enter(&mut app, "Undo");
    assert_eq!(app.document.object(s), Some(&before));
    enter(&mut app, "Redo");
    assert!(app.document.object(s).is_none());
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_apex_coplanar_failure_then_valid_clicked_target_uses_the_same_sources() {
    let mut app = test_app();
    let carrier = viboceros_geometry::NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![
            point(0., 0., 0.),
            point(2., 0., 0.),
            point(0., 3., 0.),
            point(2., 3., 0.),
        ],
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let s = app
        .document
        .add_geometry(Geometry::NurbsSurface(carrier))
        .unwrap();
    app.document
        .select_objects_direct([s], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ExtrudeSrfToPoint");
    let before = format!("{:?}", app.document);
    assert!(!app.accept_drafting_point(point(1., 1., 0.)));
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.active_command.is_some());
    assert!(app.surface_extrusion_sources.is_some());
    assert!(app.accept_drafting_point(point(1., 1.5, 6.)));
    assert!(app.active_command.is_none());
    assert!(app.document.object(s).is_some());
    assert_eq!(app.document.selected_object_count(), 1);
}
