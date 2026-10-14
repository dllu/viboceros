use super::*;

#[test]
fn temporary_project_range_uses_filtered_picking_preview_and_one_history_edit() {
    let (mut app, ids) = setup();
    app.document.clear_selection();
    let source = app.document.object(ids[1]).unwrap().clone();
    let history = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "Project");
    enter(&mut app, "SubCrv");
    assert_eq!(
        app.viewport_object_filter(),
        Some(viboceros_command::ObjectSelectionFilter::Curves)
    );
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[2]),
        mode: SelectionMode::Replace,
    });
    assert!(app.active_command.is_none());
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[1]),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "w3,5,3");
    app.intersection_prompt
        .as_mut()
        .unwrap()
        .uv_subcurves
        .pending
        .as_mut()
        .unwrap()
        .hover_parameter = Some(6.);
    let preview = app.subcurve_draft_preview().unwrap();
    assert!(preview.show_curve);
    assert_eq!(
        preview.geometry.endpoints,
        [point(3., 5., 3.), point(7., 5., 3.)]
    );
    assert_eq!(app.document.object(ids[1]), Some(&source));
    assert_eq!(app.document.undo_label(), history.as_deref());
    enter(&mut app, "w7,5,3");
    assert!(app.active_command.is_none());
    enter(&mut app, "DeleteInput=Yes");
    enter(&mut app, "");
    assert_eq!(
        app.intersection_prompt.as_ref().unwrap().first,
        Some(vec![])
    );
    assert_eq!(
        app.viewport_object_filter(),
        Some(viboceros_command::ObjectSelectionFilter::ProjectTargets)
    );
    enter(&mut app, &ids[0].to_string());
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().len(), 4);
    assert_eq!(app.document.object(ids[1]), Some(&source));
    let c = app
        .document
        .selected_objects()
        .next()
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap();
    assert_eq!(c.start_point().unwrap(), point(3., 5., 0.));
    assert_eq!(c.end_point().unwrap(), point(7., 5., 0.));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 3);
    enter(&mut app, "Redo");
    assert_eq!(app.document.selected_object_count(), 1);
}

#[test]
fn project_ranges_and_whole_points_survive_custom_direction_input() {
    let (mut app, ids) = setup();
    app.document.clear_selection();
    enter(&mut app, "Project");
    for (start, end) in [("w3,5,3", "w5,5,3"), ("w7,5,3", "w5,5,3")] {
        enter(&mut app, "SubCrv");
        enter(&mut app, &ids[1].to_string());
        enter(&mut app, start);
        enter(&mut app, end);
    }
    enter(&mut app, &ids[2].to_string());
    enter(&mut app, "Direction=Custom");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w1,0,1");
    assert_eq!(
        app.intersection_prompt
            .as_ref()
            .unwrap()
            .uv_subcurves
            .ranges
            .len(),
        2
    );
    enter(&mut app, "");
    enter(&mut app, &ids[0].to_string());
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 6);
    assert_eq!(app.document.selected_object_count(), 3);
    let selected = app
        .document
        .selected_objects()
        .map(|o| o.geometry().clone())
        .collect::<Vec<_>>();
    assert_eq!(selected[0], Geometry::Point(point(8., 5., 0.)));
    for (g, ends) in selected.iter().skip(1).zip([
        [point(0., 5., 0.), point(2., 5., 0.)],
        [point(4., 5., 0.), point(2., 5., 0.)],
    ]) {
        let c = g.curve_ref().unwrap();
        assert!(c.start_point().unwrap().distance_to(ends[0]).unwrap() < 1e-8);
        assert!(c.end_point().unwrap().distance_to(ends[1]).unwrap() < 1e-8);
    }
}

#[test]
fn project_nested_numeric_length_uses_the_existing_direction_lock_and_clear_path() {
    let (mut app, ids) = setup();
    app.document.clear_selection();
    enter(&mut app, "Project");
    enter(&mut app, "SubCrv");
    enter(&mut app, &ids[1].to_string());
    enter(&mut app, "w3,5,3");
    app.intersection_prompt
        .as_mut()
        .unwrap()
        .uv_subcurves
        .pending
        .as_mut()
        .unwrap()
        .hover_parameter = Some(6.);
    enter(&mut app, "D");
    enter(&mut app, "2mm");
    assert!(app.active_command.is_none());
    assert_eq!(
        app.intersection_prompt
            .as_ref()
            .unwrap()
            .uv_subcurves
            .ranges[0]
            .parameters,
        [2., 4.]
    );
    assert_eq!(
        app.intersection_prompt
            .as_ref()
            .unwrap()
            .uv_subcurves
            .ranges
            .len(),
        1
    );
    enter(&mut app, "SubCrv");
    enter(&mut app, "");
    assert_eq!(
        app.intersection_prompt
            .as_ref()
            .unwrap()
            .uv_subcurves
            .ranges
            .len(),
        1
    );
    enter(&mut app, "SelNone");
    assert!(
        app.intersection_prompt
            .as_ref()
            .unwrap()
            .uv_subcurves
            .ranges
            .is_empty()
    );
    enter(&mut app, "");
    assert!(app.intersection_prompt.as_ref().unwrap().first.is_none());
    assert_eq!(app.document.objects().len(), 3);
}

#[test]
fn project_nested_range_cancellation_and_invalid_end_preserve_model_and_redo() {
    for stage in 0..=3 {
        let (mut app, ids) = setup();
        enter(&mut app, "Point 12,12,12");
        enter(&mut app, "Undo");
        app.document
            .select_objects_direct([ids[0]], SelectionMode::Replace)
            .unwrap();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        let history = app.document.undo_label().map(str::to_owned);
        enter(&mut app, "Project");
        enter(&mut app, "SubCrv");
        if stage > 0 {
            enter(&mut app, &ids[1].to_string());
        }
        if stage > 1 {
            enter(&mut app, "w3,5,3");
        }
        if stage > 2 {
            enter(&mut app, "w3,5,3");
            assert!(
                app.intersection_prompt
                    .as_ref()
                    .unwrap()
                    .uv_subcurves
                    .pending
                    .is_some()
            );
            enter(&mut app, "w7,5,3");
        }
        app.cancel_current_prompt_or_selection();
        assert!(app.intersection_prompt.is_none());
        assert!(app.active_command.is_none());
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert!(app.document.is_selected(ids[0]));
        assert_eq!(app.document.undo_label(), history.as_deref());
        assert!(app.document.can_redo());
    }
}
