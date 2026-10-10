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

#[test]
fn command_first_edge_zoom_options_collect_sources_and_group_all_marks() {
    let mut app = test_app();
    enter(&mut app, "SrfPt 0,0,0 2,0,0 2,3,0 0,3,0");
    enter(&mut app, "SelNone");
    enter(&mut app, "ZoomNaked");
    assert!(app.object_prompt.is_some());
    enter(&mut app, "SelAll");
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert!(app.edge_zoom_prompt.is_some());
    assert!(
        app.commands
            .edge_analysis_view(&app.document)
            .unwrap()
            .is_none()
    );
    enter(&mut app, "Mark");
    enter(&mut app, "Next");
    enter(&mut app, "Mark");
    assert_eq!(app.document.objects().len(), 5);
    enter(&mut app, "");
    assert!(app.edge_zoom_prompt.is_none());
    assert!(
        app.commands
            .edge_zoom_view(&app.document)
            .unwrap()
            .is_none()
    );
    assert_eq!(app.document.undo_label(), Some("ZoomNaked"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 5);
}

#[test]
fn zoom_escape_keeps_accepted_marks_and_preserves_the_analysis_panel() {
    let mut app = test_app();
    enter(&mut app, "Box 0,0,0 2,3,0 4");
    enter(&mut app, "SelAll");
    enter(&mut app, "ShowEdges Show=All Color=20,40,60");
    let panel = app
        .commands
        .edge_analysis_view(&app.document)
        .unwrap()
        .unwrap();
    enter(&mut app, "SrfPt 5,0,0 7,0,0 7,3,0 5,3,0");
    let surface = app.document.objects().last().unwrap().id();
    app.document
        .select_objects_direct([surface], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ZoomNaked");
    enter(&mut app, "Mark");
    app.cancel_interactive_command(true);
    assert_eq!(app.document.objects().len(), 4);
    assert!(
        app.commands
            .edge_zoom_view(&app.document)
            .unwrap()
            .is_none()
    );
    let retained = app
        .commands
        .edge_analysis_view(&app.document)
        .unwrap()
        .unwrap();
    assert_eq!(retained.color, [20, 40, 60]);
    assert!(std::sync::Arc::ptr_eq(&panel.edges, &retained.edges));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn complete_edge_zoom_arguments_survive_command_first_selection() {
    let mut app = test_app();
    enter(&mut app, "SrfPt 0,0,0 2,0,0 2,3,0 0,3,0");
    enter(&mut app, "SelNone");
    enter(&mut app, "ZoomNaked All Mark");
    assert!(app.object_prompt.is_some());
    enter(&mut app, "SelAll");
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 9);
    assert!(app.edge_zoom_prompt.is_none());
    assert!(
        app.commands
            .edge_analysis_view(&app.document)
            .unwrap()
            .is_none()
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
}

#[test]
fn empty_naked_set_and_invalid_options_do_not_start_or_mutate_a_model() {
    let mut app = test_app();
    enter(&mut app, "Box 0,0,0 2,3,0 4");
    enter(&mut app, "SelAll");
    let label = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "ZoomNaked");
    assert!(app.edge_zoom_prompt.is_none());
    assert_eq!(app.document.undo_label(), label.as_deref());
    enter(&mut app, "SrfPt 5,0,0 7,0,0 7,3,0 5,3,0");
    let surface = app.document.objects().last().unwrap().id();
    app.document
        .select_objects_direct([surface], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ZoomNaked");
    let before = app.document.objects().len();
    enter(&mut app, "unexpected");
    assert!(app.edge_zoom_prompt.is_some());
    assert_eq!(app.document.objects().len(), before);
    enter(&mut app, "Line");
    assert!(app.edge_zoom_prompt.is_none());
    assert!(app.active_command.is_some());
}

#[test]
fn show_edges_options_and_source_addition_survive_object_prompts() {
    let mut app = test_app();
    enter(&mut app, "Box 0,0,0 2,3,0 4");
    enter(&mut app, "SelNone");
    enter(&mut app, "ShowEdges Show=All Color=20,40,60");
    enter(&mut app, "SelAll");
    enter(&mut app, "");
    let view = app
        .commands
        .edge_analysis_view(&app.document)
        .unwrap()
        .unwrap();
    assert_eq!(view.displayed().count(), 12);
    assert_eq!(view.color, [20, 40, 60]);
    enter(&mut app, "SrfPt 5,0,0 7,0,0 7,3,0 5,3,0");
    enter(&mut app, "SelNone");
    enter(&mut app, "ShowEdges Add");
    assert!(app.object_prompt.is_some());
    enter(&mut app, "SelAll");
    enter(&mut app, "");
    assert_eq!(
        app.commands
            .edge_analysis_view(&app.document)
            .unwrap()
            .unwrap()
            .sources,
        2
    );
}
