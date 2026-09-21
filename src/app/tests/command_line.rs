use super::*;

fn key(key: egui::Key, shift: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: if shift {
            egui::Modifiers::SHIFT
        } else {
            egui::Modifiers::NONE
        },
    }
}

fn keypress(code: egui::Key, shift: bool) -> Vec<egui::Event> {
    let mut release = key(code, shift);
    if let egui::Event::Key { pressed, .. } = &mut release {
        *pressed = false;
    }
    vec![key(code, shift), release]
}

#[test]
fn tabs_cycle_fuzzy_matches_and_shift_tab_goes_back_without_executing() {
    let context = egui::Context::default();
    let mut app = test_app();
    app.command_input = "po".into();
    app.command_focus_requested = true;
    command_line_frame(&context, &mut app, vec![]);
    for (shift, expected) in [(false, "Point "), (false, "PointCloud "), (true, "Point ")] {
        command_line_frame(&context, &mut app, keypress(egui::Key::Tab, shift));
        assert_eq!(app.command_input, expected);
    }
    assert_eq!(app.document.objects().len(), 0);
    app.command_input = "mshsph".into();
    command_line_frame(&context, &mut app, vec![key(egui::Key::Tab, false)]);
    assert_eq!(app.command_input, "MeshSphere ");
}

#[test]
fn arrows_recall_submitted_commands_restore_draft_and_work_from_viewport() {
    let context = egui::Context::default();
    let mut app = test_app();
    for command in ["Point 1,2,3", "Point 4,5,6"] {
        app.command_input = command.into();
        app.run_command();
    }
    app.command_input = "unfinished".into();
    app.command_focus_requested = true;
    command_line_frame(&context, &mut app, vec![]);
    for (keycode, expected) in [
        (egui::Key::ArrowUp, "Point 4,5,6"),
        (egui::Key::ArrowUp, "Point 1,2,3"),
        (egui::Key::ArrowDown, "Point 4,5,6"),
        (egui::Key::ArrowDown, "unfinished"),
    ] {
        command_line_frame(&context, &mut app, keypress(keycode, false));
        assert_eq!(app.command_input, expected);
    }
    assert_eq!(app.document.objects().len(), 2);
    let context = egui::Context::default();
    type_to_command_frame(&context, &mut app, vec![key(egui::Key::ArrowUp, false)]);
    assert_eq!(app.command_input, "Point 4,5,6");
    assert!(context.egui_wants_keyboard_input());
}

#[test]
fn history_records_commands_but_not_coordinate_prompt_responses() {
    let context = egui::Context::default();
    let mut app = test_app();
    for input in ["Line", "0,0,0", "2,3,0"] {
        app.command_input = input.into();
        app.run_command();
    }
    assert!(app.active_command.is_none());
    app.command_focus_requested = true;
    command_line_frame(&context, &mut app, vec![]);
    command_line_frame(&context, &mut app, vec![key(egui::Key::ArrowUp, false)]);
    assert_eq!(app.command_input, "Line");
}

#[test]
fn history_shortcuts_do_not_steal_keys_from_other_text_fields() {
    let mut app = test_app();
    app.command_input = "Help".into();
    app.run_command();
    let context = egui::Context::default();
    let mut name = String::from("Layer name");
    context
        .run_ui(egui::RawInput::default(), |ui| {
            ui.add(egui::TextEdit::singleline(&mut name).id(egui::Id::new("layer-editor")))
                .request_focus();
        })
        .drop_without_applying_deltas();
    context
        .run_ui(
            egui::RawInput {
                events: vec![key(egui::Key::ArrowUp, false)],
                ..Default::default()
            },
            |ui| {
                app.capture_global_command_typing(ui);
                assert!(ui.input(|input| input.key_pressed(egui::Key::ArrowUp)));
            },
        )
        .drop_without_applying_deltas();
    assert!(app.command_input.is_empty());
}

#[test]
fn directory_completion_leaves_caret_inside_quotes_for_a_new_export_filename() {
    let directory = crate::app::command_line::tests::TempDirectory::new();
    std::fs::create_dir(directory.0.join("parts with spaces")).unwrap();
    let context = egui::Context::default();
    let mut app = test_app();
    app.command_input = format!("Export3dm {}/par", directory.0.display());
    app.command_focus_requested = true;
    command_line_frame(&context, &mut app, vec![]);
    command_line_frame(&context, &mut app, keypress(egui::Key::Tab, false));
    assert!(app.command_input.ends_with("parts with spaces/\""));
    command_line_frame(
        &context,
        &mut app,
        vec![egui::Event::Text("new 模型.3dm".into())],
    );
    assert_eq!(
        app.command_input,
        format!(
            "Export3dm \"{}/parts with spaces/new 模型.3dm\"",
            directory.0.display()
        )
    );
    assert!(!directory.0.join("parts with spaces/new 模型.3dm").exists());
}
