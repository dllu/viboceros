use super::*;

#[test]
fn length_postselection_reports_whole_curve() {
    let mut app = test_app();
    app.execute_command("Line 0,0,0 3,4,0");
    let id = app.document.objects().next().unwrap().id();
    app.command_input = "Length".into();
    app.run_command();
    assert!(app.object_prompt.is_some());
    app.select_prompt_objects([id], viboceros_document::SelectionMode::Replace);
    let before = format!("{:?}", app.document);
    assert!(app.try_continue_object_prompt(""));
    assert!(app.object_prompt.is_none());
    assert_eq!(
        app.command_log.back().unwrap(),
        "Measured 1 curve(s): total length 5"
    );
    assert_eq!(format!("{:?}", app.document), before);
}

#[test]
fn length_subcurve_picks_two_locations_for_pre_and_postselection() {
    for preselected in [false, true] {
        let mut app = test_app();
        app.execute_command("Line 0,0,0 10,0,0");
        let id = app.document.objects().next().unwrap().id();
        if preselected {
            app.document
                .select_object(id, viboceros_document::SelectionMode::Replace)
                .unwrap();
        }
        app.command_input = "Length SubCrv".into();
        app.run_command();
        if !preselected {
            assert!(app.object_prompt.is_some());
            app.select_prompt_objects([id], viboceros_document::SelectionMode::Replace);
            assert!(app.try_continue_object_prompt(""));
        }
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::LengthSubCrv {
                start: None,
                display_units: None,
            })
        );
        let before = format!("{:?}", app.document);
        assert!(app.accept_drafting_point(point(2., 0., 0.)));
        assert!(!app.accept_drafting_point(point(2., 0., 0.)));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::LengthSubCrv {
                start: Some(point(2., 0., 0.)),
                display_units: None,
            })
        );
        assert!(app.accept_drafting_point(point(8., 0., 0.)));
        assert!(app.active_command.is_none());
        assert_eq!(
            app.command_log.back().unwrap(),
            "Measured 1 curve(s): total length 6"
        );
        assert_eq!(format!("{:?}", app.document), before);
    }
}

#[test]
fn length_display_units_work_through_selection_and_subcurve_point_prompts() {
    let mut app = test_app();
    app.execute_command("Units Meters Scale=No");
    app.execute_command("Line 0,0,0 10,0,0");
    let id = app.document.objects().next().unwrap().id();

    app.command_input = "Length Units=cm".into();
    app.run_command();
    assert!(app.object_prompt.is_some());
    app.select_prompt_objects([id], viboceros_document::SelectionMode::Replace);
    assert!(app.try_continue_object_prompt(""));
    assert_eq!(
        app.command_log.back().unwrap(),
        "Measured 1 curve(s): total length 1000 Centimetres"
    );

    app.document.clear_selection();
    app.command_input = "Length SubCrv".into();
    app.run_command();
    assert!(app.object_prompt.is_some());
    assert!(app.try_continue_object_prompt("Units=cm"));
    app.select_prompt_objects([id], viboceros_document::SelectionMode::Replace);
    assert!(app.try_continue_object_prompt(""));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::LengthSubCrv {
            start: None,
            display_units: Some("Centimeters"),
        })
    );
    let before = format!("{:?}", app.document);
    assert!(app.accept_drafting_point(point(2., 0., 0.)));
    app.command_input = "Units=Model_Units".into();
    app.run_command();
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::LengthSubCrv {
            start: Some(point(2., 0., 0.)),
            display_units: None,
        })
    );
    app.command_input = "Units=cm".into();
    app.run_command();
    assert!(app.accept_drafting_point(point(8., 0., 0.)));
    assert_eq!(
        app.command_log.back().unwrap(),
        "Measured 1 curve(s): total length 600 Centimetres"
    );
    assert_eq!(format!("{:?}", app.document), before);
}

#[test]
fn length_subcurve_starts_with_display_units_on_a_preselected_curve() {
    let mut app = test_app();
    app.execute_command("Units Meters Scale=No");
    app.execute_command("Line 0,0,0 10,0,0");
    let id = app.document.objects().next().unwrap().id();
    app.document
        .select_object(id, viboceros_document::SelectionMode::Replace)
        .unwrap();
    app.command_input = "Len SubCrv Units=cm".into();
    app.run_command();
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::LengthSubCrv {
            start: None,
            display_units: Some("Centimeters"),
        })
    );
    let before = format!("{:?}", app.document);
    assert!(app.accept_drafting_point(point(2., 0., 0.)));
    assert!(app.accept_drafting_point(point(8., 0., 0.)));
    assert_eq!(
        app.command_log.back().unwrap(),
        "Measured 1 curve(s): total length 600 Centimetres"
    );
    assert_eq!(format!("{:?}", app.document), before);
}
