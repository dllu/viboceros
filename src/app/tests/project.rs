use super::*;
use viboceros_document::SelectionMode;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn setup() -> (VibocerosApp, Vec<viboceros_document::ObjectId>) {
    let mut app = test_app();
    for c in [
        "SrfPt 0,0,0 10,0,0 10,10,0 0,10,0",
        "Line 1,5,3 9,5,3",
        "Point 5,5,-3",
    ] {
        enter(&mut app, c);
    }
    let ids = app.document.objects().map(|o| o.id()).collect();
    (app, ids)
}
#[test]
fn project_picks_filtered_sources_and_targets_then_creates_one_history_edit() {
    let (mut app, ids) = setup();
    app.document.clear_selection();
    enter(&mut app, "Project");
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, &format!("{},{}", ids[1], ids[2]));
    enter(&mut app, "");
    assert!(app.intersection_prompt.as_ref().unwrap().first.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[1]),
        mode: SelectionMode::Replace,
    });
    assert_eq!(app.document.selected_object_count(), 0);
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().len(), 5);
    assert_eq!(app.document.selected_object_count(), 2);
    assert_eq!(app.document.undo_label(), Some("Project"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 3);
    enter(&mut app, "Redo");
    assert_eq!(app.document.selected_object_count(), 2);
}
#[test]
fn project_mixed_preselection_executes_immediately_and_cancel_restores_selection() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "_Project");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().len(), 5);
    enter(&mut app, "Undo");
    app.document
        .select_objects_direct(ids[1..].iter().copied(), SelectionMode::Replace)
        .unwrap();
    let history = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "Project");
    enter(&mut app, "OutputLayer=Input");
    app.cancel_interactive_command(true);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        ids[1..]
    );
    assert_eq!(app.document.undo_label(), history.as_deref());
}
#[test]
fn project_view_is_resolved_at_completion_and_options_persist_in_session() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct([ids[2]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Project");
    enter(&mut app, "Direction=View");
    enter(&mut app, &ids[0].to_string());
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().len(), 4);
    assert!(app.command_log.iter().any(|s| s.contains("ViewDirection=")));
    enter(&mut app, "Undo");
    app.document
        .select_objects_direct([ids[2]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Project");
    assert!(app.command_log.back().unwrap().contains("Direction=View"));
    enter(&mut app, "Direction=0,0,0");
    assert!(app.intersection_prompt.is_some());
    app.cancel_interactive_command(true);
    assert!(app.document.is_selected(ids[2]));
    enter(
        &mut app,
        &format!(
            "Project Sources={} Targets={} _Direction=_View _ViewDirection=0,0,1",
            ids[2], ids[0]
        ),
    );
    assert_eq!(app.document.objects().len(), 4);
}

#[test]
fn project_cplane_direction_uses_finishing_view_despite_previous_drawing_plane() {
    let (mut app, ids) = setup();
    enter(&mut app, "CPlane World Front");
    app.drafting_plane = Some(app.viewports[app.active_viewport].construction_plane());
    app.document
        .select_objects_direct([ids[2]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Project");
    // The target is chosen in a different construction plane.
    app.viewports[app.active_viewport]
        .set_construction_plane(viboceros_command::CommandContext::default().construction_plane);
    enter(&mut app, &ids[0].to_string());
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().len(), 4);
    assert!(app.document.selected_objects().any(|o|matches!(o.geometry(),Geometry::Point(p) if p.distance_to(point(5.,5.,0.)).unwrap()<1e-8)));
}

#[test]
fn project_prompt_can_be_replaced_by_a_command_with_options() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct([ids[2]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Project");
    enter(&mut app, "ProjectToCPlane DeleteInput=No");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().len(), 4);
    assert_eq!(app.document.undo_label(), Some("ProjectToCPlane"));
}
