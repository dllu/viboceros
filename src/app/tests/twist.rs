use super::*;
use viboceros_geometry::Vector3;
#[test]
fn twist_axis_angle_options_cancel_copy_and_external_undo() {
    let mut app = test_app();
    app.execute_command("Point 2,1,5");
    app.execute_command("SelAll");
    assert!(app.try_start_interactive_command("Twist"));
    assert!(app.accept_drafting_point(Point3::try_new(0., 0., 0.).unwrap()));
    assert!(!app.accept_drafting_point(Point3::try_new(0., 0., 0.).unwrap()));
    assert!(app.accept_drafting_point(Point3::try_new(0., 0., 10.).unwrap()));
    assert!(app.try_continue_twist("Copy=Yes"));
    assert!(app.try_continue_twist("Rigid"));
    assert!(app.try_continue_twist("No"));
    assert!(app.try_continue_twist("90"));
    assert!(app.active_command.is_some());
    assert_eq!(app.document.objects().count(), 2);
    assert!(app.try_continue_twist("180"));
    assert_eq!(app.document.objects().count(), 3);
    assert!(app.try_continue_twist("Enter"));
    assert!(app.active_command.is_none());
    app.execute_command("Undo");
    assert_eq!(app.document.objects().count(), 1);
    app.execute_command("Redo");
    assert_eq!(app.document.objects().count(), 3);
}
#[test]
fn command_first_twist_uses_picked_sources_and_reference_points() {
    let mut app = test_app();
    app.execute_command("Point 2,1,5");
    let id = app.document.objects().next().unwrap().id();
    assert!(app.try_start_interactive_command("Twist"));
    assert!(app.object_prompt.is_some());
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    assert!(app.try_continue_transform_source_prompt("Enter"));
    for p in [[0., 0., 0.], [0., 0., 10.], [1., 0., 0.], [0., 1., 0.]] {
        assert!(app.accept_drafting_point(Point3::try_from(p).unwrap()));
    }
    assert!(app.active_command.is_none());
    assert_eq!(app.document.selected_object_count(), 0);
    let Geometry::Point(point) = app.document.object(id).unwrap().geometry() else {
        panic!();
    };
    assert!(
        point
            .distance_to(Point3::try_new(2_f64.sqrt() / 2., 3. * 2_f64.sqrt() / 2., 5.).unwrap())
            .unwrap()
            < 1e-12
    );
}

#[test]
fn tilted_twist_reference_plane_cancel_and_invalid_angle_retain_prompt_state() {
    let mut app = test_app();
    app.execute_command("Point 5,2,1");
    app.execute_command("SelAll");
    assert!(app.try_start_interactive_command("Twist"));
    for p in [[0., 0., 0.], [10., 0., 0.]] {
        assert!(app.accept_drafting_point(Point3::try_from(p).unwrap()));
    }
    let normal = app.drafting_plane.unwrap().z_axis().as_vector();
    assert!(normal.dot(Vector3::try_new(1., 0., 0.).unwrap()).unwrap() > 1. - 1e-12);
    let state = app.active_command;
    assert!(app.try_continue_twist("NaN"));
    assert_eq!(app.active_command, state);
    assert!(app.try_continue_twist("Cancel"));
    assert!(app.drafting_plane.is_none());
    assert!(app.twist_session.is_none());
    assert_eq!(app.document.selected_object_count(), 1);
}

#[test]
fn twist_mouse_turns_numeric_override_and_copy_cancel_use_one_history_entry() {
    let mut app = test_app();
    app.execute_command("Point 2,1,5");
    app.execute_command("SelAll");
    let id = app.document.objects().next().unwrap().id();
    assert!(app.try_start_interactive_command("Twist"));
    for p in [[0., 0., 0.], [0., 0., 10.], [5., 0., 0.]] {
        assert!(app.accept_drafting_point(Point3::try_from(p).unwrap()));
    }
    let original = app.document.object(id).unwrap().geometry().clone();
    assert_eq!(app.twist_preview().unwrap().last_angle, Some(0.));
    for angle in [90., 180., 270., 360., 450.] {
        assert!(app.update_twist_preview(Some(angle)));
    }
    assert!(!app.accept_drafting_point(Point3::try_new(0., 0., 2.).unwrap()));
    assert_eq!(app.twist_preview().unwrap().last_angle, Some(450.));
    assert_eq!(app.document.object(id).unwrap().geometry(), &original);
    assert!(app.try_continue_twist("Copy=Yes"));
    assert!(app.accept_drafting_point(Point3::try_new(0., 5., 0.).unwrap()));
    let first = app
        .document
        .objects()
        .find(|o| o.id() != id)
        .unwrap()
        .geometry()
        .clone();
    let Geometry::Point(first) = first else {
        panic!()
    };
    assert!(
        first
            .distance_to(Point3::try_new(-2_f64.sqrt() / 2., -3. * 2_f64.sqrt() / 2., 5.).unwrap())
            .unwrap()
            < 1e-12
    );
    assert!(app.twist_preview().is_none());
    assert!(app.accept_drafting_point(Point3::try_new(5., 0., 0.).unwrap()));
    assert!(app.update_twist_preview(Some(810.)));
    // A scalar remains explicit even after accepting the first reference direction.
    assert!(app.try_continue_twist("90"));
    let second = app
        .document
        .objects()
        .filter(|o| o.id() != id)
        .find(|o| o.geometry() != &Geometry::Point(first))
        .unwrap()
        .geometry();
    let Geometry::Point(second) = second else {
        panic!()
    };
    assert!(
        second
            .distance_to(Point3::try_new(2_f64.sqrt() / 2., 3. * 2_f64.sqrt() / 2., 5.).unwrap())
            .unwrap()
            < 1e-12
    );
    assert!(app.try_continue_twist("Cancel"));
    assert!(app.twist_preview().is_none());
    assert_eq!(app.document.objects().count(), 3);
    app.execute_command("Undo");
    assert_eq!(app.document.objects().count(), 1);
    assert_eq!(app.document.object(id).unwrap().geometry(), &original);
    app.execute_command("Redo");
    assert_eq!(app.document.objects().count(), 3);
}
