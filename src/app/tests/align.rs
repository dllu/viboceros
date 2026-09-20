use super::*;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn setup() -> VibocerosApp {
    let mut app = test_app();
    for text in ["Point 0,0,1", "Point 4,8,3", "SelAll"] {
        enter(&mut app, text);
    }
    app
}
fn positions(app: &VibocerosApp) -> Vec<Geometry> {
    app.document
        .objects()
        .map(|o| o.geometry().clone())
        .collect()
}

#[test]
fn selection_mode_and_auto_target_are_distinct_phases() {
    let mut app = setup();
    enter(&mut app, "SelNone");
    enter(&mut app, "Align");
    assert!(app.object_prompt.is_some());
    assert!(app.active_command.is_none());
    assert_eq!(app.document.selected_object_ids().count(), 0);
    enter(&mut app, "SelAll");
    enter(&mut app, "");
    assert!(
        matches!(app.active_command,Some(InteractiveCommand::Align { options, .. }) if options.mode.is_none())
    );
    let before = positions(&app);
    enter(&mut app, "");
    assert_eq!(positions(&app), before);
    assert!(!app.accept_drafting_point(point(20., 20., 20.)));
    enter(&mut app, "HorizCenter");
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    assert_eq!(
        positions(&app),
        vec![
            Geometry::Point(point(0., 4., 1.)),
            Geometry::Point(point(4., 4., 3.))
        ]
    );
    assert_eq!(app.document.undo_label(), Some("Align"));
    enter(&mut app, "Undo");
    assert_eq!(positions(&app), before);
}

#[test]
fn picked_and_typed_targets_share_the_same_edit_and_retain_failed_prompts() {
    let mut typed = setup();
    let mut picked = setup();
    for app in [&mut typed, &mut picked] {
        enter(app, "Align Concentric AlignTo=World");
    }
    enter(&mut typed, "wNaN,0,0");
    assert!(typed.active_command.is_some());
    enter(&mut typed, "w20,-4,17");
    assert!(picked.accept_drafting_point(point(20., -4., 17.)));
    assert_eq!(positions(&typed), positions(&picked));
    assert_eq!(
        positions(&typed),
        vec![
            Geometry::Point(point(20., -4., 1.)),
            Geometry::Point(point(20., -4., 3.))
        ]
    );
}

#[test]
fn alignment_can_be_cancelled_or_completed_without_a_pick() {
    let mut app = setup();
    let before = positions(&app);
    enter(&mut app, "Align Top");
    enter(&mut app, "AlignTo=World");
    app.cancel_interactive_command(true);
    assert_eq!(positions(&app), before);
    enter(&mut app, "Align Left Auto");
    assert!(app.active_command.is_none());
    assert_eq!(
        positions(&app),
        vec![
            Geometry::Point(point(0., 0., 1.)),
            Geometry::Point(point(0., 8., 3.))
        ]
    );
    enter(&mut app, "Undo");
    enter(&mut app, "Align Right");
    enter(&mut app, "Auto");
    assert!(app.active_command.is_none());
    assert_eq!(
        positions(&app),
        vec![
            Geometry::Point(point(4., 0., 1.)),
            Geometry::Point(point(4., 8., 3.))
        ]
    );
}

#[test]
fn command_first_mode_and_frame_choices_survive_object_selection() {
    let mut app = setup();
    enter(&mut app, "SelNone");
    enter(&mut app, "Align VertCenter AlignTo=World");
    enter(&mut app, "SelAll");
    enter(&mut app, "");
    assert!(
        matches!(app.active_command,Some(InteractiveCommand::Align { options, .. }) if options.world && options.mode==Some(viboceros_command::AlignmentMode::VertCenter))
    );
    enter(&mut app, "w10,100,1000");
    assert_eq!(
        positions(&app),
        vec![
            Geometry::Point(point(10., 0., 1.)),
            Geometry::Point(point(10., 8., 3.))
        ]
    );
}

#[test]
fn cancelling_command_first_alignment_releases_prompt_selection() {
    let mut app = setup();
    let before = positions(&app);
    enter(&mut app, "SelNone");
    enter(&mut app, "Align");
    enter(&mut app, "SelAll");
    enter(&mut app, "");
    assert!(app.active_command.is_some());
    app.cancel_interactive_command(true);
    assert_eq!(app.document.selected_object_ids().count(), 0);
    assert_eq!(positions(&app), before);
}
