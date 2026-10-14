use super::*;
fn source() -> (VibocerosApp, viboceros_document::ObjectId) {
    let mut app = test_app();
    enter(&mut app, "Line 10,20,-3 10,20,5");
    let id = app.document.objects().next().unwrap().id();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    (app, id)
}
fn result(app: &VibocerosApp) -> Point3 {
    let Geometry::Point(p) = app.document.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    *p
}
#[test]
fn iplane_preselection_and_aliases_finish_without_a_second_enter() {
    for command in ["IPlane", "IP", "InfinitePlane"] {
        let (mut app, id) = source();
        let frame = app.viewports[0].construction_plane();
        enter(&mut app, command);
        assert_eq!(app.intersection_prompt.as_ref().unwrap().name(), "IPlane");
        enter(&mut app, "WorldXY");
        assert!(app.intersection_prompt.is_none());
        assert_eq!(result(&app), point(10., 20., 0.));
        assert_eq!(app.document.objects().len(), 2);
        assert!(app.document.object(id).is_some());
        assert_eq!(app.document.undo_label(), Some("IPlane"));
        assert_eq!(app.viewports[0].construction_plane(), frame);
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().len(), 1);
        enter(&mut app, "Redo");
        assert_eq!(result(&app), point(10., 20., 0.));
    }
}
#[test]
fn iplane_command_first_filters_sources_then_enters_shared_plane_getter() {
    let (mut app, id) = source();
    enter(&mut app, "Point 1,2,3");
    let excluded = app.document.objects().find(|o| o.id() != id).unwrap().id();
    app.document.clear_selection();
    enter(&mut app, "IPlane");
    assert_eq!(
        app.viewport_object_filter(),
        Some(viboceros_command::ObjectSelectionFilter::Section)
    );
    enter(&mut app, &excluded.to_string());
    assert_eq!(app.document.selected_object_count(), 0);
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    assert!(
        app.intersection_prompt
            .as_ref()
            .unwrap()
            .projection
            .as_ref()
            .unwrap()
            .plane_getter
            .is_some()
    );
    enter(&mut app, "ThroughPoint");
    assert_eq!(app.active_command.unwrap().name(), "IPlane");
    enter(&mut app, "w0,0,2");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(result(&app), point(10., 20., 2.));
}
#[test]
fn iplane_invalid_points_local_undo_and_cancel_restore_selection() {
    let (mut app, id) = source();
    let history = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "IPlane");
    enter(&mut app, "3Point");
    assert!(app.accept_drafting_point(point(0., 0., 0.)));
    assert!(!app.accept_drafting_point(point(0., 0., 0.)));
    enter(&mut app, "Undo");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::ProjectPlane {
            command: "IPlane",
            step: 0,
            ..
        })
    ));
    assert_eq!(app.document.undo_label(), history.as_deref());
    enter(&mut app, "Cancel");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![id]
    );
    assert_eq!(app.document.objects().len(), 1);
}
#[test]
fn iplane_named_cplane_and_default_enter_use_read_only_plane_definitions() {
    for named in [false, true] {
        let (mut app, _) = source();
        let frame = app.viewports[0].construction_plane();
        app.viewports[0].set_construction_plane(frame.with_origin(point(0., 0., 2.)));
        if named {
            enter(&mut app, "NamedCPlane Save section plane");
            app.viewports[0].set_construction_plane(frame);
        }
        let expected = app.viewports[0].construction_plane();
        enter(&mut app, "IPlane");
        if named {
            enter(&mut app, "NamedCPlane");
            enter(&mut app, "section plane");
        } else {
            enter(&mut app, "");
        }
        assert!(app.intersection_prompt.is_none());
        assert_eq!(result(&app), point(10., 20., 2.));
        assert_eq!(app.viewports[0].construction_plane(), expected);
    }
}
#[test]
fn iplane_incomplete_enter_restarts_plane_choice_and_rejects_project_options() {
    let (mut app, _) = source();
    enter(&mut app, "IPlane");
    enter(&mut app, "Direction=Custom");
    assert!(!app.picking_project_direction());
    enter(&mut app, "3Point");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "");
    assert!(!app.picking_project_plane_points());
    assert!(
        app.intersection_prompt
            .as_ref()
            .unwrap()
            .projection
            .as_ref()
            .unwrap()
            .plane_getter
            .is_some()
    );
    enter(&mut app, "WorldXY");
    assert_eq!(result(&app), point(10., 20., 0.));
}
#[test]
fn iplane_object_and_curve_references_finish_using_the_shared_picker() {
    for mode in ["Object", "AroundCurve"] {
        let (mut app, id) = source();
        if mode == "Object" {
            enter(&mut app, "SrfPt 0,0,2 1,0,2 1,1,2 0,1,2");
        }
        app.document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let reference = if mode == "Object" {
            app.document.objects().find(|o| o.id() != id).unwrap().id()
        } else {
            id
        };
        let original = app.document.object(reference).unwrap().clone();
        enter(&mut app, "IPlane");
        enter(&mut app, mode);
        app.apply_selection_click(SelectionClick {
            object_id: Some(reference),
            mode: SelectionMode::Replace,
        });
        if mode == "AroundCurve" {
            enter(&mut app, "w10,20,2");
        }
        assert!(app.intersection_prompt.is_none());
        assert!(result(&app).distance_to(point(10., 20., 2.)).unwrap() < 1e-7);
        assert_eq!(app.document.object(reference), Some(&original));
    }
}
