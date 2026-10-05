use super::*;
use viboceros_document::SelectionMode;
use viboceros_geometry::{Brep, Frame3, Tolerance, Vector3};

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn fixture(disjoint: bool) -> (VibocerosApp, [ObjectId; 3]) {
    let mut app = test_app();
    let frame = Frame3::try_from_directions(
        point(0., 0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let ids = [0., if disjoint { 4. } else { 1. }].map(|x| {
        app.document
            .add_geometry(Geometry::Brep(
                Brep::try_box(frame, [[x, x + 2.]; 3], Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap()
    });
    let peer = app
        .document
        .add_geometry(Geometry::Point(point(20., 0., 0.)))
        .unwrap();
    (app, [ids[0], ids[1], peer])
}

#[test]
fn boolean_union_picking_filters_points_requires_two_and_applies_native_defaults() {
    let (mut app, ids) = fixture(false);
    enter(&mut app, "_-BooleanUnion");
    assert!(app.object_prompt.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[2]),
        mode: SelectionMode::Add,
    });
    assert!(!app.document.is_selected(ids[2]));
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Add,
    });
    enter(&mut app, "");
    assert!(app.object_prompt.is_some());
    assert_eq!(app.document.objects().len(), 3);
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[1]),
        mode: SelectionMode::Add,
    });
    enter(&mut app, "");
    assert!(app.object_prompt.is_none(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().len(), 2);
    assert_eq!(app.document.selected_object_count(), 0);
    let result = app.document.objects().find(|o| o.id() != ids[2]).unwrap();
    let Geometry::Brep(b) = result.geometry() else {
        panic!("union")
    };
    assert!((b.signed_volume(Tolerance::DEFAULT).unwrap() - 15.).abs() < 1e-10);
    enter(&mut app, "Undo");
    assert!(ids.iter().all(|&id| app.document.object(id).is_some()));
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn boolean_union_preselection_executes_immediately_and_retains_copied_sources() {
    let (mut app, ids) = fixture(false);
    app.document
        .select_objects_direct(ids[..2].iter().copied(), SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "BooleanUnion DeleteInput=No");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().len(), 4);
    assert_eq!(app.document.selected_object_count(), 3);
    assert!(!app.document.is_selected(ids[2]));
}

#[test]
fn boolean_union_cancellation_keeps_accepted_options_and_failure_ends_the_prompt() {
    let (mut app, ids) = fixture(false);
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let undo = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "BooleanUnion");
    enter(&mut app, "DeleteInput=No MergeCoplanarFaces=No");
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Add,
    });
    app.cancel_interactive_command(true);
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(app.document.undo_label(), undo.as_deref());
    enter(&mut app, "BooleanUnion");
    assert!(
        app.object_prompt
            .as_ref()
            .unwrap()
            .description
            .options
            .iter()
            .all(|o| !o.value)
    );
    for id in &ids[..2] {
        app.apply_selection_click(SelectionClick {
            object_id: Some(*id),
            mode: SelectionMode::Add,
        });
    }
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 4);
    assert_eq!(app.document.selected_object_count(), 2);
    assert!(app.document.is_selected(ids[0]) && app.document.is_selected(ids[1]));
    let (mut app, ids) = fixture(true);
    enter(&mut app, "BooleanUnion");
    for id in &ids[..2] {
        app.apply_selection_click(SelectionClick {
            object_id: Some(*id),
            mode: SelectionMode::Add,
        });
    }
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.selected_object_count(), 2);
}
