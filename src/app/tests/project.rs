use super::*;
use viboceros_document::SelectionMode;
#[path = "project_plane.rs"]
mod infinite;
#[path = "project_subcurve.rs"]
mod subcurve;
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

#[test]
fn project_mesh_target_is_pickable_and_mixed_preselection_completes() {
    let mut app = test_app();
    enter(&mut app, "MeshPlane 0,0 2,2 XCount=2 YCount=2");
    enter(&mut app, "Point 1,1,3");
    let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
    assert_eq!(ids.len(), 2);
    app.document
        .select_objects_direct([ids[1]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Project");
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.selected_object_count(), 1);
    enter(&mut app, "Undo");
    app.document
        .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Project");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().len(), 3);
}

#[test]
fn custom_project_two_point_getter_defers_mixed_preselection_and_preserves_history() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct([ids[0], ids[2]], SelectionMode::Replace)
        .unwrap();
    let history = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "Project Direction=Custom");
    assert!(app.picking_project_direction());
    assert_eq!(app.document.objects().len(), 3);
    assert!(app.viewport_object_filter().is_none());
    assert!(app.accept_drafting_point(point(0., 0., 0.)));
    assert_eq!(app.document.undo_label(), history.as_deref());
    assert!(!app.accept_drafting_point(point(0., 0., 0.)));
    assert!(app.picking_project_direction());
    assert!(app.accept_drafting_point(point(1., 0., 1.)));
    assert!(app.intersection_prompt.is_none());
    assert!(!app.picking_project_direction());
    assert_eq!(app.document.objects().len(), 4);
    let Geometry::Point(p) = app.document.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(p.distance_to(point(8., 5., 0.)).unwrap() < 1e-8);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 3);
    enter(&mut app, "Redo");
    assert_eq!(app.document.selected_object_count(), 1);
}
#[test]
fn custom_project_typed_world_relative_units_and_cplane_changes_share_point_routing() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct([ids[2]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Project");
    enter(&mut app, "Direction=Custom");
    enter(&mut app, "w0,0,0");
    app.viewports[app.active_viewport].set_construction_plane(
        Frame3::try_from_directions(
            point(0., 0., 0.),
            viboceros_geometry::Vector3::try_new(0., 1., 0.).unwrap(),
            viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap(),
            app.document.tolerance(),
        )
        .unwrap(),
    );
    enter(&mut app, "r1mm,1mm,0");
    assert!(!app.picking_project_direction());
    enter(&mut app, &ids[0].to_string());
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 4);
    let Geometry::Point(p) = app.document.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(p.distance_to(point(5., 8., 0.)).unwrap() < 1e-8);
}
#[test]
fn custom_project_point_filters_constraints_undo_and_cancel_are_transient() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct([ids[2]], SelectionMode::Replace)
        .unwrap();
    let history = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "Project");
    enter(&mut app, "Direction=Custom");
    enter(&mut app, "0,0,0");
    enter(&mut app, "2");
    assert!(app.point_constraint.is_some());
    enter(&mut app, "Undo");
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::ProjectDirection { start: None })
    );
    assert!(app.point_constraint.is_none());
    assert_eq!(app.document.undo_label(), history.as_deref());
    enter(&mut app, ".xy");
    assert!(app.point_filter.is_some());
    assert!(app.accept_filtered_drafting_point(point(0., 0., 8.), false));
    assert!(app.picking_project_direction());
    assert!(app.accept_filtered_drafting_point(point(9., 9., 0.), false));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::ProjectDirection {
            start: Some(point(0., 0., 0.))
        })
    );
    let before_direction = app.project_options.command();
    enter(&mut app, "Cancel");
    assert!(app.intersection_prompt.is_none());
    assert!(app.active_command.is_none());
    assert!(app.point_filter.is_none());
    assert_eq!(app.project_options.command(), before_direction);
    assert!(app.document.is_selected(ids[2]));
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.undo_label(), history.as_deref());
}
#[test]
fn custom_project_getter_keeps_options_and_can_switch_back_to_cplane_direction() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct([ids[0], ids[2]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Project Direction=Custom");
    enter(&mut app, "OutputLayer=Input");
    assert!(app.picking_project_direction());
    enter(&mut app, "DeleteInput=No");
    assert!(app.picking_project_direction());
    let selected = app.document.selected_object_ids().collect::<Vec<_>>();
    enter(&mut app, "SelNone");
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        selected
    );
    enter(&mut app, "Direction=CPlaneZ");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().len(), 4);
}
#[test]
fn custom_project_direction_persists_but_an_incomplete_replacement_does_not() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct([ids[2]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Project Direction=Custom");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w1,0,1");
    let saved = app.project_options.command();
    app.cancel_interactive_command(true);
    app.document
        .select_objects_direct([ids[2]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Project Direction=Custom");
    enter(&mut app, "w0,0,0");
    app.cancel_current_prompt_or_selection();
    assert_eq!(app.project_options.command(), saved);
    enter(&mut app, "Project");
    assert!(!app.picking_project_direction());
    enter(&mut app, &ids[0].to_string());
    enter(&mut app, "");
    let Geometry::Point(p) = app.document.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(p.distance_to(point(8., 5., 0.)).unwrap() < 1e-8);
}

#[test]
fn custom_project_getter_can_be_replaced_without_consuming_the_next_commands_points() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct([ids[2]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Project Direction=Custom");
    enter(&mut app, "0,0,0");
    enter(&mut app, "Line");
    assert!(app.intersection_prompt.is_none());
    assert!(!app.picking_project_direction());
    enter(&mut app, "0,0,0");
    enter(&mut app, "1,2,3");
    assert_eq!(app.document.objects().len(), 4);
    assert_eq!(app.document.undo_label(), Some("Line"));
    let Geometry::Line(line) = app.document.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(line.end(), point(1., 2., 3.));
}

#[test]
fn custom_project_real_viewport_clicks_snap_reference_endpoints_and_resume_selection() {
    let (mut app, ids) = setup();
    enter(&mut app, "Line 1,0,0 1,0,1");
    app.viewports[0] = Viewport::new(crate::viewport::ViewKind::Front);
    app.active_viewport = 0;
    app.document
        .select_objects_direct([ids[2]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Project Direction=Custom");
    let context = egui::Context::default();
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
    let frame = |app: &mut VibocerosApp, events: Vec<egui::Event>| {
        let mut output = ViewportOutput::default();
        let filter = app.viewport_object_filter();
        let input = ViewportInput {
            object_filter: filter,
            drafting: DraftingInput {
                active: app.picking_project_direction(),
                osnap: viboceros_drafting::ObjectSnapModes::only(
                    viboceros_drafting::ObjectSnapKind::End,
                ),
                anchor: app.active_command.and_then(InteractiveCommand::anchor),
                ..Default::default()
            },
            ..Default::default()
        };
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(rect),
                    events,
                    ..Default::default()
                },
                |ui| {
                    output = app.viewports[0].show(ui, &app.document, input, &[], 0, true);
                },
            )
            .drop_without_applying_deltas();
        output
    };
    frame(&mut app, vec![]);
    for expected in [point(1., 0., 0.), point(1., 0., 1.)] {
        let position = app.viewports[0].project(expected, rect).unwrap() + egui::vec2(1., 1.);
        let mut picked = false;
        for pressed in [true, false] {
            let output = frame(
                &mut app,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            if let Some(p) = output.picked_point {
                assert!(p.distance_to(expected).unwrap() < 1e-9);
                picked = true;
            }
            app.handle_viewport_action(output);
        }
        assert!(picked);
    }
    assert!(!app.picking_project_direction());
    assert!(app.intersection_prompt.is_some());
    assert_eq!(app.document.objects().len(), 4);
    enter(&mut app, &ids[0].to_string());
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 5);
    let Geometry::Point(p) = app.document.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(p.distance_to(point(5., 5., 0.)).unwrap() < 1e-8);
}
