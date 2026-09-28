use super::*;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.to_owned();
    app.run_command();
}

#[test]
fn interactive_set_point_accepts_typed_targets_and_repeated_copies() {
    let mut app = test_app();
    app.active_viewport = 2; // Front: local Y is world Z.
    enter(&mut app, "Point 1,2,3");
    enter(&mut app, "SelAll");
    enter(
        &mut app,
        "SetPt XSet=No YSet=Yes ZSet=No Alignment=CPlane Copy=Yes",
    );
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::SetPoint { options }) if options.copy
    ));
    enter(&mut app, "YSet=No");
    assert!(app.command_log.back().unwrap().contains("at least one"));
    enter(&mut app, "Alignment=World");
    enter(&mut app, "Alignment=CPlane");
    enter(&mut app, "0,7");
    assert_eq!(app.document.objects().count(), 2);
    assert!(app.active_command.is_some());
    enter(&mut app, "w0,0,9");
    assert_eq!(app.document.objects().count(), 3);
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.selected_object_count(), 1);
    let points = app
        .document
        .objects()
        .map(|object| match object.geometry() {
            Geometry::Point(point) => point.to_array(),
            _ => panic!("point"),
        })
        .collect::<Vec<_>>();
    assert_eq!(points, [[1.0, 2.0, 3.0], [1.0, 2.0, 7.0], [1.0, 2.0, 9.0]]);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), 2);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), 1);
}

#[test]
fn interactive_set_point_uses_picked_target_and_requires_selection() {
    let mut app = test_app();
    enter(&mut app, "SetPt");
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("no objects are selected")
    );
    enter(&mut app, "Point 1,2,3");
    enter(&mut app, "SelAll");
    app.active_viewport = 2;
    enter(&mut app, "SetPt XSet=No YSet=Yes ZSet=No Alignment=CPlane");
    assert!(app.active_command.is_some());
    assert!(app.accept_drafting_point(point(0.0, 0.0, 5.0)));
    assert!(app.active_command.is_none());
    let Geometry::Point(changed) = app.document.objects().next().unwrap().geometry() else {
        panic!("point")
    };
    assert_eq!(*changed, point(1.0, 2.0, 5.0));
    enter(&mut app, "Undo");
    let Geometry::Point(restored) = app.document.objects().next().unwrap().geometry() else {
        panic!("point")
    };
    assert_eq!(*restored, point(1.0, 2.0, 3.0));
}

#[test]
fn interactive_set_point_keeps_prompt_after_degenerate_geometry() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 1,0,0");
    enter(&mut app, "SelAll");
    enter(&mut app, "SetPt");
    assert!(!app.accept_drafting_point(point(2.0, 3.0, 4.0)));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::SetPoint { .. })
    ));
    let Geometry::Line(original) = app.document.objects().next().unwrap().geometry() else {
        panic!("line")
    };
    assert_eq!(original.start(), point(0.0, 0.0, 0.0));
    assert_eq!(original.end(), point(1.0, 0.0, 0.0));
    enter(&mut app, "XSet=No");
    enter(&mut app, "YSet=No");
    assert!(app.accept_drafting_point(point(2.0, 3.0, 4.0)));
    assert!(app.active_command.is_none());
    let Geometry::Line(changed) = app.document.objects().next().unwrap().geometry() else {
        panic!("line")
    };
    assert_eq!(changed.start(), point(0.0, 0.0, 4.0));
    assert_eq!(changed.end(), point(1.0, 0.0, 4.0));
}
