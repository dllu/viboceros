use super::*;
use crate::app::tests::test_app;
fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn setup() -> (VibocerosApp, Vec<ObjectId>) {
    let mut app = test_app();
    for input in ["Line 0,0,0 0,0,5", "Circle 0,0,2.5 2", "Point 9,8,7"] {
        app.commands.execute(&mut app.document, input).unwrap();
    }
    let ids = app.document.objects().map(|o| o.id()).collect();
    app.document.clear_history().unwrap();
    (app, ids)
}
fn pick(app: &mut VibocerosApp, id: ObjectId) {
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
}
#[test]
fn rebuilding_sections_uses_shared_value_getters_local_undo_and_model_undo() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct([], SelectionMode::Replace)
        .unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(
        &mut app,
        &format!("Sweep1 RailId={} Parameters=2.5", ids[0]),
    );
    pick(&mut app, ids[1]);
    enter(&mut app, "Simplify");
    enter(&mut app, "Rebuild");
    assert_eq!(
        app.sweep_session
            .as_ref()
            .unwrap()
            .state
            .options
            .rebuild_point_count,
        5
    );
    enter(&mut app, "RebuildCount");
    enter(&mut app, "7");
    assert_eq!(
        app.sweep_session
            .as_ref()
            .unwrap()
            .state
            .options
            .rebuild_point_count,
        7
    );
    enter(&mut app, "Undo");
    assert_eq!(
        app.sweep_session
            .as_ref()
            .unwrap()
            .state
            .options
            .rebuild_point_count,
        5
    );
    enter(&mut app, "");
    assert!(app.sweep_session.is_none(), "{:?}", app.command_log);
    for source in &before {
        assert_eq!(app.document.object(source.id()), Some(source));
    }
    let surface = app
        .document
        .objects()
        .find_map(|o| match o.geometry() {
            Geometry::Brep(b) => Some(b.faces()[0].surface()),
            _ => None,
        })
        .unwrap();
    assert_eq!(surface.degree_v(), 3);
    assert_eq!(surface.control_point_count_v(), 6);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}
#[test]
fn sweep_picked_poles_use_snapping_filters_units_and_one_model_undo() {
    let (mut app, ids) = setup();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Sweep1 Parameters=2.5");
    // Explicit Parameters retain their established profile-first, rail-last order.
    pick(&mut app, ids[1]);
    pick(&mut app, ids[0]);
    enter(&mut app, "StartPoint");
    assert_eq!(app.active_command, Some(InteractiveCommand::SweepPoint));
    enter(&mut app, ".xy");
    enter(&mut app, "w0,0,999");
    enter(&mut app, "w9,8,0mm");
    assert!(app.active_command.is_none());
    enter(&mut app, "Point");
    enter(&mut app, "Point");
    assert!(app.snaps.model_override.is_some());
    assert!(app.accept_filtered_drafting_point(point(0., 0., 5.), false));
    assert!(app.snaps.model_override.is_none());
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "");
    assert!(app.sweep_session.is_none(), "{:?}", app.command_log);
    let Geometry::Brep(b) = app
        .document
        .objects()
        .find(|o| matches!(o.geometry(), Geometry::Brep(_)))
        .unwrap()
        .geometry()
    else {
        panic!()
    };
    assert!(b.is_solid());
    assert_eq!((b.vertices().len(), b.edges().len()), (2, 1));
    for expected in [point(0., 0., 0.), point(0., 0., 5.)] {
        assert!(b.vertices().iter().any(|v| v.point() == expected));
    }
    for source in &before {
        assert_eq!(app.document.object(source.id()), Some(source));
    }
    assert_eq!(app.document.undo_label(), Some("Sweep1"));
    let after = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), after);
}
#[test]
fn sweep_local_undo_and_cancel_preserve_model_redo_and_preselection() {
    let (mut app, ids) = setup();
    app.commands
        .execute(&mut app.document, "Point 1,2,3")
        .unwrap();
    app.document.undo().unwrap();
    app.document
        .select_objects_direct([ids[0]], SelectionMode::Replace)
        .unwrap();
    let before = format!("{:?}", app.document);
    let redo = app.document.redo_label().map(str::to_owned);
    enter(&mut app, "Sweep1");
    enter(&mut app, "Point");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "Undo");
    assert!(
        app.sweep_session
            .as_ref()
            .unwrap()
            .state
            .options
            .ends
            .is_empty()
    );
    pick(&mut app, ids[1]);
    enter(&mut app, "Undo");
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[0]]
    );
    enter(&mut app, "StartPoint");
    app.cancel_current_prompt_or_selection();
    assert!(app.command_line_idle());
    assert_eq!(format!("{:?}", app.document), before);
    assert_eq!(app.document.redo_label(), redo.as_deref());
}
#[test]
fn sweep_failed_build_retains_editable_sources_and_options() {
    let (mut app, ids) = setup();
    enter(&mut app, &format!("Sweep1 RailId={} Parameters=0", ids[0]));
    pick(&mut app, ids[1]);
    enter(&mut app, "StartPoint");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "");
    assert!(app.sweep_session.is_some());
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "Parameters");
    enter(&mut app, "2.5");
    enter(&mut app, "EndPoint");
    enter(&mut app, "w0,0,5");
    enter(&mut app, "");
    assert!(app.sweep_session.is_none(), "{:?}", app.command_log);
    assert_eq!(app.document.undo_label(), Some("Sweep1"));
}
#[test]
fn sweep_point_getter_survives_transparent_cplane_changes_and_rejects_stale_named_rail() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct([ids[0]], SelectionMode::Replace)
        .unwrap();
    app.commands
        .execute(&mut app.document, "SetObjectName Rail")
        .unwrap();
    app.document.clear_selection();
    app.document.clear_history().unwrap();
    enter(&mut app, "Sweep1 RailName=Rail Parameters=2.5 Point");
    assert_eq!(app.active_command, Some(InteractiveCommand::SweepPoint));
    enter(&mut app, "CPlane 3Point");
    for p in ["w10,20,30", "w11,20,30", "w10,21,30"] {
        enter(&mut app, p);
    }
    assert_eq!(app.active_command, Some(InteractiveCommand::SweepPoint));
    enter(&mut app, "-10,-20,-30");
    pick(&mut app, ids[1]);
    enter(&mut app, "EndPoint");
    enter(&mut app, "w0,0,5");
    app.document.delete_object(ids[0]).unwrap();
    let label = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "");
    assert!(app.sweep_session.is_none());
    assert_eq!(app.document.undo_label(), label.as_deref());
    assert_eq!(app.document.objects().count(), 2);
}

#[test]
fn sweep_window_picks_keep_input_order_and_ignore_group_expansion() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    app.commands
        .execute(&mut app.document, "Group Keep")
        .unwrap();
    app.document.clear_selection();
    enter(&mut app, "Sweep1");
    app.apply_selection_window(SelectionWindow {
        object_ids: vec![ids[0], ids[1], ids[2]],
        mode: SelectionMode::Replace,
        crossing: true,
        inverted: false,
    });
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[0], ids[1]]
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.selected_object_count(), 0);
    pick(&mut app, ids[0]);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[0]]
    );
    app.select_sweep_objects([ids[0], ids[0]], SelectionMode::Toggle);
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Undo");
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[0]]
    );
    enter(&mut app, "Cancel");
    assert!(app.command_line_idle());
}
#[test]
fn sweep_rejects_a_pole_beyond_the_shared_section_budget_without_losing_picks() {
    let (mut app, ids) = setup();
    let curve = app.document.object(ids[1]).unwrap().geometry().clone();
    let profiles = (0..viboceros_geometry::Sweep1::MAX_SECTIONS)
        .map(|_| app.document.add_geometry(curve.clone()).unwrap())
        .collect::<Vec<_>>();
    enter(&mut app, &format!("Sweep1 RailId={}", ids[0]));
    app.select_prompt_objects(profiles.clone(), SelectionMode::Add);
    enter(&mut app, "EndPoint");
    assert_eq!(app.active_command, None);
    assert_eq!(app.document.selected_object_count(), profiles.len());
    app.select_prompt_objects([profiles[0]], SelectionMode::Remove);
    enter(&mut app, "EndPoint");
    assert_eq!(app.active_command, Some(InteractiveCommand::SweepPoint));
    enter(&mut app, "Undo");
    enter(&mut app, "Cancel");
    assert!(app.command_line_idle());
}
#[test]
fn sweep_invalid_point_input_and_options_leave_the_picker_editable() {
    let (mut app, ids) = setup();
    enter(&mut app, "Sweep1");
    enter(&mut app, "Point");
    assert!(app.active_command.is_none());
    pick(&mut app, ids[0]);
    enter(&mut app, "Point");
    enter(&mut app, "wNaN,0,0");
    assert_eq!(app.active_command, Some(InteractiveCommand::SweepPoint));
    enter(&mut app, "Undo");
    enter(&mut app, "RefitRail");
    enter(&mut app, "Maybe");
    assert_eq!(app.sweep_session.as_ref().unwrap().value, Some("RefitRail"));
    enter(&mut app, "Yes");
    assert_eq!(
        app.sweep_session.as_ref().unwrap().state.options.refit,
        Some(true)
    );
    enter(&mut app, "Undo");
    assert_eq!(
        app.sweep_session.as_ref().unwrap().state.options.refit,
        None
    );
    enter(&mut app, "Cancel");
    assert!(app.command_line_idle());
}

#[test]
fn sweep_unrelated_options_do_not_rebind_a_changed_named_rail() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct([ids[0]], SelectionMode::Replace)
        .unwrap();
    app.commands
        .execute(&mut app.document, "SetObjectName Rail")
        .unwrap();
    app.document.clear_selection();
    enter(&mut app, "Sweep1 RailName=Rail Parameters=2.5");
    pick(&mut app, ids[1]);
    app.document.delete_object(ids[0]).unwrap();
    let label = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "RefitRail=Yes");
    enter(&mut app, "");
    assert!(app.sweep_session.is_none(), "{:?}", app.command_log);
    assert_eq!(app.document.undo_label(), label.as_deref());
}

#[test]
fn sweep_named_rail_uses_command_case_aliases_and_rejects_case_ambiguous_names() {
    let (mut app, ids) = setup();
    app.document
        .set_object_names([(ids[0], Some("Rail".into()))])
        .unwrap();
    enter(&mut app, "Sweep1 RailName=rAiL Parameters=2.5 Point");
    assert_eq!(app.active_command, Some(InteractiveCommand::SweepPoint));
    enter(&mut app, "w0,0,0");
    pick(&mut app, ids[1]);
    enter(&mut app, "Point");
    enter(&mut app, "w0,0,5");
    enter(&mut app, "");
    assert!(app.sweep_session.is_none(), "{:?}", app.command_log);
    assert!(
        app.document
            .objects()
            .any(|o| matches!(o.geometry(), Geometry::Brep(_)))
    );
    let (mut app, ids) = setup();
    app.document
        .set_object_names([(ids[0], Some("Rail".into())), (ids[2], Some("RAIL".into()))])
        .unwrap();
    enter(&mut app, "Sweep1 RailName=rail Point");
    assert_eq!(app.active_command, None);
    assert!(app.sweep_session.is_some());
    assert_eq!(app.document.objects().count(), 3);
}

#[test]
fn oversized_sweep_selection_stops_a_long_stream_and_leaves_picks_atomic() {
    let (mut app, ids) = setup();
    let curve = app.document.object(ids[1]).unwrap().geometry().clone();
    let profiles = (0..viboceros_geometry::Sweep1::MAX_SECTIONS + 2)
        .map(|_| app.document.add_geometry(curve.clone()).unwrap())
        .collect::<Vec<_>>();
    let before = format!("{:?}", app.document);
    enter(&mut app, &format!("Sweep1 RailId={}", ids[0]));
    app.select_sweep_objects(
        profiles
            .into_iter()
            .chain(std::iter::repeat_with(|| -> ObjectId {
                panic!("oversized batch must stop before consuming the rest of a long stream")
            })),
        SelectionMode::Add,
    );
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.sweep_session.as_ref().unwrap().state.sources.is_empty());
    assert!(app.sweep_session.as_ref().unwrap().undo.is_empty());
}
