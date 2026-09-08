//! Replay actual Rhino prompt measurements, not constructor-derived expectations.

use super::*;
use serde_json::Value;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.to_owned();
    app.run_command();
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
