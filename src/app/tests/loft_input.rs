use super::*;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

fn circle(app: &mut VibocerosApp) -> ObjectId {
    app.commands
        .execute(&mut app.document, "Circle 0,0,1 1")
        .unwrap();
    app.document.objects().last().unwrap().id()
}

fn pick(app: &mut VibocerosApp, id: ObjectId) {
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
}

#[test]
fn picked_loft_poles_share_snapping_and_commit_one_undo_step() {
    let mut app = test_app();
    let id = circle(&mut app);
    app.document.clear_history().unwrap();
    let source = app.document.object(id).unwrap().clone();
    enter(&mut app, "Loft Point");
    assert_eq!(app.active_command, Some(InteractiveCommand::LoftPoint));
    assert_eq!(app.viewport_object_filter(), None);
    enter(&mut app, "w0,0,-1");
    assert_eq!(app.active_command, None);
    assert_eq!(
        app.viewport_object_filter(),
        Some(viboceros_command::ObjectSelectionFilter::Curves)
    );
    pick(&mut app, id);
    enter(&mut app, "Point");
    assert_eq!(app.active_command, Some(InteractiveCommand::LoftPoint));
    enter(&mut app, "Point"); // Now this is the one-shot Point object snap.
    assert!(app.snaps.model_override.is_some());
    assert!(app.accept_filtered_drafting_point(point(0., 0., 3.), false));
    assert!(app.snaps.model_override.is_none());
    assert_eq!(app.document.objects().count(), 1);
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "");
    assert!(app.loft_prompt.is_none(), "{:?}", app.command_log);
    let output = app.document.objects().find(|o| o.id() != id).unwrap();
    let Geometry::Brep(brep) = output.geometry() else {
        panic!("Loft B-rep")
    };
    assert!(brep.is_solid());
    assert!(
        brep.vertices()
            .iter()
            .any(|v| v.point() == point(0., 0., -1.))
    );
    assert!(
        brep.vertices()
            .iter()
            .any(|v| v.point() == point(0., 0., 3.))
    );
    assert_eq!(&source, app.document.object(id).unwrap());
    assert_eq!(app.document.undo_label(), Some("Loft"));
    let after = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), 1);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), after);
}

#[test]
fn ordered_curve_picks_ignore_groups_and_regions_filter_noncurves() {
    let mut app = test_app();
    for line in [
        "Line 0,0,0 1,0,0",
        "Line 0,0,3 2,0,3",
        "Line 0,0,5 1,0,5",
        "Point 5,5,5",
    ] {
        app.commands.execute(&mut app.document, line).unwrap();
    }
    let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
    app.document
        .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    app.commands
        .execute(&mut app.document, "Group Profiles")
        .unwrap();
    app.document.clear_selection();
    enter(&mut app, "Loft");
    pick(&mut app, ids[2]);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[2]]
    );
    app.apply_selection_window(SelectionWindow {
        object_ids: vec![ids[0], ids[1], ids[3]],
        mode: SelectionMode::Replace,
        crossing: false,
        inverted: false,
    });
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[2], ids[0], ids[1]]
    );
    enter(&mut app, "Type");
    enter(&mut app, "Straight");
    let mut expected = app.document.clone();
    app.commands
        .execute(&mut expected, "Loft Type=Straight")
        .unwrap();
    enter(&mut app, "");
    assert_eq!(
        app.document.objects().last().unwrap().geometry(),
        expected.objects().last().unwrap().geometry()
    );
    assert_eq!(
        app.document.objects().last().unwrap().attributes(),
        expected.objects().last().unwrap().attributes()
    );
    assert!(
        app.document
            .objects()
            .take(4)
            .eq(expected.objects().take(4))
    );
}

#[test]
fn loft_escape_and_undo_restore_picks_without_consuming_model_redo() {
    let mut app = test_app();
    let id = circle(&mut app);
    app.commands
        .execute(&mut app.document, "Point 9,9,9")
        .unwrap();
    app.document.undo().unwrap();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let redo = app.document.redo_label().map(str::to_owned);
    let before = format!("{:?}", app.document);
    enter(&mut app, "Loft");
    enter(&mut app, "Point");
    enter(&mut app, "w0,0,4");
    enter(&mut app, "Undo");
    assert_eq!(
        app.viewport_object_filter(),
        Some(viboceros_command::ObjectSelectionFilter::Curves)
    );
    enter(&mut app, "StartPoint");
    app.cancel_current_prompt_or_selection();
    assert!(app.command_line_idle());
    assert_eq!(before, format!("{:?}", app.document));
    assert_eq!(app.document.redo_label(), redo.as_deref());
    assert_eq!(app.document.selected_object_ids().collect::<Vec<_>>(), [id]);
}

#[test]
fn invalid_options_and_insufficient_picks_leave_loft_editable() {
    let mut app = test_app();
    let id = circle(&mut app);
    app.document.clear_history().unwrap();
    enter(&mut app, "Loft");
    enter(&mut app, "");
    assert!(app.loft_prompt.is_some());
    pick(&mut app, id);
    enter(&mut app, "Closed=Yes");
    enter(&mut app, "Point");
    assert_eq!(app.active_command, None);
    enter(&mut app, "Closed=No");
    enter(&mut app, "Point");
    enter(&mut app, "wNaN,0,4");
    assert_eq!(app.active_command, Some(InteractiveCommand::LoftPoint));
    enter(&mut app, "w0,0,4");
    enter(&mut app, "Type=Uniform");
    enter(&mut app, "Closed=Yes");
    assert_eq!(app.document.objects().count(), 1);
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "Type=Tight SplitAtTangents=Yes Natural");
    enter(&mut app, "");
    assert_eq!(app.document.objects().count(), 2, "{:?}", app.command_log);
}

#[test]
fn numeric_end_options_can_precede_interactive_curve_selection() {
    let mut app = test_app();
    let id = circle(&mut app);
    enter(&mut app, "Loft StartPoint=0,0,-1 EndPoint=0,0,4");
    pick(&mut app, id);
    enter(&mut app, "");
    assert_eq!(app.document.objects().count(), 2, "{:?}", app.command_log);
}

#[test]
fn loft_point_getter_survives_transparent_construction_plane_changes() {
    let mut app = test_app();
    let id = circle(&mut app);
    enter(&mut app, "Loft StartPoint");
    enter(&mut app, "CPlane 3Point");
    enter(&mut app, "w10,20,30");
    enter(&mut app, "w11,20,30");
    enter(&mut app, "w10,21,30");
    assert_eq!(app.active_command, Some(InteractiveCommand::LoftPoint));
    enter(&mut app, "-10,-20,-31"); // CPlane coordinate resolves to world (0,0,-1).
    pick(&mut app, id);
    enter(&mut app, "Point");
    enter(&mut app, "w0,0,4");
    enter(&mut app, "");
    let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
        panic!("Loft")
    };
    assert!(
        brep.vertices()
            .iter()
            .any(|v| v.point() == point(0., 0., -1.)),
        "{:?}",
        app.command_log
    );
}

#[test]
fn failed_loft_retains_picks_for_correction_and_preserves_model_history() {
    let mut app = test_app();
    let first = circle(&mut app);
    let duplicate = circle(&mut app);
    app.commands
        .execute(&mut app.document, "Circle 0,0,5 2")
        .unwrap();
    let last = app.document.objects().last().unwrap().id();
    app.document.clear_history().unwrap();
    enter(&mut app, "Loft");
    pick(&mut app, first);
    pick(&mut app, duplicate);
    let before = format!("{:?}", app.document);
    enter(&mut app, "");
    assert!(app.loft_prompt.is_some());
    assert_eq!(before, format!("{:?}", app.document));
    app.apply_selection_click(SelectionClick {
        object_id: Some(duplicate),
        mode: SelectionMode::Remove,
    });
    pick(&mut app, last);
    enter(&mut app, "");
    assert!(app.loft_prompt.is_none(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().count(), 4);
    assert_eq!(app.document.undo_label(), Some("Loft"));
}

#[test]
fn deleting_a_picked_source_cancels_loft_without_recreating_it() {
    let mut app = test_app();
    let id = circle(&mut app);
    enter(&mut app, "Loft");
    pick(&mut app, id);
    enter(&mut app, "Point");
    enter(&mut app, "w0,0,4");
    app.document.delete_object(id).unwrap();
    let label = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "");
    assert!(app.loft_prompt.is_none());
    assert_eq!(app.document.objects().count(), 0);
    assert_eq!(app.document.undo_label(), label.as_deref());
}

#[test]
fn loft_rejects_more_than_256_sections_before_accepting_a_pole() {
    let mut app = test_app();
    let line = viboceros_geometry::LineSegment::try_new(
        point(0., 0., 0.),
        point(1., 0., 0.),
        app.document.tolerance(),
    )
    .unwrap();
    let ids = (0..256)
        .map(|_| app.document.add_geometry(Geometry::Line(line)).unwrap())
        .collect::<Vec<_>>();
    enter(&mut app, "Loft");
    app.apply_selection_window(SelectionWindow {
        object_ids: ids.clone(),
        mode: SelectionMode::Add,
        crossing: true,
        inverted: false,
    });
    enter(&mut app, "Point");
    assert_eq!(app.active_command, None);
    enter(&mut app, "StartPoint=0,0,-1");
    assert_eq!(app.document.selected_object_count(), 256);
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[255]),
        mode: SelectionMode::Remove,
    });
    enter(&mut app, "Point");
    assert_eq!(app.active_command, Some(InteractiveCommand::LoftPoint));
    enter(&mut app, "Undo");
    assert_eq!(app.active_command, None);
}

#[test]
fn selection_commands_remain_inside_the_loft_getter() {
    let mut app = test_app();
    let id = circle(&mut app);
    app.commands
        .execute(&mut app.document, "Point 3,4,5")
        .unwrap();
    enter(&mut app, "Loft");
    enter(&mut app, "SelAll");
    assert_eq!(app.document.selected_object_ids().collect::<Vec<_>>(), [id]);
    enter(&mut app, "SelNone");
    assert!(app.loft_prompt.is_some());
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Undo");
    assert_eq!(app.document.selected_object_ids().collect::<Vec<_>>(), [id]);
    enter(&mut app, "Cancel");
    assert!(app.command_line_idle());
    assert_eq!(app.document.selected_object_count(), 0);
}

#[test]
fn loft_poles_accept_coordinate_filters_units_and_relative_coordinates() {
    let mut app = test_app();
    let id = circle(&mut app);
    enter(&mut app, "Loft Point");
    enter(&mut app, ".xy");
    enter(&mut app, "w0,0,999");
    assert_eq!(app.active_command, Some(InteractiveCommand::LoftPoint));
    enter(&mut app, "w9,8,-1cm");
    assert_eq!(app.active_command, None);
    pick(&mut app, id);
    enter(&mut app, "Point");
    enter(&mut app, "rw0,0,20mm");
    enter(&mut app, "");
    let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
        panic!("Loft")
    };
    for expected in [point(0., 0., -10.), point(0., 0., 10.)] {
        assert!(
            brep.vertices().iter().any(|v| v.point() == expected),
            "{:?}",
            app.command_log
        );
    }
}
