use super::*;
use viboceros_document::SelectionMode;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
#[test]
fn point_deviation_two_sets_accepts_points_and_mesh_targets_without_model_edits() {
    let mut app = test_app();
    enter(&mut app, "Point 1,2,3");
    let s = app.document.objects().last().unwrap().id();
    enter(&mut app, "Line 0,0,0 10,0,0");
    let t = app.document.objects().last().unwrap().id();
    let history = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "PointDeviation Ignore=10");
    assert!(app.intersection_prompt.as_ref().unwrap().first.is_none());
    app.apply_selection_click(SelectionClick {
        object_id: Some(s),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    assert_eq!(
        app.intersection_prompt.as_ref().unwrap().first,
        Some(vec![s])
    );
    app.apply_selection_click(SelectionClick {
        object_id: Some(t),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().len(), 2);
    assert_eq!(app.document.undo_label(), history.as_deref());
    let v = app
        .commands
        .point_deviation_view(&app.document)
        .unwrap()
        .unwrap();
    assert_eq!(v.samples.len(), 1);
    assert!((v.samples[0].distance - 13_f64.sqrt()).abs() < 1e-12);
}
#[test]
fn point_deviation_cancel_restores_preselection_and_viewport_window_filters_targets() {
    let mut app = test_app();
    enter(&mut app, "Point 1,2,3");
    let s = app.document.objects().last().unwrap().id();
    enter(&mut app, "Line 0,0,0 10,0,0");
    let t = app.document.objects().last().unwrap().id();
    app.document
        .select_objects_direct([s], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "PointDeviation");
    assert_eq!(
        app.intersection_prompt.as_ref().unwrap().first,
        Some(vec![s])
    );
    app.apply_selection_window(SelectionWindow {
        object_ids: vec![s, t],
        mode: SelectionMode::Replace,
        crossing: false,
        inverted: false,
    });
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![t]
    );
    app.cancel_interactive_command(true);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![s]
    );
    assert!(
        app.commands
            .point_deviation_view(&app.document)
            .unwrap()
            .is_none()
    );
}
