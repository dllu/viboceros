use super::*;
use serde_json::Value;
use viboceros_command::subcurve_input::SubcurveMode;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

#[test]
fn subcurve_option_memory_replays_native_workflow_including_cancelled_changes() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/subcurve_preferences.json"
    ))
    .unwrap();
    let mut app = test_app();
    for row in q["results"][0]["value"]["records"].as_array().unwrap() {
        let step = &row["step"];
        if let Some(remember) = step["remember"].as_bool() {
            enter(
                &mut app,
                if remember {
                    "RememberCopyOptions Yes"
                } else {
                    "RememberCopyOptions No"
                },
            );
            continue;
        }
        let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
        for id in ids {
            app.document.delete_object(id).unwrap();
        }
        enter(&mut app, "Line 0,0 4,6");
        let id = app.document.objects().last().unwrap().id();
        app.document.select_command_results([id]).unwrap();
        app.document.clear_history().unwrap();
        let mut command = "SubCrv".to_owned();
        for (field, name) in [
            ("copy", "Copy"),
            ("mode", "Mode"),
            ("midpoint", "FromMidpoint"),
        ] {
            if let Some(value) = step.get(field) {
                let value = if let Some(b) = value.as_bool() {
                    if b { "Yes" } else { "No" }
                } else {
                    value.as_str().unwrap()
                };
                command.push_str(&format!(" {name}={value}"));
            }
        }
        enter(&mut app, &command);
        let pending = app.subcurve_prompt.as_ref().unwrap();
        let choices = &row["choices"];
        if let Some(copy) = choices["Copy"].as_str() {
            assert_eq!(pending.copy, copy == "Yes", "step {}", row["index"]);
        }
        if let Some(midpoint) = choices["FromMidpoint"].as_str() {
            assert_eq!(pending.from_midpoint, midpoint == "Yes");
        }
        if let Some(mode) = choices["Mode"].as_str() {
            assert_eq!(pending.mode, SubcurveMode::parse(mode).unwrap());
        }
        if step["query"] == true || step["finish"] == "Cancel" {
            app.cancel_current_prompt_or_selection();
        } else {
            enter(&mut app, "2,3,0");
            enter(&mut app, "3,4.5,0");
        }
        assert!(app.subcurve_prompt.is_none());
        assert_eq!(
            app.document.objects().len(),
            row["after"].as_array().unwrap().len(),
            "step {}",
            row["index"]
        );
        for (actual, expected) in app.document.objects().zip(row["after"].as_array().unwrap()) {
            assert_eq!(
                app.document.is_selected(actual.id()),
                expected["selected"].as_bool().unwrap()
            );
            if expected["kind"] == "point" {
                let Geometry::Point(p) = actual.geometry() else {
                    panic!()
                };
                let e = Point3::try_from(
                    serde_json::from_value::<[f64; 3]>(expected["point"].clone()).unwrap(),
                )
                .unwrap();
                assert!(p.distance_to(e).unwrap() < 1e-6);
            } else {
                let c = actual.geometry().curve_ref().unwrap().to_nurbs().unwrap();
                for s in expected["samples"].as_array().unwrap() {
                    let p =
                        Point3::try_from(serde_json::from_value::<[f64; 3]>(s.clone()).unwrap())
                            .unwrap();
                    let t = c.closest_parameter(p, app.document.tolerance()).unwrap();
                    assert!(c.evaluate(t).unwrap().distance_to(p).unwrap() < 1e-6);
                }
            }
        }
    }
}

#[test]
fn subcurve_option_edits_survive_cancel_and_undo_without_affecting_other_commands() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0 4,6");
    let source = app.document.objects().next().unwrap().id();
    app.document.select_command_results([source]).unwrap();
    app.document.clear_history().unwrap();
    let scale = app.commands.copy_default("Scale");
    enter(&mut app, "SubCrv");
    enter(&mut app, "Copy=Yes");
    enter(&mut app, "Mode=MarkEnds");
    enter(&mut app, "FromMidpoint=Yes");
    app.cancel_current_prompt_or_selection();
    assert_eq!(app.commands.copy_default("Scale"), scale);
    assert!(app.commands.subcurve_defaults().copy);
    assert_eq!(
        app.commands.subcurve_defaults().mode,
        SubcurveMode::MarkEnds
    );
    assert!(app.commands.subcurve_defaults().from_midpoint);
    assert!(!app.document.can_undo());
    enter(&mut app, "SubCrv");
    enter(&mut app, &source.to_string());
    enter(&mut app, "Mode=Invalid");
    enter(&mut app, "FromMidpoint=Maybe");
    enter(&mut app, "Copy=Maybe");
    assert_eq!(
        app.subcurve_prompt.as_ref().unwrap().mode,
        SubcurveMode::MarkEnds
    );
    enter(&mut app, "2,3");
    enter(&mut app, "1");
    assert_eq!(app.document.objects().len(), 3);
    enter(&mut app, "Undo");
    assert_eq!(
        app.commands.subcurve_defaults().mode,
        SubcurveMode::MarkEnds
    );
    assert!(app.commands.subcurve_defaults().copy);
}
