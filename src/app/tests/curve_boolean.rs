use super::boolean_intersection::enter;
use super::*;
fn setup() -> (VibocerosApp, Vec<ObjectId>) {
    let mut app = test_app();
    for input in [
        "Polyline 0,0,0 6,0,0 6,6,0 0,6,0 0,0,0",
        "Polyline 4,2,0 10,2,0 10,8,0 4,8,0 4,2,0",
    ] {
        enter(&mut app, input);
    }
    let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
    app.document.clear_selection();
    app.document.clear_history().unwrap();
    (app, ids)
}
#[test]
fn source_clicks_region_toggles_preview_cancel_and_one_undo_redo() {
    let (mut app, ids) = setup();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "CurveBoolean Output=Surfaces DeleteInput=Used");
    assert!(!app.command_line_idle());
    assert_eq!(
        app.viewport_object_filter(),
        Some(viboceros_command::ObjectSelectionFilter::Curves)
    );
    assert!(!app.viewport_drafting_input(false, false).active);
    for id in &ids {
        app.apply_selection_click(SelectionClick {
            object_id: Some(*id),
            mode: SelectionMode::Replace,
        });
    }
    assert_eq!(app.document.selected_object_count(), 2);
    enter(&mut app, "");
    assert!(app.viewport_drafting_input(false, false).active);
    assert!(app.viewport_object_filter().is_none());
    assert!(app.apply_drafting_point(Point3::try_new(1., 1., 0.).unwrap()));
    let p = app.curve_boolean_prompt.as_ref().unwrap();
    assert!(p.scene().is_some());
    assert_eq!(p.scene().unwrap().objects().len(), 3);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!app.document.can_undo());
    app.cancel_current_prompt_or_selection();
    assert!(app.command_line_idle());
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "CurveBoolean");
    for id in &ids {
        enter(&mut app, &id.to_string());
    }
    enter(&mut app, "");
    enter(&mut app, "AllRegions");
    enter(&mut app, "");
    assert!(app.command_line_idle());
    let after = app.document.objects().cloned().collect::<Vec<_>>();
    assert_eq!(after.len(), 4); // union and two source remnants, one split at its seam
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!app.document.can_undo());
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), after);
}
#[test]
fn preselected_options_toggles_and_new_command_preserve_model_and_preferences() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "CurveBoolean");
    assert!(app.viewport_drafting_input(false, false).active);
    enter(&mut app, "CombineRegions No");
    enter(&mut app, "Output Surfaces");
    enter(&mut app, "1,1,0");
    enter(&mut app, "1,1,0");
    enter(&mut app, "");
    assert!(!app.document.can_undo());
    assert!(app.command_line_idle());
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "CurveBoolean");
    enter(&mut app, "Point 20,20,0");
    assert!(app.command_line_idle());
    assert_eq!(app.document.objects().len(), 3);
    let options = app.commands.curve_boolean_options();
    assert!(!options.combine);
    assert_eq!(
        options.output,
        viboceros_command::curve_boolean::Output::Surfaces
    );
}

#[test]
fn nested_subcurve_direction_length_preview_and_stale_range_are_isolated() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 10,0,0");
    let id = app.document.objects().next().unwrap().id();
    app.document.clear_selection();
    app.document.clear_history().unwrap();
    let before = app.document.object(id).unwrap().clone();
    enter(&mut app, "CurveBoolean");
    enter(&mut app, "SubCrv");
    enter(&mut app, &id.to_string());
    assert!(app.viewport_drafting_input(false, false).active);
    app.apply_drafting_point(Point3::try_new(5., 0., 0.).unwrap());
    app.update_subcurve_hover(Point3::try_new(8., 0., 0.).unwrap());
    assert!(app.curve_boolean_subcurve_preview().is_some());
    enter(&mut app, "D");
    enter(&mut app, "D"); // the shortcut toggles back to free
    let pending = app
        .curve_boolean_prompt
        .as_ref()
        .unwrap()
        .subcurves
        .pending
        .as_ref()
        .unwrap();
    assert_eq!(pending.locked_forward, None);
    enter(&mut app, "Direction=Locked");
    enter(&mut app, "-8"); // magnitude, clamped to the open endpoint
    let prompt = app.curve_boolean_prompt.as_ref().unwrap();
    assert!(prompt.subcurves.pending.is_none());
    assert_eq!(prompt.subcurves.ranges.len(), 1);
    let curve = before.geometry().curve_ref().unwrap();
    let range = prompt.subcurves.ranges[0].parameters;
    assert!((curve.evaluate(range[0]).unwrap().x() - 5.).abs() < 1e-8);
    assert!((curve.evaluate(range[1]).unwrap().x() - 10.).abs() < 1e-8);
    assert_eq!(app.document.object(id), Some(&before));
    assert!(!app.document.can_undo());
    app.document
        .transform_objects(
            [id],
            viboceros_geometry::AffineTransform3::from_translation(
                viboceros_geometry::Vector3::try_new(0., 1., 0.).unwrap(),
            ),
        )
        .unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, ""); // a stale stored range cannot prepare a graph
    assert!(app.curve_boolean_prompt.is_none());
    assert!(!app.document.can_undo());
    assert_eq!(app.document.objects().len(), 1);
}
