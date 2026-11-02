use super::direction_analysis_support as support;
use super::*;
use viboceros_oracle::direction_analysis::{Fixture, settings, snapshot};

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn analysis_commands_picks_controls_and_history_match_native_states() {
    for (op, native) in support::cases() {
        let f: Fixture = serde_json::from_value(op.clone()).unwrap();
        let mut app = test_app();
        let tolerance = Tolerance::try_new(1e-8, 1e-12, 1e-8).unwrap();
        app.document = Document::new(tolerance);
        let mut ids = vec![
            app.document
                .add_geometry(f.source(tolerance).unwrap())
                .unwrap(),
        ];
        if f.companion {
            ids.push(
                app.document
                    .add_geometry(f.source(tolerance).unwrap())
                    .unwrap(),
            );
        }
        if f.preselect {
            app.document
                .select_objects_direct([ids[0]], SelectionMode::Replace)
                .unwrap();
        }
        app.document.clear_history().unwrap();
        let mut states = vec![snapshot(&app.document, &app.commands, &ids, f.companion).unwrap()];
        let mut flags = vec![settings(&app.document, &app.commands)];
        for (index, step) in f.steps.iter().enumerate() {
            if step == "Delete" {
                app.document
                    .select_objects_direct([ids[0]], SelectionMode::Replace)
                    .unwrap();
            }
            match step.as_str() {
                "SelectPrimary" | "SelectCompanion" => {
                    let index = usize::from(step == "SelectCompanion");
                    app.document
                        .select_objects_direct([ids[index]], SelectionMode::Replace)
                        .unwrap();
                }
                "SelNone" => {
                    app.document.clear_selection();
                }
                _ => {
                    enter(&mut app, &Fixture::command(step));
                    if app.object_prompt.is_some() {
                        let pick = *f.picks.get(&index.to_string()).unwrap_or(&0);
                        app.apply_selection_click(SelectionClick {
                            object_id: Some(ids[pick]),
                            mode: SelectionMode::Replace,
                        });
                        enter(&mut app, "");
                    }
                }
            }
            assert!(
                app.object_prompt.is_none(),
                "{}: {:?}",
                op["id"],
                app.command_log
            );
            states.push(snapshot(&app.document, &app.commands, &ids, f.companion).unwrap());
            flags.push(settings(&app.document, &app.commands));
        }
        support::compare(
            &serde_json::json!({"states":states,"settings":flags}),
            &native,
            op["id"].as_str().unwrap(),
        );
    }
}

#[test]
fn display_settings_and_close_leave_an_active_point_getter_untouched() {
    let mut app = test_app();
    app.commands
        .execute(&mut app.document, "Line 0,0,0 4,2,1")
        .unwrap();
    let id = app.document.objects().last().unwrap().id();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ShowDir");
    enter(&mut app, "Line");
    enter(&mut app, "10,10,10");
    let command = app.active_command;
    let before = format!("{:?}", app.document);
    app.set_analysis_display("ShowDir Direction=No U=No V=Yes Color=12,34,56");
    assert_eq!(app.active_command, command);
    assert_eq!(format!("{:?}", app.document), before);
    app.set_analysis_display("ShowDirOff");
    assert_eq!(app.active_command, command);
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "11,12,13");
    assert!(app.command_line_idle());
    assert_eq!(app.document.objects().count(), 2);
}

#[test]
fn command_first_analysis_keeps_typed_flags_through_object_picking() {
    let mut app = test_app();
    app.commands
        .execute(&mut app.document, "Line 0,0,0 4,2,1")
        .unwrap();
    let id = app.document.objects().last().unwrap().id();
    enter(&mut app, "ShowDir Direction=No U=No V=Yes");
    assert!(app.object_prompt.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    let view = app.commands.direction_analysis_view(&app.document).unwrap();
    assert!(!view.options.direction && !view.options.u && view.options.v);
    assert!(app.document.is_selected(id));
}
