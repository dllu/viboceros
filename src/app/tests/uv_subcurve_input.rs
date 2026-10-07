use super::*;
use viboceros_document::SelectionMode;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

fn setup() -> (VibocerosApp, Vec<viboceros_document::ObjectId>) {
    let mut app = test_app();
    enter(&mut app, "SrfPt 0,0,0 4,0,0 4,6,0 0,6,0");
    enter(&mut app, "Line 0,0 4,6");
    enter(&mut app, "Rectangle 0,0 4,6");
    let ids = app.document.objects().map(|o| o.id()).collect();
    app.document.clear_history().unwrap();
    app.document.clear_selection();
    (app, ids)
}

#[test]
fn apply_inline_subcurve_accepts_clicks_and_typed_endpoints_without_source_edits() {
    let (mut app, ids) = setup();
    let original = app.document.object(ids[1]).unwrap().geometry().clone();
    enter(&mut app, "ApplyCrv");
    enter(&mut app, "SubCrv");
    assert_eq!(
        app.intersection_prompt.as_ref().unwrap().filter(),
        viboceros_command::ObjectSelectionFilter::Curves
    );
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[1]),
        mode: SelectionMode::Replace,
    });
    assert!(app.accept_drafting_point(point(1., 1.5, 0.)));
    enter(&mut app, "3,4.5,0");
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
    assert_eq!(app.document.objects().len(), 3);
    assert!(app.document.undo_label().is_none());
    enter(&mut app, &ids[2].to_string());
    enter(&mut app, "");
    enter(&mut app, &ids[0].to_string());
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().len(), 5);
    assert_eq!(app.document.object(ids[1]).unwrap().geometry(), &original);
    let output = app
        .document
        .selected_objects()
        .find(|o| o.attributes().name().is_none())
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap();
    assert!(
        output
            .start_point()
            .unwrap()
            .distance_to(point(1., 1.5, 0.))
            .unwrap()
            < 1e-8
    );
    assert!(
        output
            .end_point()
            .unwrap()
            .distance_to(point(3., 4.5, 0.))
            .unwrap()
            < 1e-8
    );
    assert_eq!(app.document.undo_label(), Some("ApplyCrv"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Redo");
    assert_eq!(app.document.selected_object_count(), 2);
}

#[test]
fn create_inline_subcurves_repeat_reverse_and_reject_coincident_endpoints() {
    let (mut app, ids) = setup();
    enter(&mut app, "CreateUVCrv");
    enter(&mut app, &ids[0].to_string());
    for (start, end) in [("3,4.5", "1,1.5"), ("1.6,2.4", "2.4,3.6")] {
        enter(&mut app, "SubCrv");
        enter(&mut app, &ids[1].to_string());
        enter(&mut app, start);
        enter(&mut app, start);
        assert!(
            app.intersection_prompt
                .as_ref()
                .unwrap()
                .uv_subcurves
                .pending
                .is_some()
        );
        enter(&mut app, end);
    }
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().len(), 6);
    assert_eq!(app.document.selected_object_count(), 3);
    assert!(!app.document.is_selected(ids[1]));
    let outputs = app
        .document
        .objects()
        .filter(|o| !ids.contains(&o.id()) && o.attributes().name().is_none())
        .collect::<Vec<_>>();
    assert!(
        outputs[1]
            .geometry()
            .curve_ref()
            .unwrap()
            .start_point()
            .unwrap()
            .distance_to(point(3., 4.5, 0.))
            .unwrap()
            < 1e-8
    );
}

#[test]
fn cancelling_nested_uv_subcurve_restores_initial_selection_and_history() {
    for command in ["ApplyCrv", "CreateUVCrv"] {
        for stage in 0..3 {
            let (mut app, ids) = setup();
            app.document
                .select_objects_direct([ids[0]], SelectionMode::Replace)
                .unwrap();
            let before = app
                .document
                .objects()
                .map(|o| (o.id(), o.geometry().clone(), o.attributes().clone()))
                .collect::<Vec<_>>();
            let selection = app.document.selected_object_ids().collect::<Vec<_>>();
            enter(&mut app, command);
            if command == "ApplyCrv" {
                assert!(app.intersection_prompt.as_ref().unwrap().first.is_none());
            }
            enter(&mut app, "SubCrv");
            if stage > 0 {
                enter(&mut app, &ids[1].to_string());
            }
            if stage > 1 {
                enter(&mut app, "1,1.5");
            }
            app.cancel_current_prompt_or_selection();
            assert!(app.active_command.is_none());
            assert!(app.intersection_prompt.is_none());
            assert_eq!(
                app.document
                    .objects()
                    .map(|o| (o.id(), o.geometry().clone(), o.attributes().clone()))
                    .collect::<Vec<_>>(),
                before
            );
            assert_eq!(
                app.document.selected_object_ids().collect::<Vec<_>>(),
                selection
            );
            assert!(!app.document.can_undo());
            assert!(!app.document.can_redo());
        }
    }
}

#[test]
fn selnone_discards_temporary_ranges_and_another_command_cancels_nested_input() {
    let (mut app, ids) = setup();
    enter(&mut app, "CreateUVCrv");
    enter(&mut app, &ids[0].to_string());
    enter(&mut app, "SubCrv");
    enter(&mut app, &ids[1].to_string());
    enter(&mut app, "1,1.5");
    enter(&mut app, "3,4.5");
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
    assert_eq!(app.document.objects().len(), 4);
    enter(&mut app, "Undo");
    enter(&mut app, "ApplyCrv");
    enter(&mut app, "SubCrv");
    enter(&mut app, &ids[1].to_string());
    enter(&mut app, "1,1.5");
    enter(&mut app, "Line 10,0 11,1");
    assert!(app.intersection_prompt.is_none());
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().len(), 4);
    assert_eq!(app.document.undo_label(), Some("Line"));
}
