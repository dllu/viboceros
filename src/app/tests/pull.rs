use super::*;
#[test]
fn pull_command_first_selects_sources_and_targets_then_replays_one_step() {
    let (mut app, ids) = setup();
    app.document.clear_selection();
    enter(&mut app, "Pull");
    enter(&mut app, &ids[0].to_string());
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, &ids[1].to_string());
    enter(&mut app, "");
    assert_eq!(app.intersection_prompt.as_ref().unwrap().name(), "Pull");
    enter(&mut app, &ids[0].to_string());
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().len(), 4);
    assert_eq!(app.document.undo_label(), Some("Pull"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 3);
    enter(&mut app, "Redo");
    assert_eq!(app.document.selected_object_count(), 1);
}
#[test]
fn pull_mixed_preselection_executes_and_preserves_its_source() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Pull");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.selected_object_count(), 2);
    for id in ids {
        assert!(app.document.object(id).is_some());
    }
}
#[test]
fn pull_options_persist_independently_from_project_and_reject_direction() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct([ids[1]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Pull");
    enter(&mut app, "Direction=Custom");
    assert!(!app.picking_project_direction());
    enter(&mut app, "DeleteInput=Yes");
    enter(&mut app, "OutputLayer=Input");
    app.cancel_interactive_command(true);
    assert!(app.document.is_selected(ids[1]));
    enter(&mut app, "Pull");
    assert!(app.command_log.back().unwrap().contains("DeleteInput=Yes"));
    app.cancel_interactive_command(true);
    enter(&mut app, "Project");
    assert!(app.command_log.back().unwrap().contains("DeleteInput=No"));
    app.cancel_interactive_command(true);
    enter(&mut app, "Pull");
    enter(&mut app, &ids[0].to_string());
    enter(&mut app, "");
    assert!(app.document.object(ids[1]).is_none());
    assert_eq!(app.document.undo_label(), Some("Pull"));
}
#[test]
fn pull_infinite_plane_uses_the_shared_point_getter() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct([ids[2]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Pull");
    enter(&mut app, "IP");
    enter(&mut app, "ThroughPoint");
    assert_eq!(app.active_command.unwrap().name(), "Pull");
    enter(&mut app, "w0,0,2");
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_none());
    let Geometry::Point(p) = app.document.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(*p, point(5., 5., 2.));
}
#[test]
fn pull_temporary_ranges_keep_the_original_despite_delete_input() {
    let (mut app, ids) = setup();
    app.document.clear_selection();
    enter(&mut app, "Pull DeleteInput=Yes");
    enter(&mut app, "SubCrv");
    enter(&mut app, &ids[1].to_string());
    enter(&mut app, "w3,5,3");
    enter(&mut app, "w7,5,3");
    enter(&mut app, "");
    enter(&mut app, &ids[0].to_string());
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_none());
    assert!(app.document.object(ids[1]).is_some());
    let curve = app
        .document
        .selected_objects()
        .next()
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap();
    assert!((curve.length(app.document.tolerance()).unwrap() - 4.).abs() < 1e-8);
}
#[test]
fn pull_pending_plane_cancel_restores_selection_without_a_model_edit() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct([ids[2]], SelectionMode::Replace)
        .unwrap();
    let history = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "Pull");
    enter(&mut app, "IP");
    enter(&mut app, "3Point");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "Undo");
    assert_eq!(app.document.undo_label(), history.as_deref());
    enter(&mut app, "Cancel");
    assert!(app.intersection_prompt.is_none());
    assert!(app.document.is_selected(ids[2]));
    assert_eq!(app.document.objects().len(), 3);
}
