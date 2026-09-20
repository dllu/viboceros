use super::*;

#[test]
fn evaluate_uv_pre_and_postselection_preserve_options_and_query_history() {
    for preselected in [false, true] {
        let mut app = test_app();
        let surface = viboceros_geometry::NurbsSurface::try_bilinear([
            point(0., 0., 0.),
            point(4., 0., 0.),
            point(4., 2., 0.),
            point(0., 2., 0.),
        ])
        .unwrap();
        let id = app
            .document
            .add_geometry(Geometry::NurbsSurface(surface))
            .unwrap();
        if preselected {
            app.document
                .select_object(id, viboceros_document::SelectionMode::Replace)
                .unwrap();
        }
        app.command_input = "EvaluateUVPt Normalized=Yes".into();
        app.run_command();
        if !preselected {
            assert!(app.object_prompt.is_some());
            app.select_prompt_objects([id], viboceros_document::SelectionMode::Replace);
            let selected = format!("{:?}", app.document);
            assert!(app.try_continue_object_prompt(""));
            assert_eq!(format!("{:?}", app.document), selected);
        }
        assert!(app.object_prompt.is_none());
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::EvaluateUv {
                options: viboceros_command::EvaluateUvOptions {
                    normalized: true,
                    create_point: false
                }
            })
        );
        let before = format!("{:?}", app.document);
        assert!(app.accept_drafting_point(point(1., 1., 3.)));
        assert!(app.active_command.is_none());
        assert_eq!(
            app.command_log.back().unwrap(),
            "Surface UV coordinates = 0.25,0.5 (normalized)"
        );
        assert_eq!(format!("{:?}", app.document), before);
        assert!(app.try_start_interactive_command("EvaluateUVPt CreatePoint=Yes"));
        app.command_input = "Normalized=Yes".into();
        app.run_command();
        let pending = app.active_command;
        app.command_input = "Normalized=Maybe".into();
        app.run_command();
        assert_eq!(app.active_command, pending);
        assert_eq!(format!("{:?}", app.document), before);
        assert!(app.try_continue_point_input("w1,1,3"));
        assert_eq!(app.document.objects().count(), 2);
        assert_eq!(app.document.undo_label(), Some("EvaluateUVPt"));
        app.execute_command("Undo");
        assert_eq!(app.document.objects().count(), 1);
        let before = format!("{:?}", app.document);
        assert!(app.try_start_interactive_command("EvaluateUVPt"));
        app.cancel_interactive_command(true);
        assert_eq!(format!("{:?}", app.document), before);
    }
}

#[test]
fn evaluate_uv_failed_pick_keeps_point_prompt_and_previous_anchor() {
    let mut app = test_app();
    app.execute_command("SrfPt 0,0,0 4,0,0 4,2,0 0,2,0");
    app.execute_command("SelAll");
    assert!(app.try_start_interactive_command("EvaluateUVPt"));
    let pending = app.active_command;
    app.document.clear_selection();
    let before = format!("{:?}", app.document);
    let previous_last = app.last_point;
    assert!(!app.accept_drafting_point(point(1., 1., 3.)));
    assert_eq!(app.active_command, pending);
    assert_eq!(app.last_point, previous_last);
    assert_eq!(format!("{:?}", app.document), before);
}
