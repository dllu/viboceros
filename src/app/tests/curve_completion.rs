use super::*;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn failed_curve_completion_keeps_filters_constraints_snap_override_and_redo() {
    for (command, points) in [
        ("Curve Degree=5", ["0", "1,0,0", "0"]),
        ("InterpCrv StartTangent=1,0,0", ["0", "2,3,0", "4,0,0"]),
    ] {
        let mut app = test_app();
        for input in ["Point 9,9,9", "Undo", command] {
            enter(&mut app, input);
        }
        for input in points {
            enter(&mut app, input);
        }
        for input in ["NoSnap", "2", ".xy"] {
            enter(&mut app, input);
        }
        assert!(app.point_filter.is_some());
        assert!(app.point_constraint.is_some());
        assert!(app.snaps.model_override.is_some());
        let state = (
            app.active_command,
            app.curve_points.clone(),
            app.drafting_plane,
            app.last_point,
            app.point_filter,
            app.point_constraint,
            app.snaps.model_override,
        );
        let document = format!("{:?}", app.document);
        enter(&mut app, "Close");
        assert_eq!(app.command_input, "Close");
        assert_eq!(
            (
                app.active_command,
                app.curve_points.clone(),
                app.drafting_plane,
                app.last_point,
                app.point_filter,
                app.point_constraint,
                app.snaps.model_override,
            ),
            state
        );
        assert_eq!(format!("{:?}", app.document), document);
        assert!(app.document.can_redo());

        enter(&mut app, "Close=Open");
        enter(&mut app, "");
        assert!(app.active_command.is_none());
        assert!(app.command_input.is_empty());
        assert!(app.point_filter.is_none());
        assert!(app.point_constraint.is_none());
        assert!(app.snaps.model_override.is_none());
        assert_eq!(app.document.objects().len(), 1);
        assert_eq!(
            app.document.undo_label(),
            Some(command.split_whitespace().next().unwrap())
        );
        assert!(!app.document.can_redo());
    }
}
