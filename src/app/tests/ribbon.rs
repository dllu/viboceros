use super::boolean_intersection::{enter, pick};
use super::*;

fn fixture() -> (VibocerosApp, ObjectId) {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 5,0,0");
    let id = app.document.objects().next().unwrap().id();
    app.document.clear_selection();
    app.document.clear_history().unwrap();
    (app, id)
}
#[test]
fn preselected_ribbon_edits_options_measures_distance_and_retains_sources() {
    let (mut app, id) = fixture();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Ribbon");
    assert!(app.ribbon_session.is_some(), "{:?}", app.command_log);
    enter(&mut app, "Corner");
    enter(&mut app, "Smooth");
    enter(&mut app, "Ortho");
    assert!(app.ribbon_session.is_some());
    enter(&mut app, "Ortho");
    assert!(app.ribbon_session.is_some());
    enter(&mut app, "Distance");
    enter(&mut app, "0");
    assert!(app.ribbon_session.is_some());
    enter(&mut app, "0,0,0");
    enter(&mut app, "0,3,0");
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "w0,5,0");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().len(), 2);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![id]
    );
    assert_eq!(app.commands.ribbon_options().distance, 3.);
    assert_eq!(
        app.commands.ribbon_options().corner,
        viboceros_geometry::RibbonCornerStyle::Smooth
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.commands.ribbon_options().distance, 3.);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 2);
}
#[test]
fn command_first_curve_pick_finishes_without_enter_and_releases_selection_on_replay() {
    let (mut app, id) = fixture();
    enter(&mut app, "Ribbon Distance=.5 OutputLayer=Input");
    assert!(app.object_prompt.is_some(), "{:?}", app.command_log);
    pick(&mut app, id);
    assert!(app.ribbon_session.is_some(), "{:?}", app.command_log);
    assert!(app.ribbon_session.as_ref().unwrap().postselected);
    enter(&mut app, "0,2,0");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().len(), 2);
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Redo");
    assert_eq!(app.document.selected_object_count(), 0);
}
#[test]
fn canceled_and_stale_ribbon_drafts_do_not_create_outputs_or_history() {
    let (mut app, id) = fixture();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Ribbon");
    enter(&mut app, "Distance");
    enter(&mut app, "0,0,0");
    enter(&mut app, "0,0,0");
    assert!(app.ribbon_session.is_some());
    app.cancel_current_prompt_or_selection();
    assert!(app.command_line_idle());
    assert!(app.document.is_selected(id));
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
    enter(&mut app, "Ribbon");
    app.document
        .set_object_names([(id, Some("changed".to_owned()))])
        .unwrap();
    enter(&mut app, "0,2,0");
    assert!(app.command_line_idle());
    assert_eq!(app.document.objects().len(), 1);
}
