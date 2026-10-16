use super::*;
use viboceros_document::SelectionMode;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn setup() -> (VibocerosApp, Vec<viboceros_document::ObjectId>) {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 10,0,0");
    enter(&mut app, "Line 0,2,0 10,2,0");
    let ids = app.document.objects().map(|o| o.id()).collect();
    (app, ids)
}
#[test]
fn crv_deviation_command_first_picker_requires_two_curves_and_enter_finishes() {
    let (mut app, ids) = setup();
    enter(&mut app, "CrvDeviation");
    assert!(app.object_prompt.is_some());
    app.document
        .select_objects_direct([ids[0]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "");
    assert!(app.object_prompt.is_some());
    app.document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert!(app.commands.curve_deviation_view(&app.document).is_some());
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "KeepMarks=Yes");
    assert!(
        app.commands
            .curve_deviation_view(&app.document)
            .unwrap()
            .keep_marks
    );
    enter(&mut app, "");
    assert!(app.commands.curve_deviation_view(&app.document).is_none());
    assert_eq!(app.document.objects().len(), 14);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 2);
}
#[test]
fn crv_deviation_preselection_escape_and_replacement_dismiss_without_edits() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct(ids.clone(), SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "CrvDeviation KeepMarks=Yes");
    assert!(app.commands.curve_deviation_view(&app.document).is_some());
    app.cancel_current_prompt_or_selection();
    assert!(app.commands.curve_deviation_view(&app.document).is_none());
    assert_eq!(app.document.objects().len(), 2);
    app.document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "CrvDeviation");
    enter(&mut app, "Distance");
    assert!(app.commands.curve_deviation_view(&app.document).is_none());
    assert_eq!(app.document.objects().len(), 2);
}
#[test]
fn crv_deviation_subcurve_promotes_picker_and_keeps_source_objects() {
    let (mut app, ids) = setup();
    let source = app.document.object(ids[0]).unwrap().geometry().clone();
    let undo = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "CrvDeviation");
    enter(&mut app, "SubCrv");
    assert!(app.object_prompt.is_none());
    assert_eq!(
        app.intersection_prompt.as_ref().unwrap().name(),
        "CrvDeviation"
    );
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    assert!(app.accept_drafting_point(point(3., 0., 0.)));
    enter(&mut app, "7,0,0");
    assert!(app.active_command.is_none());
    assert_eq!(
        app.intersection_prompt
            .as_ref()
            .unwrap()
            .uv_subcurves
            .ranges
            .len(),
        1
    );
    assert_eq!(app.document.objects().len(), 2);
    assert_eq!(app.document.undo_label(), undo.as_deref());
    enter(&mut app, &ids[1].to_string());
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_none());
    let v = app.commands.curve_deviation_view(&app.document).unwrap();
    assert_eq!(v.ends[0].parameters, [3., 3.]);
    assert_eq!(v.ends[1].parameters, [7., 7.]);
    assert_eq!(app.document.object(ids[0]).unwrap().geometry(), &source);
    assert_eq!(app.document.undo_label(), undo.as_deref());
}
#[test]
fn crv_deviation_inline_subcurve_reverse_and_preselection_do_not_duplicate_inputs() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct(ids.clone(), SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "CrvDeviation SubCrv KeepMarks=Yes");
    enter(&mut app, &ids[0].to_string());
    enter(&mut app, "7,0,0");
    enter(&mut app, "3,0,0");
    enter(&mut app, "");
    let v = app.commands.curve_deviation_view(&app.document).unwrap();
    assert_eq!(v.ends[0].parameters, [-7., 7.]);
    assert_eq!(v.ends[1].parameters, [-3., 3.]);
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 14);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 2);
}
#[test]
fn crv_deviation_subcurve_empty_cancel_and_clear_preserve_model() {
    let (mut app, ids) = setup();
    let geometry = app
        .document
        .objects()
        .map(|o| o.geometry().clone())
        .collect::<Vec<_>>();
    app.document
        .select_objects_direct(ids.clone(), SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "CrvDeviation SubCrv");
    enter(&mut app, &ids[0].to_string());
    enter(&mut app, "3,0,0");
    app.cancel_current_prompt_or_selection();
    assert!(app.intersection_prompt.is_none());
    assert!(app.active_command.is_none());
    assert_eq!(app.document.selected_object_count(), 2);
    enter(&mut app, "CrvDeviation SubCrv");
    enter(&mut app, "");
    assert!(
        app.intersection_prompt
            .as_ref()
            .unwrap()
            .uv_subcurves
            .pending
            .is_none()
    );
    enter(&mut app, "SelNone");
    enter(&mut app, "");
    assert!(app.commands.curve_deviation_view(&app.document).is_none());
    assert!(app.intersection_prompt.is_some());
    assert_eq!(
        app.document
            .objects()
            .map(|o| o.geometry().clone())
            .collect::<Vec<_>>(),
        geometry
    );
}
#[test]
fn crv_deviation_subcurve_numeric_length_uses_shared_confirmation() {
    let (mut app, ids) = setup();
    enter(&mut app, "CrvDeviation SubCrv");
    enter(&mut app, &ids[0].to_string());
    enter(&mut app, "3,0,0");
    enter(&mut app, "2");
    assert_eq!(
        app.intersection_prompt
            .as_ref()
            .unwrap()
            .uv_subcurves
            .pending
            .as_ref()
            .unwrap()
            .length,
        Some(2.)
    );
    assert!(app.accept_drafting_point(point(7., 0., 0.)));
    enter(&mut app, &ids[1].to_string());
    enter(&mut app, "");
    let v = app.commands.curve_deviation_view(&app.document).unwrap();
    assert_eq!(v.ends[0].points[0], point(3., 0., 0.));
    assert_eq!(v.ends[1].points[0], point(5., 0., 0.));
    assert_eq!(app.document.objects().len(), 2);
}
