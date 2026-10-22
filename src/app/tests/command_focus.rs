use super::*;

#[test]
fn default_layout_has_three_parallel_views_and_one_perspective_view() {
    let app = test_app();
    assert_eq!(
        app.viewports.iter().map(Viewport::kind).collect::<Vec<_>>(),
        vec![
            ViewKind::Top,
            ViewKind::Perspective,
            ViewKind::Front,
            ViewKind::Right,
        ]
    );
}

#[test]
fn command_completion_prioritizes_prefixes_and_is_case_insensitive() {
    let commands = CommandRegistry::with_builtins();
    assert_eq!(
        command_completions(&commands, "pOlY")[..3],
        ["Polygon", "PolygonCount", "Polyline"]
    );
    assert_eq!(
        command_completions(&commands, "_eXtRuDeCrVt")[..2],
        ["ExtrudeCrvTapered", "ExtrudeCrvToPoint"]
    );
    assert!(command_completions(&commands, "Point ").is_empty());
    assert!(command_completions(&commands, "").is_empty());
}

#[test]
fn tab_completes_the_first_visible_command_match() {
    let context = egui::Context::default();
    let mut app = test_app();
    app.command_input = "po".to_owned();
    app.command_focus_requested = true;
    command_line_frame(&context, &mut app, Vec::new());
    command_line_frame(
        &context,
        &mut app,
        vec![egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: Some(egui::Key::Tab),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert_eq!(app.command_input, "Point ");
}

#[test]
fn typing_outside_the_command_box_queues_focus_and_text() {
    let mut app = test_app();
    let context = egui::Context::default();
    let mut input = egui::RawInput::default();
    input.events.push(egui::Event::Text("Line".to_owned()));
    context
        .run_ui(input, |ui| app.capture_global_command_typing(ui))
        .drop_without_applying_deltas();
    assert_eq!(app.command_input, "Line");
    assert!(app.command_focus_requested);
    command_line_frame(&context, &mut app, Vec::new());
    assert!(!app.command_focus_requested);
    assert!(context.egui_wants_keyboard_input());
}

#[test]
fn first_type_to_command_character_stays_at_the_start() {
    let context = egui::Context::default();
    let mut app = test_app();

    type_to_command_frame(&context, &mut app, vec![egui::Event::Text("L".to_owned())]);
    type_to_command_frame(
        &context,
        &mut app,
        vec![egui::Event::Text("ine".to_owned())],
    );

    assert_eq!(app.command_input, "Line");
}

#[test]
fn focused_buttons_do_not_block_type_to_command() {
    let context = egui::Context::default();
    let mut app = test_app();
    context
        .run_ui(egui::RawInput::default(), |ui| {
            ui.button("Toolbar button").request_focus();
        })
        .drop_without_applying_deltas();
    let mut input = egui::RawInput::default();
    input.events.push(egui::Event::Text("Circle".to_owned()));
    context
        .run_ui(input, |ui| {
            app.capture_global_command_typing(ui);
            let _ = ui.button("Toolbar button");
        })
        .drop_without_applying_deltas();
    assert_eq!(app.command_input, "Circle");
    assert!(app.command_focus_requested);
}

#[test]
fn type_to_command_does_not_steal_from_another_text_editor() {
    let context = egui::Context::default();
    let mut app = test_app();
    let mut layer_name = String::new();
    context
        .run_ui(egui::RawInput::default(), |ui| {
            ui.add(egui::TextEdit::singleline(&mut layer_name).id(egui::Id::new("layer-name")))
                .request_focus();
        })
        .drop_without_applying_deltas();
    let mut input = egui::RawInput::default();
    input
        .events
        .push(egui::Event::Text("Construction".to_owned()));
    context
        .run_ui(input, |ui| {
            app.capture_global_command_typing(ui);
            ui.add(egui::TextEdit::singleline(&mut layer_name).id(egui::Id::new("layer-name")));
        })
        .drop_without_applying_deltas();
    assert_eq!(layer_name, "Construction");
    assert!(app.command_input.is_empty());
    assert!(!app.command_focus_requested);
}

#[test]
fn app_commands_and_interactive_commands_are_case_insensitive() {
    let mut app = test_app();
    app.command_input = "pOiNt 7,8,9".to_owned();
    app.run_command();
    assert!(matches!(
        app.document.objects().next().unwrap().geometry(),
        Geometry::Point(created) if *created == point(7.0, 8.0, 9.0)
    ));
    assert!(app.try_start_interactive_command("lInE"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Line { start: None })
    );
}

#[test]
fn viewport_enter_finishes_an_interactive_curve() {
    let mut app = test_app();
    assert!(app.try_start_interactive_command("Polyline"));
    app.accept_drafting_point(point(0.0, 0.0, 0.0));
    app.accept_drafting_point(point(2.0, 0.0, 0.0));

    assert!(app.handle_viewport_action(ViewportOutput {
        enter_pressed: true,
        ..ViewportOutput::default()
    }));
    assert_eq!(app.active_command, None);
    assert!(matches!(
        app.document.objects().next().unwrap().geometry(),
        Geometry::Polyline(polyline) if polyline.segment_count() == 1
    ));
}
