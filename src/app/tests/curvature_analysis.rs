use super::*;
use viboceros_oracle::curvature_analysis::{Fixture, settings, snapshot};
#[path = "../../../crates/viboceros-oracle/tests/support/curvature_analysis.rs"]
mod support;
use super::direction_analysis_support as compare;
fn enter(app: &mut VibocerosApp, s: &str) {
    app.command_input = s.into();
    app.run_command();
}

#[test]
fn native_commands_picks_visibility_history_and_analysis_switches_match() {
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
            if matches!(step.as_str(), "Hide" | "Delete") {
                app.document
                    .select_objects_direct([id], SelectionMode::Replace)
                    .unwrap();
            }
            enter(
                &mut app,
                &if step == "CurvatureAnalysis" {
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
        compare::compare(
            &serde_json::json!({"states":states,"settings":options}),
            &support::expected(&op, &native),
            op["id"].as_str().unwrap(),
        );
    }
}

#[test]
fn panel_controls_preserve_point_getters_and_typed_style_survives_picking() {
    let mut app = test_app();
    enter(&mut app, "SrfPt -2,-2,0 2,-2,0 2,2,0 -2,2,0");
    enter(&mut app, "SelNone");
    let id = app.document.objects().next().unwrap().id();
    enter(&mut app, "CurvatureAnalysis Range=-2,2 Style=Mean");
    assert!(app.object_prompt.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    let view = app.commands.curvature_analysis_view(&app.document).unwrap();
    assert_eq!(view.options.range(), [-2., 2.]);
    assert_eq!(view.options.style.name(), "Mean");
    enter(&mut app, "Line");
    let active = app.active_command;
    app.set_analysis_display("CurvatureAnalysis Style=Gaussian Range=-1,1 ShowIsoCurves=Yes");
    assert_eq!(app.active_command, active);
    app.set_analysis_display("CurvatureAnalysis MaxRange");
    assert_eq!(app.active_command, active);
    app.set_analysis_display("CurvatureAnalysisOff");
    assert_eq!(app.active_command, active);
}
