use super::*;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
#[test]
fn tapered_source_getter_preserves_inline_options_and_accepts_option_edits() {
    let mut app = test_app();
    enter(&mut app, "Circle 0,0,0 3");
    let id = app.document.objects().next().unwrap().id();
    enter(&mut app, "ExtrudeCrvTapered DraftAngle=12 Solid=Yes");
    assert!(app.object_prompt.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    let Some(InteractiveCommand::ExtrudeCurveTapered {
        base: None,
        options,
    }) = app.active_command
    else {
        panic!()
    };
    assert_eq!(options.draft_angle, 12.);
    assert!(options.solid);
    let before = format!("{:?}", app.document);
    enter(&mut app, "FlipAngle Corners=Round");
    let Some(InteractiveCommand::ExtrudeCurveTapered { options, .. }) = app.active_command else {
        panic!()
    };
    assert_eq!(options.draft_angle, -12.);
    assert!(options.round_corners);
    enter(&mut app, "DraftAngle=90");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::ExtrudeCurveTapered { .. })
    ));
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "Cancel");
    assert!(app.active_command.is_none());
}
#[cfg(feature = "native-smlib")]
#[test]
fn tapered_distance_getter_commits_and_replays_geometry() {
    let mut app = test_app();
    enter(&mut app, "Circle 0,0,0 3");
    enter(&mut app, "SelAll");
    enter(&mut app, "ExtrudeCrvTapered DraftAngle=10 Solid=Yes");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::ExtrudeCurveTapered { .. })
    ));
    enter(&mut app, "2");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().count(), 2);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), 1);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().count(), 2);
}
#[test]
fn tapered_failed_distance_keeps_the_getter_and_sources() {
    let mut app = test_app();
    enter(&mut app, "Circle 0,0,0 3");
    enter(&mut app, "SelAll");
    enter(&mut app, "ExtrudeCrvTapered");
    app.drafting_plane = Some(app.viewports[2].construction_plane());
    let plane = app.drafting_plane;
    let before = format!("{:?}", app.document);
    let active = app.active_command;
    enter(&mut app, "0");
    assert_eq!(app.drafting_plane, plane);
    assert_eq!(app.active_command, active);
    assert_eq!(format!("{:?}", app.document), before);
}

#[cfg(feature = "native-smlib")]
#[test]
fn tapered_direction_points_use_displacement_and_keep_the_base_after_failed_target() {
    let mut app = test_app();
    enter(&mut app, "Circle 0,0,0 3");
    enter(&mut app, "SelAll");
    let source = app.document.objects().next().unwrap().clone();
    enter(&mut app, "ExtrudeCrvTapered DraftAngle=10 Solid=Yes");
    let base = point(3., 4., 5.);
    assert!(app.accept_drafting_point(base));
    let active = app.active_command;
    assert!(!app.accept_drafting_point(base));
    assert_eq!(app.active_command, active);
    assert_eq!(app.document.object(source.id()), Some(&source));
    assert!(app.accept_drafting_point(point(3., 4., 7.)));
    assert!(app.active_command.is_none());
    let Geometry::Brep(b) = app.document.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(b.is_solid());
    assert!((b.bounds().max().z() - 2.).abs() < 1e-8);
}
