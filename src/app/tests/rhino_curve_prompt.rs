//! Replay actual Rhino prompt measurements, not constructor-derived expectations.

use super::*;
use serde_json::Value;

#[test]
fn successful_recorded_interpolation_prompts_match_despite_coarse_model_tolerance() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/interpolation_point_prompt_rhino_only.json"
    ))
    .unwrap();
    let response: Value = serde_json::from_str(include_str!(
        "../../../docs/interpolation-point-prompt-measurement.json"
    ))
    .unwrap();
    let mut successes = 0;
    let mut failures = 0;
    assert_eq!(response["engine"], "rhino");
    assert_eq!(request["operations"].as_array().unwrap().len(), 8);
    assert_eq!(response["results"].as_array().unwrap().len(), 8);
    for operation in request["operations"].as_array().unwrap() {
        let id = operation["id"].as_str().unwrap();
        let result = response["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap();
        let expected = &result["value"];
        if expected["command_succeeded"] == false {
            // Rhino solver rejection is diagnostic, not an instruction to
            // reject distinct inputs that our solver may be able to handle.
            failures += 1;
            continue;
        }
        let mut app = test_app();
        app.document.set_tolerance(
            Tolerance::try_new(
                request["tolerance"]["absolute"].as_f64().unwrap(),
                request["tolerance"]["relative"].as_f64().unwrap(),
                request["tolerance"]["angular"].as_f64().unwrap(),
            )
            .unwrap(),
        );
        enter(&mut app, "InterpCrv");
        for input in operation["points"].as_array().unwrap() {
            enter(&mut app, input.as_str().unwrap());
        }
        let preview = app.curve_draft_preview().unwrap();
        enter(&mut app, "");
        assert!(app.active_command.is_none(), "{id}");
        let Geometry::NurbsCurve(curve) = app.document.objects().next().unwrap().geometry() else {
            panic!("curve");
        };
        assert_eq!(curve, preview.as_ref());
        assert_eq!(
            curve.degree(),
            expected["degree"].as_u64().unwrap() as usize
        );
        assert_eq!(
            curve.is_closed().unwrap(),
            expected["closed"].as_bool().unwrap()
        );
        let controls = expected["control_points"].as_array().unwrap();
        assert_eq!(curve.control_points().len(), controls.len(), "{id}");
        for (actual, expected) in curve.control_points().iter().zip(controls) {
            let expected = Point3::try_new(
                expected[0].as_f64().unwrap(),
                expected[1].as_f64().unwrap(),
                expected[2].as_f64().unwrap(),
            )
            .unwrap();
            let error = actual.point().distance_to(expected).unwrap();
            assert!(error <= 1e-9, "{id}: control error {error}");
        }
        successes += 1;
    }
    assert_eq!((successes, failures), (6, 2));
}

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.to_owned();
    app.run_command();
}

#[test]
fn recorded_interpolation_closures_match_prompt_completion() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/interpolation_closure_prompt_rhino_only.json"
    ))
    .unwrap();
    let response: Value = serde_json::from_str(include_str!(
        "../../../docs/interpolation-closure-prompt-measurement.json"
    ))
    .unwrap();
    assert_eq!(response["engine"], "rhino");
    assert_eq!(response["results"].as_array().unwrap().len(), 2);
    assert_eq!(request["operations"].as_array().unwrap().len(), 2);
    replay_interpolation_closures(&request, &response);
}

fn replay_interpolation_closures(request: &Value, response: &Value) {
    assert_eq!(response["engine"], "rhino");
    assert_eq!(request["protocol_version"], response["protocol_version"]);
    assert_eq!(
        request["operations"].as_array().unwrap().len(),
        response["results"].as_array().unwrap().len()
    );
    for operation in request["operations"].as_array().unwrap() {
        let closure = operation["closure"].as_str().unwrap();
        assert!(matches!(closure, "Smooth" | "Sharp"));
        assert_eq!(operation["degree"], 3);
        assert_eq!(operation["origin"], serde_json::json!([0, 0, 0]));
        assert_eq!(operation["x_axis"], serde_json::json!([1, 0, 0]));
        assert_eq!(operation["y_axis"], serde_json::json!([0, 1, 0]));
        let result = response["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == operation["id"])
            .unwrap();
        let mut app = test_app();
        app.document.set_tolerance(
            Tolerance::try_new(
                request["tolerance"]["absolute"].as_f64().unwrap(),
                request["tolerance"]["relative"].as_f64().unwrap(),
                request["tolerance"]["angular"].as_f64().unwrap(),
            )
            .unwrap(),
        );
        enter(&mut app, "InterpCrv");
        for point in operation["points"].as_array().unwrap() {
            enter(&mut app, point.as_str().unwrap());
        }
        enter(
            &mut app,
            if closure == "Smooth" {
                "Close"
            } else {
                "Sharp"
            },
        );
        assert!(app.active_command.is_none(), "{closure}");
        assert_eq!(app.document.objects().count(), 1);
        let Geometry::NurbsCurve(curve) = app.document.objects().next().unwrap().geometry() else {
            panic!("curve");
        };
        assert_eq!(
            curve.is_closed().unwrap(),
            result["value"]["closed"].as_bool().unwrap()
        );
        assert_eq!(
            curve.degree(),
            result["value"]["degree"].as_u64().unwrap() as usize
        );
        assert_eq!(curve.is_periodic(), closure == "Smooth");
        let controls = result["value"]["control_points"].as_array().unwrap();
        assert_eq!(curve.control_points().len(), controls.len());
        for (actual, expected) in curve.control_points().iter().zip(controls) {
            let expected = Point3::try_new(
                expected[0].as_f64().unwrap(),
                expected[1].as_f64().unwrap(),
                expected[2].as_f64().unwrap(),
            )
            .unwrap();
            assert!(
                actual.point().distance_to(expected).unwrap() <= 1e-9,
                "{closure}"
            );
        }
    }
}

#[test]
fn recorded_closed_interpolation_retains_nearby_points_at_both_model_tolerances() {
    let measurement: Value = serde_json::from_str(include_str!(
        "../../../docs/interpolation-closure-tolerance-measurement.json"
    ))
    .unwrap();
    let batches = measurement["batches"].as_array().unwrap();
    assert_eq!(batches.len(), 2);
    for batch in batches {
        assert_eq!(batch["request"]["operations"].as_array().unwrap().len(), 4);
        replay_interpolation_closures(&batch["request"], &batch["response"]);
    }
}

#[test]
fn recorded_rhino_curve_threshold_sequences_match_interactive_completion() {
    let measurement: Value = serde_json::from_str(include_str!(
        "../../../docs/control-point-threshold-measurement.json"
    ))
    .unwrap();
    let runs = measurement["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 3);
    let mut checked = 0;
    for run in runs {
        let request = &run["request"];
        let response = &run["response"];
        assert_eq!(request["protocol_version"], 1);
        assert_eq!(request["iterations"], 1);
        assert_eq!(response["protocol_version"], 1);
        assert_eq!(response["engine"], "rhino");
        let operations = request["operations"].as_array().unwrap();
        let results = response["results"].as_array().unwrap();
        assert_eq!(operations.len(), results.len());
        let mut ids = BTreeSet::new();
        for operation in operations {
            let id = operation["id"].as_str().unwrap();
            assert!(ids.insert(id));
            let matches = results
                .iter()
                .filter(|result| result["id"] == id)
                .collect::<Vec<_>>();
            assert_eq!(matches.len(), 1, "{id}");
            let result = matches[0];
            assert!(result.get("error").is_none(), "{result}");
            let expected = &result["value"];
            assert_eq!(operation["op"], "control_point_prompt");
            // These measurements intentionally use the world XY frame. Reject
            // new frames until the replay also sets that construction plane.
            assert_eq!(operation["origin"], serde_json::json!([0, 0, 0]));
            assert_eq!(operation["x_axis"], serde_json::json!([1, 0, 0]));
            assert_eq!(operation["y_axis"], serde_json::json!([0, 1, 0]));
            let tolerance = &request["tolerance"];
            let mut app = test_app();
            app.document.set_tolerance(
                Tolerance::try_new(
                    tolerance["absolute"].as_f64().unwrap(),
                    tolerance["relative"].as_f64().unwrap(),
                    tolerance["angular"].as_f64().unwrap(),
                )
                .unwrap(),
            );
            enter(
                &mut app,
                &format!("Curve Degree={}", operation["degree"].as_u64().unwrap()),
            );
            for input in operation["points"].as_array().unwrap() {
                enter(&mut app, input.as_str().unwrap());
            }
            let expected_points = expected["control_points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| {
                    let coordinates = value.as_array().unwrap();
                    assert_eq!(coordinates.len(), 3);
                    Point3::try_new(
                        coordinates[0].as_f64().unwrap(),
                        coordinates[1].as_f64().unwrap(),
                        coordinates[2].as_f64().unwrap(),
                    )
                    .unwrap()
                })
                .collect::<Vec<_>>();
            assert_eq!(app.curve_points, expected_points, "{id}");
            assert_eq!(app.document.objects().len(), 0);
            enter(&mut app, "");
            assert!(app.active_command.is_none(), "{id}");
            assert_eq!(app.document.objects().len(), 1);
            let Geometry::NurbsCurve(curve) = app.document.objects().next().unwrap().geometry()
            else {
                panic!("{id}: expected NURBS curve");
            };
            assert_eq!(
                curve.degree(),
                expected["degree"].as_u64().unwrap() as usize,
                "{id}"
            );
            assert_eq!(
                curve.is_closed().unwrap(),
                expected["closed"].as_bool().unwrap(),
                "{id}"
            );
            assert_eq!(
                curve
                    .control_points()
                    .iter()
                    .map(|p| p.point())
                    .collect::<Vec<_>>(),
                expected_points,
                "{id}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 30);
}
