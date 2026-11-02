use super::direction_analysis_support as compare_support;
use super::*;
use viboceros_oracle::draft_angle_analysis::{Fixture, settings, snapshot};
#[path = "../../../crates/viboceros-oracle/tests/support/draft_angle.rs"]
mod support;

fn enter(app: &mut VibocerosApp, s: &str) {
    app.command_input = s.into();
    app.run_command();
}

#[test]
fn native_cases_replay_through_application_commands_and_object_picks() {
    for (op, native) in support::cases() {
        let f: Fixture = serde_json::from_value(op.clone()).unwrap();
        let mut app = test_app();
        let tolerance = Tolerance::try_new(1e-8, 1e-12, 1e-8).unwrap();
        app.document = Document::new(tolerance);
        let source = f.source(tolerance).unwrap();
        let id = app.document.add_geometry(source.clone()).unwrap();
        if f.preselect {
            app.document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
        }
        app.document.clear_history().unwrap();
        let mut states = vec![snapshot(&app.document, &app.commands, id)];
        let mut options = vec![settings(&app.document, &app.commands)];
        for step in &f.steps {
            if matches!(step.as_str(), "Delete" | "Hide") {
                app.document
                    .select_objects_direct([id], SelectionMode::Replace)
                    .unwrap();
            }
            enter(
                &mut app,
                &if step == "DraftAngleAnalysis" {
                    f.start_command()
                } else {
                    step.clone()
                },
            );
            if app.object_prompt.is_some() {
                app.apply_selection_click(SelectionClick {
                    object_id: Some(id),
                    mode: SelectionMode::Replace,
                });
                enter(&mut app, "");
            }
            assert!(
                app.object_prompt.is_none(),
                "{} {:?}",
                op["id"],
                app.command_log
            );
            if let Some(o) = app.document.object(id) {
                assert_eq!(o.geometry(), &source);
            }
            states.push(snapshot(&app.document, &app.commands, id));
            options.push(settings(&app.document, &app.commands));
        }
        compare_support::compare(
            &serde_json::json!({"states":states,"settings":options}),
            &support::expected(&op, &native),
            op["id"].as_str().unwrap(),
        );
    }
}

#[test]
fn panel_display_controls_preserve_active_point_getter() {
    let mut app = test_app();
    enter(&mut app, "SrfPt 0,0,0 2,0,0 2,2,0 0,2,0");
    enter(&mut app, "SelAll");
    enter(&mut app, "DraftAngleAnalysis");
    enter(&mut app, "Line");
    assert!(app.active_command.is_some());
    app.set_analysis_display("DraftAngleAnalysis Range=-5,5 Up=1,0,0");
    assert!(app.active_command.is_some());
    assert_eq!(
        app.commands
            .draft_angle_analysis_view(&app.document)
            .unwrap()
            .options
            .range,
        [-5., 5.]
    );
    app.set_analysis_display("DraftAngleAnalysisOff");
    assert!(app.active_command.is_some());
}
