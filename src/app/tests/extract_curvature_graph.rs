use super::*;
use viboceros_document::SelectionMode;
fn enter(app: &mut VibocerosApp, s: &str) {
    app.command_input = s.into();
    app.run_command();
}
fn scene() -> (VibocerosApp, Vec<viboceros_document::ObjectId>) {
    let mut app = test_app();
    enter(&mut app, "Circle 0,0,0 2");
    enter(&mut app, "SelAll");
    enter(&mut app, "CurvatureGraph");
    enter(&mut app, "Circle 5,0,0 2");
    let ids = app.document.objects().map(|o| o.id()).collect();
    (app, ids)
}
#[test]
fn extract_curvature_graph_picker_accepts_only_graphed_curves_and_clears_postselection() {
    let (mut app, ids) = scene();
    enter(&mut app, "SelNone");
    enter(&mut app, "ExtractCurvatureGraph");
    assert!(app.object_prompt.is_some());
    app.select_prompt_objects([ids[1]], SelectionMode::Add);
    assert!(!app.document.is_selected(ids[1]));
    app.select_prompt_objects([ids[0]], SelectionMode::Add);
    assert!(app.document.is_selected(ids[0]));
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.selected_object_count(), 0);
    assert!(
        app.commands
            .curvature_graph_view(&app.document)
            .unwrap()
            .is_some()
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 2);
}
#[test]
fn extract_curvature_graph_ignores_ungraphed_preselection_and_empty_enter_cancels() {
    let (mut app, ids) = scene();
    app.document
        .select_objects_direct([ids[1]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ExtractCurvatureGraph");
    assert!(app.object_prompt.is_some());
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().len(), 2);
    app.document
        .select_objects_direct([ids[0]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ExtractCurvatureGraph");
    assert!(app.object_prompt.is_none());
    assert!(app.document.is_selected(ids[0]));
    assert_eq!(app.document.objects().len(), 3);
}
