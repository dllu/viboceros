use super::*;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn setup() -> (VibocerosApp, ObjectId, ObjectId) {
    let mut app = test_app();
    let s = app
        .document
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
        .unwrap();
    let path = app
        .document
        .add_geometry(Geometry::NurbsCurve(
            viboceros_geometry::NurbsCurve::try_new(
                2,
                vec![
                    point(10., 20., 30.),
                    point(10., 21., 31.),
                    point(10., 20., 32.),
                ],
                vec![0., 0., 0., 1., 1., 1.],
            )
            .unwrap(),
        ))
        .unwrap();
    (app, s, path)
}
#[test]
fn surface_path_source_picker_handoff_options_and_cancel() {
    let (mut app, s, _) = setup();
    let source = app.document.object(s).unwrap().clone();
    enter(&mut app, "ExtrudeSrfAlongCrv Solid=No Reverse=Yes");
    assert!(app.object_prompt.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(s),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert!(!app.command_line_idle());
    assert!(!app.component_preselection_available());
    let session = app.surface_path_prompt.as_ref().unwrap();
    assert!(!session.options.solid && session.options.reverse);
    enter(&mut app, "Solid=Yes DeleteInput=Yes PathLength=1mm");
    let options = app.surface_path_prompt.as_ref().unwrap().options;
    assert!(options.solid && options.delete_input);
    assert!(options.path_length.is_some());
    enter(&mut app, "Solid=No PathLength=-1");
    assert_eq!(app.surface_path_prompt.as_ref().unwrap().options, options);
    enter(&mut app, "Cancel");
    assert!(app.surface_path_prompt.is_none());
    assert_eq!(app.document.object(s), Some(&source));
}
#[test]
fn surface_path_invalid_curve_and_changed_sources_keep_the_picker() {
    let (mut app, s, path) = setup();
    app.document
        .select_objects_direct([s], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ExtrudeSrfAlongCrv");
    let before = format!("{:?}", app.document);
    app.apply_selection_click(SelectionClick {
        object_id: Some(s),
        mode: SelectionMode::Replace,
    });
    assert!(app.surface_path_prompt.is_some());
    assert_eq!(format!("{:?}", app.document), before);
    app.document
        .replace_object_geometries([(s, Geometry::Point(point(1., 2., 3.)))])
        .unwrap();
    let before = format!("{:?}", app.document);
    app.apply_selection_click(SelectionClick {
        object_id: Some(path),
        mode: SelectionMode::Replace,
    });
    assert!(app.surface_path_prompt.is_some());
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.command_log.back().unwrap().contains("source changed"));
}
#[cfg(feature = "native-smlib")]
#[test]
fn surface_path_component_sources_and_clicked_path_commit_one_history_entry() {
    let (mut app, s, path) = setup();
    let original = app.document.object(s).unwrap().clone();
    let rail = app.document.object(path).unwrap().clone();
    app.accept_component_click(crate::viewport::ComponentClick {
        picks: vec![crate::viewport::ComponentPick {
            object: s,
            kind: viboceros_command::ComponentSelectionKind::BrepFace,
            index: 0,
        }],
        preselection: true,
        modifiers: egui::Modifiers::default(),
    });
    enter(&mut app, "ExtrudeSrfAlongCrv DeleteInput=Yes");
    assert!(app.object_prompt.is_none());
    assert!(app.surface_path_prompt.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(path),
        mode: SelectionMode::Replace,
    });
    assert!(app.surface_path_prompt.is_none());
    assert!(app.document.object(s).is_none());
    assert_eq!(app.document.object(path), Some(&rail));
    let Geometry::Brep(b) = app.document.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(b.is_solid());
    assert!((b.signed_volume(app.document.tolerance()).unwrap().abs() - 12.).abs() < 1e-5);
    enter(&mut app, "Undo");
    assert_eq!(app.document.object(s), Some(&original));
    enter(&mut app, "Redo");
    assert!(app.document.object(s).is_none());
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn surface_path_kernel_decline_retains_source_picks_and_getter() {
    let (mut app, s, path) = setup();
    app.document
        .select_objects_direct([s], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ExtrudeSrfAlongCrv");
    let before = format!("{:?}", app.document);
    app.apply_selection_click(SelectionClick {
        object_id: Some(path),
        mode: SelectionMode::Replace,
    });
    assert!(app.surface_path_prompt.is_some());
    assert_eq!(format!("{:?}", app.document), before);
}
