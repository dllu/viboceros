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
                [[0., 10.], [0., 8.], [0., 6.]],
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap()
}
fn pick(app: &mut VibocerosApp, id: ObjectId, index: usize, modifiers: egui::Modifiers) {
    app.accept_component_click(crate::viewport::ComponentClick {
        picks: vec![crate::viewport::ComponentPick {
            object: id,
            kind: viboceros_command::ComponentSelectionKind::BrepFace,
            index,
        }],
        preselection: false,
        modifiers,
    });
}
#[test]
fn shell_face_picker_options_units_and_cancel_preserve_geometry() {
    let mut app = test_app();
    let s = source(&mut app);
    let original = app.document.object(s).unwrap().clone();
    enter(&mut app, "Shell DeleteInput=No");
    assert!(app.shell_prompt.is_some());
    assert!(!app.command_line_idle());
    pick(&mut app, s, 0, Default::default());
    pick(&mut app, s, 1, Default::default());
    pick(
        &mut app,
        s,
        1,
        egui::Modifiers {
            ctrl: true,
            ..Default::default()
        },
    );
    assert_eq!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .len(),
        1
    );
    enter(&mut app, "Thickness");
    assert!(app.shell_prompt.unwrap().awaiting_thickness);
    enter(&mut app, "1cm");
    let options = app.shell_prompt.unwrap().options;
    assert_eq!(options.thickness, 10.);
    enter(&mut app, "gibberish");
    assert!(app.shell_prompt.is_some());
    enter(&mut app, "DeleteInput=Yes Thickness=0");
    assert_eq!(app.shell_prompt.unwrap().options, options);
    enter(&mut app, "Cancel");
    assert!(app.shell_prompt.is_none());
    assert_eq!(app.document.object(s), Some(&original));
}
#[test]
fn shell_empty_and_stale_faces_keep_the_picker() {
    let mut app = test_app();
    let s = source(&mut app);
    enter(&mut app, "Shell");
    enter(&mut app, "");
    assert!(app.shell_prompt.is_some());
    pick(&mut app, s, 0, Default::default());
    app.document
        .replace_object_geometries([(s, Geometry::Point(point(1., 2., 3.)))])
        .unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "");
    assert!(app.shell_prompt.is_some());
    assert_eq!(format!("{:?}", app.document), before);
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn shell_portable_failure_retains_face_picks() {
    let mut app = test_app();
    let s = source(&mut app);
    enter(&mut app, "Shell");
    pick(&mut app, s, 0, Default::default());
    let before = format!("{:?}", app.document);
    enter(&mut app, "");
    assert!(app.shell_prompt.is_some());
    assert_eq!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(format!("{:?}", app.document), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn shell_component_preselection_and_thickness_failure_repair_commit_one_history_step() {
    let mut app = test_app();
    let s = source(&mut app);
    let original = app.document.object(s).unwrap().clone();
    app.accept_component_click(crate::viewport::ComponentClick {
        picks: vec![crate::viewport::ComponentPick {
            object: s,
            kind: viboceros_command::ComponentSelectionKind::BrepFace,
            index: 0,
        }],
        preselection: true,
        modifiers: Default::default(),
    });
    enter(&mut app, "Shell Thickness=100");
    let before = format!("{:?}", app.document);
    enter(&mut app, "");
    assert!(app.shell_prompt.is_some());
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "Thickness=1");
    enter(&mut app, "");
    assert!(app.shell_prompt.is_none());
    assert!(app.document.object(s).is_none());
    assert!(
        matches!(app.document.selected_objects().next().unwrap().geometry(),Geometry::Brep(b)if b.is_solid())
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.object(s), Some(&original));
    enter(&mut app, "Redo");
    assert!(app.document.object(s).is_none());
}
