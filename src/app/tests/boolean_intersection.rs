use super::*;
use viboceros_document::SelectionMode;
use viboceros_geometry::{Brep, Frame3, Tolerance, Vector3};

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn fixture() -> (VibocerosApp, [ObjectId; 3]) {
    let mut app = test_app();
    let frame = Frame3::try_from_directions(
        point(0., 0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let boxes = [0., 1.].map(|x| {
        app.document
            .add_geometry(Geometry::Brep(
                Brep::try_box(frame, [[x, x + 2.]; 3], Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap()
    });
    for (i, &id) in boxes.iter().enumerate() {
        app.document
            .set_object_geometry_user_text([id], "Code", Some(&format!("geometry-{i}")))
            .unwrap();
    }
    let peer = app
        .document
        .add_geometry(Geometry::Point(point(20., 0., 0.)))
        .unwrap();
    (app, [boxes[0], boxes[1], peer])
}
fn pick(app: &mut VibocerosApp, id: ObjectId) {
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
}
fn output(app: &VibocerosApp, ids: &[ObjectId]) -> ObjectId {
    let object = app
        .document
        .objects()
        .find(|o| !ids.contains(&o.id()))
        .unwrap();
    let Geometry::Brep(b) = object.geometry() else {
        panic!("intersection")
    };
    assert!((b.signed_volume(Tolerance::DEFAULT).unwrap() - 1.).abs() < 1e-10);
    object.id()
}

#[test]
fn common_picking_filters_points_accepts_two_enters_and_replays_history() {
    let (mut app, ids) = fixture();
    enter(&mut app, "_-BooleanIntersection");
    assert_eq!(
        app.viewport_object_filter(),
        Some(viboceros_command::ObjectSelectionFilter::SurfaceComponents)
    );
    pick(&mut app, ids[2]);
    assert!(!app.document.is_selected(ids[2]));
    enter(&mut app, "");
    assert!(app.intersection_prompt.as_ref().unwrap().first.is_none());
    pick(&mut app, ids[0]);
    pick(&mut app, ids[1]);
    enter(&mut app, "");
    assert_eq!(
        app.intersection_prompt.as_ref().unwrap().first.as_deref(),
        Some(&ids[..2])
    );
    assert_eq!(app.document.selected_object_count(), 2);
    assert_eq!(app.document.objects().len(), 3);
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_none(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().len(), 2);
    let result = output(&app, &ids);
    assert!(!app.document.is_selected(result));
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(
        app.document
            .object(result)
            .unwrap()
            .geometry_user_text()
            .get("Code")
            .unwrap(),
        "geometry-1"
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 2);
    assert_eq!(app.document.selected_object_count(), 0);
}

#[test]
fn two_sets_use_first_owner_and_preselection_selects_only_the_result() {
    for pre in [false, true] {
        let (mut app, ids) = fixture();
        if pre {
            app.document
                .select_objects_direct([ids[0]], SelectionMode::Replace)
                .unwrap();
        }
        enter(&mut app, "BooleanIntersection");
        if !pre {
            pick(&mut app, ids[0]);
            enter(&mut app, "");
        }
        enter(&mut app, "");
        assert!(app.intersection_prompt.is_some()); // Single first object needs a second set.
        pick(&mut app, ids[0]);
        assert_eq!(app.document.selected_object_count(), 1);
        pick(&mut app, ids[1]);
        enter(&mut app, "");
        assert!(app.intersection_prompt.is_none());
        let result = output(&app, &ids);
        assert_eq!(app.document.is_selected(result), pre);
        assert_eq!(
            app.document
                .object(result)
                .unwrap()
                .geometry_user_text()
                .get("Code")
                .unwrap(),
            "geometry-0"
        );
        assert!(!app.document.is_selected(ids[2]));
    }
}

#[test]
fn common_postselection_uses_pick_order_but_preselection_uses_document_order() {
    for pre in [false, true] {
        let (mut app, ids) = fixture();
        if pre {
            for id in [ids[1], ids[0]] {
                app.document
                    .select_objects_direct([id], SelectionMode::Add)
                    .unwrap();
            }
        }
        enter(&mut app, "BooleanIntersection");
        if !pre {
            for id in [ids[1], ids[0]] {
                pick(&mut app, id);
            }
            enter(&mut app, "");
        }
        enter(&mut app, "");
        let result = output(&app, &ids);
        assert!(!app.document.is_selected(result));
        assert_eq!(
            app.document
                .object(result)
                .unwrap()
                .geometry_user_text()
                .get("Code")
                .unwrap(),
            if pre { "geometry-1" } else { "geometry-0" }
        );
    }
}

#[test]
fn cancellations_discard_options_and_second_phase_retains_first_set() {
    let (mut app, ids) = fixture();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let undo = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "BooleanIntersection");
    enter(&mut app, "_DeleteInput _No");
    pick(&mut app, ids[0]);
    enter(&mut app, "Cancel");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(app.document.undo_label(), undo.as_deref());
    enter(&mut app, "BooleanIntersection");
    assert!(
        app.intersection_prompt
            .as_ref()
            .unwrap()
            .boolean
            .as_ref()
            .unwrap()
            .delete_input
    );
    pick(&mut app, ids[0]);
    enter(&mut app, "");
    pick(&mut app, ids[1]);
    enter(&mut app, "SelNone");
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        ids[..1]
    );
    pick(&mut app, ids[1]);
    app.cancel_interactive_command(true);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        ids[..1]
    );
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(app.document.undo_label(), undo.as_deref());
}

#[test]
fn successful_options_are_remembered_and_failed_common_intersection_ends_selection() {
    let (mut app, ids) = fixture();
    enter(&mut app, "BooleanIntersection DeleteInput=No");
    pick(&mut app, ids[0]);
    pick(&mut app, ids[1]);
    enter(&mut app, "");
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 4);
    assert_eq!(app.document.selected_object_count(), 2);
    let result = output(&app, &ids);
    assert!(!app.document.is_selected(result));
    app.document.clear_selection();
    enter(&mut app, "BooleanIntersection");
    assert!(
        !app.intersection_prompt
            .as_ref()
            .unwrap()
            .boolean
            .as_ref()
            .unwrap()
            .delete_input
    );
    enter(&mut app, "Cancel");
    let (mut app, ids) = fixture();
    let extra = app
        .document
        .add_geometry(Geometry::Brep(
            Brep::try_box(
                Frame3::try_from_directions(
                    point(0., 0., 0.),
                    Vector3::try_new(1., 0., 0.).unwrap(),
                    Vector3::try_new(0., 1., 0.).unwrap(),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
                [[10., 11.]; 3],
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ))
        .unwrap();
    enter(&mut app, "BooleanIntersection");
    for id in [ids[0], ids[1], extra] {
        pick(&mut app, id);
    }
    enter(&mut app, "");
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().len(), 4);
    assert_eq!(app.document.selected_object_count(), 3);
}
