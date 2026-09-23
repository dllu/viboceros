use super::*;
use viboceros_document::SelectionMode;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.to_owned();
    app.run_command();
}

fn setup() -> (VibocerosApp, Vec<viboceros_document::ObjectId>) {
    let mut app = test_app();
    for input in ["Line 0,0 10,0", "Line 3,-5 3,5", "Line 7,-5 7,5"] {
        enter(&mut app, input);
    }
    let ids = app.document.objects().map(|object| object.id()).collect();
    (app, ids)
}

#[test]
fn viewport_picks_two_sets_and_ignores_within_set_intersections() {
    let (mut app, ids) = setup();
    enter(&mut app, "IntersectTwoSets");
    assert!(app.intersection_prompt.as_ref().unwrap().first.is_none());
    for id in &ids[..2] {
        app.apply_selection_click(SelectionClick {
            object_id: Some(*id),
            mode: SelectionMode::Replace,
        });
    }
    assert_eq!(app.document.selected_object_count(), 2);
    enter(&mut app, "");
    assert_eq!(
        app.intersection_prompt.as_ref().unwrap().first.as_deref(),
        Some(&ids[..2])
    );
    assert_eq!(app.document.selected_object_count(), 0);
    app.apply_selection_window(SelectionWindow {
        object_ids: vec![ids[2]],
        mode: SelectionMode::Replace,
        crossing: false,
        inverted: false,
    });
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().count(), 4);
    let output = app.document.selected_objects().next().unwrap();
    assert!(matches!(output.geometry(), Geometry::Point(location)
        if *location == point(7., 0., 0.)));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), 3);
}

#[test]
fn preselection_starts_second_set_and_cancel_restores_sources() {
    let (mut app, ids) = setup();
    app.document
        .select_object(ids[0], SelectionMode::Replace)
        .unwrap();
    let history = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "IntersectTwoSets OutputLayer=FirstSet");
    assert_eq!(
        app.intersection_prompt.as_ref().unwrap().first.as_deref(),
        Some(&ids[..1])
    );
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "OutputLayer=SecondSet");
    assert_eq!(
        app.intersection_prompt.as_ref().unwrap().output_layer,
        "SecondSet"
    );
    enter(&mut app, "OutputLayer=Bad");
    assert_eq!(
        app.intersection_prompt.as_ref().unwrap().output_layer,
        "SecondSet"
    );
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_some());
    enter(&mut app, &ids[2].to_string());
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        ids[2..]
    );
    app.cancel_interactive_command(true);
    assert!(app.intersection_prompt.is_none());
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        ids[..1]
    );
    assert_eq!(app.document.undo_label(), history.as_deref());
}
