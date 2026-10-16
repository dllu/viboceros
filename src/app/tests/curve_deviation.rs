use super::*;
use viboceros_document::SelectionMode;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn setup() -> (VibocerosApp, Vec<viboceros_document::ObjectId>) {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 10,0,0");
    enter(&mut app, "Line 0,2,0 10,2,0");
    let ids = app.document.objects().map(|o| o.id()).collect();
    (app, ids)
}
#[test]
fn crv_deviation_command_first_picker_requires_two_curves_and_enter_finishes() {
    let (mut app, ids) = setup();
    enter(&mut app, "CrvDeviation");
    assert!(app.object_prompt.is_some());
    app.document
        .select_objects_direct([ids[0]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "");
    assert!(app.object_prompt.is_some());
    app.document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert!(app.commands.curve_deviation_view(&app.document).is_some());
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "KeepMarks=Yes");
    assert!(
        app.commands
            .curve_deviation_view(&app.document)
            .unwrap()
            .keep_marks
    );
    enter(&mut app, "");
    assert!(app.commands.curve_deviation_view(&app.document).is_none());
    assert_eq!(app.document.objects().len(), 14);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 2);
}
#[test]
fn crv_deviation_preselection_escape_and_replacement_dismiss_without_edits() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct(ids.clone(), SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "CrvDeviation KeepMarks=Yes");
    assert!(app.commands.curve_deviation_view(&app.document).is_some());
    app.cancel_current_prompt_or_selection();
    assert!(app.commands.curve_deviation_view(&app.document).is_none());
    assert_eq!(app.document.objects().len(), 2);
    app.document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "CrvDeviation");
    enter(&mut app, "Distance");
    assert!(app.commands.curve_deviation_view(&app.document).is_none());
    assert_eq!(app.document.objects().len(), 2);
}
