use super::*;

#[test]
fn zoom_ends_preserves_modeling_input_selection_and_document_history() {
    let mut app = test_app();
    for command in [
        "Polyline 0,0,0 1000,500,0 2,0,0",
        "SelAll",
        "Point 5,5,5",
        "Undo",
        "Line",
        "0",
    ] {
        enter(&mut app, command);
    }
    let context = egui::Context::default();
    layout_viewports(&context, &mut app);
    let before = app
        .viewports
        .iter()
        .map(Viewport::camera_snapshot)
        .collect::<Vec<_>>();
    let pending = app.active_command;
    let selected = app.document.selected_object_ids().collect::<Vec<_>>();
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    let redo = app.document.redo_label().map(str::to_owned);
    enter(&mut app, "ZoomEnds All");
    assert_ne!(app.viewports[0].camera_snapshot(), before[0]);
    for (index, snapshot) in before.iter().enumerate().skip(1) {
        assert_eq!(app.viewports[index].camera_snapshot(), *snapshot);
    }
    assert_eq!(app.active_command, pending);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        selected
    );
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(app.document.redo_label(), redo.as_deref());
    enter(&mut app, "UndoView");
    assert_eq!(app.viewports[0].camera_snapshot(), before[0]);
    app.document.clear_selection();
    enter(&mut app, "ZoomEnds");
    assert_eq!(app.viewports[0].camera_snapshot(), before[0]);
    assert_eq!(
        app.command_log.back().map(String::as_str),
        Some("No visible curve end markers to zoom to")
    );
}

#[test]
fn show_ends_filters_live_markers_and_zoom_uses_session_sources() {
    let mut app = test_app();
    for command in ["Polyline 0,0,0 10,10,0 2,0,0", "SelAll", "Line", "0"] {
        enter(&mut app, command);
    }
    let context = egui::Context::default();
    layout_viewports(&context, &mut app);
    let pending = app.active_command;
    let selected = app.document.selected_object_ids().collect::<Vec<_>>();
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    let undo = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "ShowEnds");
    let analysis = app.end_analysis.as_ref().unwrap();
    assert_eq!(analysis.sources, selected);
    let markers = collect_end_markers(
        &app.document,
        analysis.sources.iter().copied(),
        analysis.options,
    )
    .unwrap();
    assert_eq!(markers.len(), 2);
    assert_eq!(app.active_command, pending);
    app.document.clear_selection();
    app.end_analysis.as_mut().unwrap().options.ends = false;
    let single_marker = collect_end_markers(
        &app.document,
        selected.iter().copied(),
        app.end_analysis.as_ref().unwrap().options,
    )
    .unwrap();
    assert_eq!(single_marker.len(), 1);
    enter(&mut app, "ZoomEnds");
    assert_eq!(
        app.viewports[0].camera_snapshot().target,
        nalgebra::Vector3::new(0., 0., 0.)
    );
    app.end_analysis.as_mut().unwrap().options.ends = true;
    enter(&mut app, "ZoomEnds");
    assert_eq!(
        app.viewports[0].camera_snapshot().target,
        nalgebra::Vector3::new(1., 0., 0.)
    );
    assert_eq!(app.active_command, pending);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(app.document.undo_label(), undo.as_deref());
    enter(&mut app, "ShowEndsOff");
    assert!(app.end_analysis.is_none());
    let camera = app.viewports[0].camera_snapshot();
    enter(&mut app, "ZoomEnds");
    assert_eq!(app.viewports[0].camera_snapshot(), camera);
}

#[test]
fn zoom_ends_cycles_session_markers_and_preserves_position_on_failure() {
    let mut app = test_app();
    enter(&mut app, "Polyline 0,0,0 10,10,0 2,0,0");
    enter(&mut app, "SelAll");
    let context = egui::Context::default();
    layout_viewports(&context, &mut app);
    enter(&mut app, "ZoomEnds Next");
    assert_eq!(
        app.command_log.back().map(String::as_str),
        Some("Run ShowEnds with visible selected curves first")
    );
    enter(&mut app, "ShowEnds");
    enter(&mut app, "ZoomEnds Current");
    assert_eq!(app.end_analysis.as_ref().unwrap().current, 0);
    assert_eq!(
        app.viewports[0].camera_snapshot().target,
        nalgebra::Vector3::new(0., 0., 0.)
    );
    enter(&mut app, "ZoomEnds Next");
    assert_eq!(app.end_analysis.as_ref().unwrap().current, 1);
    assert_eq!(
        app.viewports[0].camera_snapshot().target,
        nalgebra::Vector3::new(2., 0., 0.)
    );
    enter(&mut app, "ZoomEnds Next");
    assert_eq!(app.end_analysis.as_ref().unwrap().current, 0);
    enter(&mut app, "ZoomEnds Previous");
    assert_eq!(app.end_analysis.as_ref().unwrap().current, 1);
    app.end_analysis.as_mut().unwrap().options = EndMarkerOptions {
        starts: false,
        ends: false,
        seams: false,
        joints: false,
    };
    let camera = app.viewports[0].camera_snapshot();
    enter(&mut app, "ZoomEnds Next");
    assert_eq!(app.end_analysis.as_ref().unwrap().current, 1);
    assert_eq!(app.viewports[0].camera_snapshot(), camera);
}

#[test]
fn zoom_ends_mark_creates_one_undo_step_for_current_or_all_markers() {
    let mut app = test_app();
    enter(&mut app, "Polyline 0,0,0 10,10,0 2,0,0");
    enter(&mut app, "SelAll");
    let context = egui::Context::default();
    layout_viewports(&context, &mut app);
    let selected = app.document.selected_object_ids().collect::<Vec<_>>();
    let original_count = app.document.objects().count();
    enter(&mut app, "ShowEnds");
    enter(&mut app, "ZoomEnds Mark");
    assert_eq!(app.document.objects().count(), original_count + 1);
    assert_eq!(app.document.undo_label(), Some("Mark curve ends"));
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        selected
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), original_count);
    enter(&mut app, "ZoomEnds All");
    enter(&mut app, "ZoomEnds Mark");
    assert_eq!(app.document.objects().count(), original_count + 2);
    assert_eq!(app.document.undo_label(), Some("Mark curve ends"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), original_count);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        selected
    );
}

#[test]
fn end_analysis_adds_and_removes_selected_curves_without_model_edits() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 1,0,0");
    enter(&mut app, "Line 10,0,0 11,0,0");
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    app.document
        .select_object(ids[0], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ShowEnds");
    assert_eq!(app.end_analysis.as_ref().unwrap().sources, [ids[0]]);
    app.document
        .select_object(ids[1], SelectionMode::Replace)
        .unwrap();
    let before_objects = app.document.objects().cloned().collect::<Vec<_>>();
    let before_history = app.document.undo_label().map(str::to_owned);
    let selected = app.document.selected_object_ids().collect::<Vec<_>>();
    app.add_selected_to_end_analysis();
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        selected
    );
    assert_eq!(app.end_analysis.as_ref().unwrap().sources, ids);
    assert_eq!(
        collect_end_markers(
            &app.document,
            ids.iter().copied(),
            EndMarkerOptions::default()
        )
        .unwrap()
        .len(),
        4
    );
    app.add_selected_to_end_analysis();
    assert_eq!(app.end_analysis.as_ref().unwrap().sources.len(), 2);
    app.document
        .select_object(ids[0], SelectionMode::Replace)
        .unwrap();
    app.remove_selected_from_end_analysis();
    assert_eq!(app.end_analysis.as_ref().unwrap().sources, [ids[1]]);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[0]]
    );
    app.document
        .select_object(ids[1], SelectionMode::Replace)
        .unwrap();
    app.remove_selected_from_end_analysis();
    assert!(app.end_analysis.as_ref().unwrap().sources.is_empty());
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        before_objects
    );
    assert_eq!(app.document.undo_label(), before_history.as_deref());
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[1]]
    );
}

#[test]
fn show_ends_postselection_picks_curves_without_interrupting_modeling() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 1,0,0");
    enter(&mut app, "Line 10,0,0 11,0,0");
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    app.document.clear_selection();
    enter(&mut app, "Line");
    enter(&mut app, "0");
    let pending = app.active_command;
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    let undo = app.document.undo_label().map(str::to_owned);
    app.zoom_factor_pending = Some(0);
    enter(&mut app, "ShowEnds");
    assert_eq!(
        app.end_analysis_pick.as_ref().unwrap().mode,
        EndAnalysisPickMode::Show
    );
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    app.apply_selection_region(
        SelectionWindow {
            object_ids: vec![ids[1]],
            mode: SelectionMode::Replace,
            crossing: false,
            inverted: false,
        },
        false,
    );
    assert_eq!(app.end_analysis.as_ref().unwrap().sources, ids);
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "");
    assert!(app.end_analysis_pick.is_none());
    assert_eq!(app.zoom_factor_pending, Some(0));
    assert_eq!(app.active_command, pending);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(app.document.undo_label(), undo.as_deref());
    app.start_end_analysis_pick(EndAnalysisPickMode::Remove);
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    assert_eq!(app.end_analysis.as_ref().unwrap().sources, [ids[1]]);
    app.cancel_end_analysis_pick(true);
    assert_eq!(app.end_analysis.as_ref().unwrap().sources, ids);
    app.start_end_analysis_pick(EndAnalysisPickMode::Remove);
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    assert_eq!(app.end_analysis.as_ref().unwrap().sources, [ids[1]]);
    assert_eq!(app.document.selected_object_count(), 0);
}
