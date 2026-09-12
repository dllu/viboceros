use super::*;

#[test]
fn radius_without_preselection_keeps_failed_pick_open_then_measures() {
    for name in ["Radius", "Diameter"] {
        let mut app = test_app();
        let before = format!("{:?}", app.document);
        assert!(app.try_start_interactive_command(name));
        let pending = app.active_command;
        let previous_last = app.last_point;
        assert!(!app.accept_drafting_point(point(2., 0., 0.)));
        assert_eq!(app.active_command, pending);
        assert_eq!(app.last_point, previous_last);
        assert_eq!(format!("{:?}", app.document), before);
        // Simulate a document update without dispatching a replacement command.
        app.commands
            .execute(&mut app.document, "Circle 0,0,0 2")
            .unwrap();
        assert_eq!(app.document.selected_object_count(), 0);
        let before = format!("{:?}", app.document);
        assert!(app.accept_drafting_point(point(2., 0., 0.)));
        assert!(app.active_command.is_none());
        assert_eq!(app.command_log.back().unwrap(), "Radius = 2; Diameter = 4");
        assert_eq!(format!("{:?}", app.document), before);
    }
}

#[test]
fn radius_and_diameter_point_input_report_mark_and_cancel() {
    for name in ["Radius", "Diameter"] {
        let mut app = test_app();
        app.execute_command("Circle 0,0,0 2");
        app.execute_command("SelAll");
        let before = format!("{:?}", app.document);
        assert!(app.try_start_interactive_command(name));
        assert!(app.accept_drafting_point(point(2., 0., 0.)));
        assert_eq!(app.command_log.back().unwrap(), "Radius = 2; Diameter = 4");
        assert_eq!(format!("{:?}", app.document), before);
        assert!(app.try_start_interactive_command(name));
        app.cancel_interactive_command(true);
        assert_eq!(format!("{:?}", app.document), before);
        assert!(app.try_start_interactive_command(&format!("{name} Mark{name}=Yes")));
        assert!(app.try_continue_point_input("w2,0,0"));
        assert_eq!(app.document.objects().count(), 3);
        assert_eq!(app.document.undo_label(), Some(name));
        app.execute_command("Undo");
        assert_eq!(app.document.objects().count(), 1);
        assert!(!app.try_start_interactive_command(&format!("{name} Mark{name}=Maybe")));
        assert!(!app.try_start_interactive_command(&format!("{name} 2,0,0")));
    }
}
