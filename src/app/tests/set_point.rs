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
fn interactive_set_point_uses_picked_target_and_prompts_for_selection() {
    let mut app = test_app();
    enter(&mut app, "SetPt");
    assert!(app.object_prompt.is_some());
    app.cancel_interactive_command(false);
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

fn grip_source(app: &mut VibocerosApp, edit: bool) -> (ObjectId, Vec<Point3>) {
    use viboceros_document::ControlPointId;
    enter(app, "ControlPointCurve 2 0,0,1 1,3,2 3,-2,3 4,0,4");
    let id = app.document.objects().next().unwrap().id();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(app, if edit { "EditPtOn" } else { "PointsOn" });
    let points = app.document.control_point_locations(id).unwrap().to_vec();
    app.document
        .select_control_points(
            [ControlPointId {
                object: id,
                index: 1,
            }],
            SelectionMode::Replace,
        )
        .unwrap();
    (id, points)
}

#[test]
fn set_point_accepts_control_and_edit_grips_in_complete_and_incremental_input() {
    for edit in [false, true] {
        for incremental in [false, true] {
            let mut app = test_app();
            let (id, before) = grip_source(&mut app, edit);
            let original = app.document.object(id).unwrap().geometry().clone();
            if incremental {
                enter(&mut app, "SetPt XSet=No YSet=No ZSet=Yes");
                assert!(app.active_command.is_some());
                assert!(app.accept_drafting_point(point(0., 0., 8.)));
            } else {
                enter(&mut app, "SetPt 0,0,8 XSet=No YSet=No ZSet=Yes");
            }
            assert!(app.active_command.is_none());
            assert!(app.set_point_sources.is_none());
            assert_eq!(app.document.undo_label(), Some("SetPt"));
            for (i, q) in app
                .document
                .control_point_locations(id)
                .unwrap()
                .iter()
                .enumerate()
            {
                let expected = if i == 1 {
                    point(before[i].x(), before[i].y(), 8.)
                } else {
                    before[i]
                };
                assert!(q.distance_to(expected).unwrap() < 1e-8);
            }
            enter(&mut app, "Undo");
            assert_eq!(app.document.object(id).unwrap().geometry(), &original);
        }
    }
}

#[test]
fn command_first_set_point_retains_options_and_repeated_copies_use_frozen_grips() {
    use viboceros_document::{ControlPointId, GripMode};
    for edit in [false, true] {
        let mut app = test_app();
        let (id, before) = grip_source(&mut app, edit);
        app.document.clear_selection();
        enter(&mut app, "SetPt XSet=No YSet=No ZSet=Yes Copy=Yes");
        assert!(app.object_prompt.is_some());
        app.document
            .select_control_points(
                [ControlPointId {
                    object: id,
                    index: 1,
                }],
                SelectionMode::Add,
            )
            .unwrap();
        enter(&mut app, "");
        assert!(app.object_prompt.is_none());
        assert!(
            matches!(app.active_command,Some(InteractiveCommand::SetPoint {options}) if options.copy && options.axes == [false,false,true])
        );
        for z in [8., 9.] {
            enter(&mut app, &format!("0,0,{z}"));
            assert!(app.active_command.is_some());
            assert!(app.set_point_sources.is_some());
        }
        assert_eq!(app.document.objects().count(), 3);
        assert_eq!(app.document.control_point_locations(id).unwrap(), before);
        let copies = app
            .document
            .objects()
            .filter(|o| o.id() != id)
            .map(|o| o.id())
            .collect::<Vec<_>>();
        for (&copy, z) in copies.iter().zip([8., 9.]) {
            assert_eq!(
                app.document.grip_mode(copy),
                Some(if edit {
                    GripMode::Edit
                } else {
                    GripMode::Control
                })
            );
            let points = app.document.control_point_locations(copy).unwrap();
            for (i, q) in points.iter().enumerate() {
                let expected = if i == 1 {
                    point(before[i].x(), before[i].y(), z)
                } else {
                    before[i]
                };
                assert!(q.distance_to(expected).unwrap() < 1e-8);
            }
        }
        enter(&mut app, "");
        assert!(app.active_command.is_none());
        assert!(app.set_point_sources.is_none());
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().count(), 2);
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().count(), 1);
    }
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
