use super::*;
use viboceros_document::ControlPointId;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn source(app: &mut VibocerosApp, edit: bool) -> ObjectId {
    enter(app, "ControlPointCurve 2 2,0,0 0,2,0 -2,0,0");
    let id = app.document.objects().next().unwrap().id();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(app, if edit { "EditPtOn" } else { "PointsOn" });
    app.document.clear_selection();
    id
}

#[test]
fn keyboard_grip_inversion_feeds_set_point_without_selecting_the_parent() {
    for edit in [false, true] {
        let mut app = test_app();
        let id = source(&mut app, edit);
        let before = app.document.control_point_locations(id).unwrap().to_vec();
        app.document
            .select_control_points(
                [ControlPointId {
                    object: id,
                    index: 1,
                }],
                SelectionMode::Add,
            )
            .unwrap();
        enter(&mut app, "InvertPt");
        assert_eq!(app.document.selected_control_points().count(), 2);
        assert!(!app.document.is_selected(id));
        enter(&mut app, "SetPt 0,0,5 XSet=No YSet=No ZSet=Yes");
        let points = app.document.control_point_locations(id).unwrap();
        for (index, q) in points.iter().enumerate() {
            let expected = if index == 1 {
                before[index]
            } else {
                point(before[index].x(), before[index].y(), 5.)
            };
            assert!(q.distance_to(expected).unwrap() < 1e-8);
        }
        assert_eq!(app.document.undo_label(), Some("SetPt"));
        enter(&mut app, "Undo");
        assert_eq!(app.document.control_point_locations(id).unwrap(), before);
    }
}

#[test]
fn point_selectors_keep_command_first_set_point_open_and_reject_extra_arguments_atomically() {
    for edit in [false, true] {
        let mut app = test_app();
        let id = source(&mut app, edit);
        enter(&mut app, "SetPt XSet=No YSet=No ZSet=Yes");
        assert!(app.object_prompt.is_some());
        enter(&mut app, "_-sElPt");
        assert!(app.object_prompt.is_some());
        assert_eq!(app.document.selected_control_points().count(), 3);
        assert!(!app.document.is_selected(id));
        let before = format!("{:?}", app.document);
        enter(&mut app, "SelPt extra");
        assert!(app.object_prompt.is_some());
        assert_eq!(format!("{:?}", app.document), before);
        enter(&mut app, "InvertPt");
        assert_eq!(app.document.selected_control_points().count(), 0);
        enter(&mut app, "InvertPt");
        assert_eq!(app.document.selected_control_points().count(), 3);
        enter(&mut app, "");
        assert!(matches!(
            app.active_command,
            Some(InteractiveCommand::SetPoint { .. })
        ));
        enter(&mut app, "0,0,7");
        assert!(app.active_command.is_none());
        assert!(
            app.document
                .control_point_locations(id)
                .unwrap()
                .iter()
                .all(|p| (p.z() - 7.).abs() < 1e-8)
        );
    }
}

#[test]
fn smooth_source_selection_filters_point_objects_while_point_selectors_keep_the_getter() {
    let mut app = test_app();
    source(&mut app, false);
    let point_id = app
        .document
        .add_geometry(Geometry::Point(point(0., -2., 0.)))
        .unwrap();
    enter(&mut app, "Smooth");
    enter(&mut app, "SelPt");
    assert!(
        app.object_prompt
            .as_ref()
            .is_some_and(|p| p.description.command == "Smooth")
    );
    assert_eq!(app.document.selected_control_points().count(), 3);
    assert!(!app.document.is_selected(point_id));
    enter(&mut app, "InvertPt");
    assert_eq!(app.document.selected_control_points().count(), 0);
    assert!(app.object_prompt.is_some());
    app.cancel_interactive_command(false);
}

#[test]
fn circle_fit_source_selection_can_select_points_and_invert_only_grips_before_enter() {
    let mut app = test_app();
    let parent = source(&mut app, false);
    let peer = app
        .document
        .add_geometry(Geometry::Point(point(0., -2., 0.)))
        .unwrap();
    let original = app.document.object(parent).unwrap().geometry().clone();
    enter(&mut app, "Circle FitPoints");
    enter(&mut app, "SelPt");
    assert!(app.object_prompt.is_some());
    assert_eq!(app.document.selected_control_points().count(), 3);
    assert!(app.document.is_selected(peer));
    assert!(!app.document.is_selected(parent));
    enter(&mut app, "InvertPt");
    assert_eq!(app.document.selected_control_points().count(), 0);
    assert!(app.document.is_selected(peer));
    enter(&mut app, "InvertPt");
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert!(
        app.document
            .objects()
            .any(|o| matches!(o.geometry(),Geometry::Circle(c) if (c.radius()-2.).abs()<1e-8))
    );
    assert_eq!(app.document.object(parent).unwrap().geometry(), &original);
    assert_eq!(app.document.undo_label(), Some("Circle"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), 2);
}
