use super::*;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn edge_analysis_command_first_picking_and_modes_do_not_change_geometry_or_history() {
    let mut app = test_app();
    enter(&mut app, "Box 0,0,0 2,3,0 4");
    enter(&mut app, "SelNone");
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    let label = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "ShowEdges");
    assert!(app.object_prompt.is_some());
    enter(&mut app, "SelAll");
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    enter(&mut app, "ShowEdges Show=All Color=20,40,60");
    let view = app
        .commands
        .edge_analysis_view(&app.document)
        .unwrap()
        .unwrap();
    assert_eq!(view.displayed().count(), 12);
    assert_eq!(view.color, [20, 40, 60]);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(app.document.undo_label(), label.as_deref());
    enter(&mut app, "ShowEdgesOff");
    assert!(
        app.commands
            .edge_analysis_view(&app.document)
            .unwrap()
            .is_none()
    );
}
#[test]
fn edge_analysis_marks_selected_edge_ends_and_undo_preserves_analysis_session() {
    let mut app = test_app();
    enter(&mut app, "Box 0,0,0 2,3,0 4");
    enter(&mut app, "SelAll");
    enter(&mut app, "ShowEdges Show=All");
    enter(&mut app, "ShowEdges Zoom Current");
    enter(&mut app, "ShowEdges Mark");
    assert_eq!(app.document.objects().len(), 3);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    assert!(
        app.commands
            .edge_analysis_view(&app.document)
            .unwrap()
            .is_some()
    );
}
