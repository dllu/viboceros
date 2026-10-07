use super::*;
use viboceros_document::SelectionMode;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.to_owned();
    app.run_command();
}
fn setup() -> (VibocerosApp, Vec<viboceros_document::ObjectId>) {
    let mut app = test_app();
    for input in [
        "SrfPt 0,0,0 4,0,0 4,6,2 0,6,0",
        "Rectangle 10,20 14,26",
        "Line 11,21.5 13,24.5",
        "Point 12,23",
    ] {
        enter(&mut app, input);
    }
    let ids = app.document.objects().map(|o| o.id()).collect();
    (app, ids)
}
#[test]
fn typed_sources_and_viewport_target_complete_apply_curves_in_one_step() {
    let (mut app, ids) = setup();
    app.document.clear_selection();
    enter(&mut app, "ApplyCrv");
    assert_eq!(app.intersection_prompt.as_ref().unwrap().name(), "ApplyCrv");
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    assert_eq!(app.document.selected_object_count(), 0);
    enter(
        &mut app,
        &ids[1..]
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(","),
    );
    enter(&mut app, "");
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().len(), 7);
    assert_eq!(app.document.selected_object_count(), 3);
    assert!(app.document.selected_objects().any(|o|matches!(o.geometry(),Geometry::Point(p) if p.distance_to(point(2.,3.,0.5)).unwrap()<1e-8)));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 4);
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Redo");
    assert_eq!(app.document.selected_object_count(), 3);
}
#[test]
fn preselection_and_cancellation_restore_sources_and_do_not_change_history() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct(ids[1..].iter().copied(), SelectionMode::Replace)
        .unwrap();
    let history = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "ApplyCurves");
    assert_eq!(
        app.intersection_prompt.as_ref().unwrap().first.as_deref(),
        Some(&ids[1..])
    );
    assert_eq!(app.document.selected_object_count(), 0);
    app.cancel_interactive_command(true);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        ids[1..]
    );
    assert_eq!(app.document.undo_label(), history.as_deref());
}
#[test]
fn apply_curves_uses_world_xy_with_a_rotated_construction_plane() {
    let (mut app, ids) = setup();
    enter(&mut app, "CPlane World Front");
    app.document
        .select_objects_direct(ids[1..].iter().copied(), SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "_ApplyCurves");
    enter(&mut app, &ids[0].to_string());
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().len(), 7);
}

#[test]
fn grouped_source_picks_expand_curves_and_points_and_keep_target_as_reference() {
    let (mut app, ids) = setup();
    app.document
        .add_group(Some("UV".into()), ids.iter().copied())
        .unwrap();
    app.document.clear_selection();
    enter(&mut app, "ApplyCrv");
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[2]),
        mode: SelectionMode::Replace,
    });
    assert_eq!(app.document.selected_object_count(), 3);
    assert!(!app.document.is_selected(ids[0]));
    enter(&mut app, "");
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    assert_eq!(app.document.objects().len(), 7);
    assert_eq!(app.document.selected_object_count(), 3);
}

#[test]
fn apply_uv_reference_face_click_keeps_whole_polysurface_and_maps_one_face() {
    let mut app = test_app();
    enter(&mut app, "Box 0,0,0 4,6,0 2");
    enter(&mut app, "Line 0,0 1,1");
    let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
    let original = app.document.object(ids[0]).unwrap().geometry().clone();
    app.document.select_command_results([ids[1]]).unwrap();
    enter(&mut app, "ApplyCrv");
    assert!(app.picking_uv_reference());
    let Geometry::Brep(brep) = &original else {
        panic!()
    };
    let surface = brep.faces()[5].surface().clone();
    app.accept_component_face_hit(ids[0], 5, None);
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.object(ids[0]).unwrap().geometry(), &original);
    let c = app
        .document
        .selected_objects()
        .next()
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap()
        .to_nurbs()
        .unwrap();
    for i in 0..=32 {
        let t = i as f64 / 32.;
        let expected = surface
            .evaluate(
                surface.parameter_at_u(t).unwrap(),
                surface.parameter_at_v(t).unwrap(),
            )
            .unwrap();
        assert!(
            c.parameter_sampler()
                .unwrap()
                .evaluate(t)
                .unwrap()
                .distance_to(expected)
                .unwrap()
                < 1e-9
        );
    }
}
