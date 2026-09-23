use super::*;

#[test]
fn area_postselection_accepts_display_units_at_start_or_during_selection() {
    let mut app = test_app();
    app.execute_command("Units Meters Scale=No");
    app.execute_command("Rectangle 0,0 3,4");
    let id = app.document.objects().next().unwrap().id();
    for input in ["Area Units=cm", "Area"] {
        app.document.clear_selection();
        app.command_input = input.into();
        app.run_command();
        assert!(app.object_prompt.is_some());
        if input == "Area" {
            assert!(app.try_continue_object_prompt("Units=cm"));
        }
        app.select_prompt_objects([id], viboceros_document::SelectionMode::Replace);
        let before = format!("{:?}", app.document);
        assert!(app.try_continue_object_prompt(""));
        assert!(app.object_prompt.is_none());
        assert_eq!(
            app.command_log.back().unwrap(),
            "Measured 1 object(s): total area 120000 Centimetres²"
        );
        assert_eq!(format!("{:?}", app.document), before);
    }
}
