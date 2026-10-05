use super::boolean_intersection::{enter, fixture, pick};
use super::*;
use viboceros_document::SelectionMode;

fn result(app: &VibocerosApp, ids: &[ObjectId]) -> ObjectId {
    let o = app
        .document
        .objects()
        .find(|o| !ids.contains(&o.id()))
        .unwrap();
    let Geometry::Brep(b) = o.geometry() else {
        panic!("difference")
    };
    assert!(
        (b.signed_volume(viboceros_geometry::Tolerance::DEFAULT)
            .unwrap()
            - 7.)
            .abs()
            < 1e-10
    );
    assert_eq!(o.geometry_user_text().get("Code").unwrap(), "geometry-0");
    o.id()
}

#[test]
fn difference_picking_requires_cutters_filters_points_and_records_one_undo() {
    let (mut app, ids) = fixture();
    enter(&mut app, "_-BooleanDifference");
    pick(&mut app, ids[2]);
    assert!(!app.document.is_selected(ids[2]));
    pick(&mut app, ids[0]);
    enter(&mut app, "");
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_some());
    assert_eq!(app.document.objects().len(), 3);
    pick(&mut app, ids[1]);
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().len(), 2);
    assert!(!app.document.is_selected(result(&app, &ids)));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn difference_preselection_selects_results_and_preserves_source_selection_on_undo() {
    let (mut app, ids) = fixture();
    app.document
        .select_objects_direct([ids[0]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "BooleanDifference");
    assert_eq!(
        app.intersection_prompt.as_ref().unwrap().first.as_deref(),
        Some(&ids[..1])
    );
    pick(&mut app, ids[1]);
    enter(&mut app, "");
    let new = result(&app, &ids);
    assert!(app.document.is_selected(new));
    enter(&mut app, "Undo");
    assert!(app.document.is_selected(ids[0]));
    assert!(!app.document.is_selected(ids[1]));
    enter(&mut app, "Redo");
    assert!(app.document.is_selected(new));
}

#[test]
fn difference_accepted_options_survive_both_cancellations_and_hidden_cutters_keep_memory() {
    let (mut app, ids) = fixture();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "BooleanDifference");
    enter(&mut app, "DeleteInput=No");
    pick(&mut app, ids[0]);
    enter(&mut app, "Cancel");
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "BooleanDifference");
    assert!(
        !app.intersection_prompt
            .as_ref()
            .unwrap()
            .boolean
            .as_ref()
            .unwrap()
            .delete_input
    );
    enter(&mut app, "DeleteCutters=No");
    assert!(
        app.intersection_prompt
            .as_ref()
            .unwrap()
            .boolean
            .as_ref()
            .unwrap()
            .delete_cutters
    );
    enter(&mut app, "DeleteInput=Yes");
    enter(&mut app, "DeleteCutters=No");
    pick(&mut app, ids[0]);
    enter(&mut app, "");
    pick(&mut app, ids[1]);
    enter(&mut app, "Cancel");
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        ids[..1]
    );
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "BooleanDifference");
    assert!(
        !app.intersection_prompt
            .as_ref()
            .unwrap()
            .boolean
            .as_ref()
            .unwrap()
            .delete_cutters
    );
    pick(&mut app, ids[1]);
    enter(&mut app, "");
    assert!(app.document.object(ids[0]).is_none());
    assert!(app.document.is_selected(ids[1]));
    assert!(app.document.is_selected(result(&app, &ids))); // First target was preselected after cancellation.
}

#[test]
fn difference_delete_input_no_retains_sources_and_explicit_sets_execute_without_picking() {
    let (mut app, ids) = fixture();
    enter(
        &mut app,
        &format!(
            "BooleanDifference DeleteInput=No FirstSet={} SecondSet={}",
            ids[0], ids[1]
        ),
    );
    assert!(app.intersection_prompt.is_none() && app.object_prompt.is_none());
    assert_eq!(app.document.objects().len(), 4);
    assert!(app.document.is_selected(ids[0]) && app.document.is_selected(ids[1]));
    assert!(app.document.is_selected(result(&app, &ids)));
}
