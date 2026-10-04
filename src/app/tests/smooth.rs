use super::*;
use serde_json::{Value, json};
use viboceros_command::smooth::{Coordinates, Options};
use viboceros_document::ControlPointId;
use viboceros_geometry::{Frame3, Vector3};

mod workflow;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn point_value(value: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(value.clone()).unwrap()).unwrap()
}
fn snapshot(app: &VibocerosApp, id: ObjectId) -> Value {
    let mut rows = super::grip_transform::snapshot(app, id, None);
    for row in rows.as_array_mut().unwrap() {
        row.as_object_mut().unwrap().remove("role");
    }
    rows
}
fn compare(actual: &Value, expected: &Value, label: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => assert!(
            (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 2e-12,
            "{label}: {actual} vs {expected}"
        ),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{label}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{label}/{i}"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{label}"
            );
            for (key, b) in b {
                compare(&a[key], b, &format!("{label}/{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{label}"),
    }
}

#[test]
fn smooth_replays_304_native_commands_complete_geometry_grips_selection_and_history() {
    let matrices = [
        (
            include_str!("../../../tools/rhino_oracle/fixtures/smooth_fixed.json"),
            include_str!("../../../tools/rhino_oracle/observations/smooth_fixed.json"),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/smooth_selected.json"),
            include_str!("../../../tools/rhino_oracle/observations/smooth_selected.json"),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/smooth_object.json"),
            include_str!("../../../tools/rhino_oracle/observations/smooth_object.json"),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/smooth_object_selected.json"),
            include_str!("../../../tools/rhino_oracle/observations/smooth_object_selected.json"),
        ),
    ];
    let mut verified = 0;
    for (request, capture) in matrices {
        let request: Value = serde_json::from_str(request).unwrap();
        let capture: Value = serde_json::from_str(capture).unwrap();
        let ops = request["operations"].as_array().unwrap();
        let rows = capture["results"].as_array().unwrap();
        assert_eq!(ops.len(), rows.len());
        for (op, row) in ops.iter().zip(rows) {
            assert_eq!(op["id"], row["id"]);
            let label = op["id"].as_str().unwrap();
            let native = &row["value"];
            let mut app = test_app();
            let mut factory = op.clone();
            if op["source"].as_str().unwrap().starts_with("mesh") {
                factory["source"] = json!("mesh");
            }
            let id = app
                .document
                .add_geometry(super::grip_transform::geometry(
                    &factory,
                    &native["before"][0],
                ))
                .unwrap();
            app.document
                .set_object_names([(id, Some("smooth source".into()))])
                .unwrap();
            if native["before"][0]["grips_on"] == true {
                app.document.enable_control_points([id]).unwrap();
                app.document
                    .select_control_points(
                        native["before"][0]["grips"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .filter(|g| g["selected"] == true)
                            .map(|g| ControlPointId {
                                object: id,
                                index: g["index"].as_u64().unwrap() as usize,
                            }),
                        SelectionMode::Add,
                    )
                    .unwrap();
            }
            if native["before"][0]["selected"] == true {
                app.document
                    .select_objects_direct([id], SelectionMode::Add)
                    .unwrap();
            }
            let plane = &native["plane"];
            let frame = Frame3::try_from_directions(
                point_value(&plane["origin"]),
                Vector3::try_from(
                    serde_json::from_value::<[f64; 3]>(plane["x_axis"].clone()).unwrap(),
                )
                .unwrap(),
                Vector3::try_from(
                    serde_json::from_value::<[f64; 3]>(plane["y_axis"].clone()).unwrap(),
                )
                .unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap();
            app.viewports[app.active_viewport].set_construction_plane(frame);
            app.document.clear_history().unwrap();
            compare(
                &snapshot(&app, id),
                &native["before"],
                &format!("{label}/before"),
            );
            enter(&mut app, "_-Smooth");
            if op["selection"] == "command_first" {
                enter(&mut app, "_SelAll");
                enter(&mut app, "");
            }
            assert_eq!(
                app.object_prompt.as_ref().unwrap().phase,
                crate::app::object_selection::ObjectPromptPhase::Options,
                "{label}"
            );
            let args = native["macro"]
                .as_str()
                .unwrap()
                .split_whitespace()
                .skip_while(|s| !s.starts_with("_SmoothFactor="))
                .take_while(|s| *s != "_Enter")
                .collect::<Vec<_>>()
                .join(" ");
            assert!(!args.is_empty());
            enter(&mut app, &args);
            enter(&mut app, "");
            assert!(
                app.object_prompt.is_none(),
                "{label}: {:?}",
                app.command_log
            );
            compare(
                &snapshot(&app, id),
                &native["after"],
                &format!("{label}/end"),
            );
            enter(&mut app, "_Cancel");
            enter(&mut app, "_Cancel");
            compare(
                &snapshot(&app, id),
                &native["after_script"],
                &format!("{label}/post-macro"),
            );
            enter(&mut app, "Undo");
            compare(
                &snapshot(&app, id),
                &native["undo"],
                &format!("{label}/undo"),
            );
            assert!(!app.document.can_undo(), "{label}: expected one record");
            enter(&mut app, "Redo");
            compare(
                &snapshot(&app, id),
                &native["redo"],
                &format!("{label}/redo"),
            );
            verified += 1;
        }
    }
    assert_eq!(verified, 304);
}

#[test]
fn smooth_numeric_options_validate_atomically_cancel_discards_and_undo_retains_memory() {
    let mut app = test_app();
    let id = app
        .document
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_clamped_uniform(
                2,
                vec![
                    point(2., 0., 0.),
                    point(0., 2., 0.),
                    point(-2., 0., 0.),
                    point(0., -2., 0.),
                ],
            )
            .unwrap(),
        ))
        .unwrap();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let before = app.document.object(id).unwrap().clone();
    enter(&mut app, "Smooth");
    enter(&mut app, "SmoothFactor");
    for bad in ["NaN", "inf", "garbage"] {
        enter(&mut app, bad);
    }
    assert_eq!(app.commands.smooth_options_default(), Options::default());
    enter(&mut app, "-.4");
    assert!(
        app.object_prompt
            .as_ref()
            .unwrap()
            .command_override
            .as_ref()
            .unwrap()
            .contains("SmoothFactor=-0.4")
    );
    enter(&mut app, "Steps");
    for bad in ["0", "-1", "2.5", "2147483648"] {
        enter(&mut app, bad);
        assert_eq!(
            app.object_prompt.as_ref().unwrap().phase,
            crate::app::object_selection::ObjectPromptPhase::SmoothSteps
        );
    }
    assert_eq!(app.commands.smooth_options_default(), Options::default());
    enter(&mut app, "2");
    enter(&mut app, "CoordinateSystem");
    enter(&mut app, "_Object");
    enter(&mut app, "X Y FixBoundaries");
    let local = |app: &VibocerosApp| {
        viboceros_command::smooth::parse(
            &app.object_prompt
                .as_ref()
                .unwrap()
                .command_override
                .as_ref()
                .unwrap()
                .split_whitespace()
                .skip(1)
                .collect::<Vec<_>>(),
            Options::default(),
        )
        .unwrap()
    };
    let accepted = local(&app);
    enter(&mut app, "Steps=3 X=Yes SmoothFactor=NaN");
    assert_eq!(local(&app), accepted);
    app.cancel_current_prompt_or_selection();
    assert_eq!(
        app.document.object(id).unwrap().geometry(),
        before.geometry()
    );
    assert!(!app.document.can_undo());
    assert_eq!(accepted.coordinates, Coordinates::Object);
    assert_eq!(app.commands.smooth_options_default(), Options::default());
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, &accepted.command_line());
    enter(&mut app, "");
    enter(&mut app, "Undo");
    assert_eq!(app.commands.smooth_options_default(), accepted);
    assert_eq!(
        app.document.object(id).unwrap().geometry(),
        before.geometry()
    );
}
