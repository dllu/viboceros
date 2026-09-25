use super::*;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.to_owned();
    app.run_command();
}

#[test]
fn named_view_restores_camera_projection_and_cplane_in_another_viewport() {
    let mut app = test_app();
    enter(&mut app, "SetView World Perspective");
    enter(&mut app, "CPlane World Left");
    enter(&mut app, "SetView CPlane Front");
    let saved = app.viewports[0].named_view_snapshot();
    enter(&mut app, "NamedView Save Upper left");
    assert_eq!(
        app.named_views.names().collect::<Vec<_>>(),
        vec!["Upper left"]
    );
    let other_before = app.viewports[1].camera_snapshot();
    app.active_viewport = 1;
    app.viewports[1].display_mode = DisplayMode::Ghosted;
    enter(&mut app, "NamedView Restore Upper left");
    assert_eq!(app.viewports[1].named_view_snapshot(), saved);
    assert_eq!(app.viewports[1].display_mode, DisplayMode::Ghosted);
    assert_eq!(app.viewports[0].named_view_snapshot(), saved);
    assert_eq!(app.document.undo_label(), None);
    assert!(app.viewports[1].undo_view());
    assert_eq!(app.viewports[1].camera_snapshot(), other_before);
}

#[test]
fn named_view_edits_preserve_snapshot_and_reject_duplicate_names() {
    let mut app = test_app();
    enter(&mut app, "NamedView Save Front detail");
    let first = *app.named_views.get("front detail").unwrap();
    enter(&mut app, "NamedView Save FRONT DETAIL");
    assert_eq!(app.named_views.names().count(), 1);
    assert!(app.command_log.back().unwrap().contains("already exists"));
    enter(&mut app, "NamedView Duplicate Front detail | Copy");
    enter(&mut app, "NamedView Rename Copy | Work view");
    assert_eq!(*app.named_views.get("work view").unwrap(), first);
    enter(&mut app, "NamedView MoveUp Work view");
    assert_eq!(
        app.named_views.names().collect::<Vec<_>>(),
        vec!["Work view", "Front detail"]
    );
    enter(&mut app, "SetView World Right");
    enter(&mut app, "NamedView Update Work view");
    assert_ne!(*app.named_views.get("Work view").unwrap(), first);
    enter(&mut app, "NamedView Delete Front detail");
    enter(&mut app, "NamedView List");
    assert_eq!(app.command_log.back().unwrap(), "Named views: Work view");
}

#[test]
fn named_view_commands_leave_a_partial_modeling_prompt_and_redo_intact() {
    let mut app = test_app();
    for input in ["Point 1,2,3", "Undo", "Line", "0"] {
        enter(&mut app, input);
    }
    let pending = app.active_command;
    let drafting_plane = app.drafting_plane;
    enter(&mut app, "NamedView Save Line start");
    enter(&mut app, "NamedView Restore Line start");
    assert_eq!(app.active_command, pending);
    assert_eq!(app.drafting_plane, drafting_plane);
    assert!(app.document.can_redo());
    assert_eq!(app.document.objects().len(), 0);
    enter(&mut app, "1,2");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().len(), 1);
}
