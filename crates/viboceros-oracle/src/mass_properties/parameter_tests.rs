use crate::{ProbeRequest, run_request};
use serde_json::{Value, json};

const REQUEST: &str =
    include_str!("../../../../tools/rhino_oracle/fixtures/trim_parameter_frames.json");

fn expected(id: &str) -> (f64, Option<f64>) {
    let disk = |r: f64| std::f64::consts::PI / 6. * ((1. + 4. * r * r).powf(1.5) - 1.);
    if id.contains("plane") {
        (std::f64::consts::PI / 4., None)
    } else if id.contains("annulus") {
        (disk(0.5) - disk(0.25), None)
    } else if id.contains("capped") {
        (
            disk(0.5) + std::f64::consts::PI / 4.,
            Some((if id.contains("inward") { -1. } else { 1. }) * std::f64::consts::PI / 32.),
        )
    } else {
        assert!(id.contains("disk"));
        (disk(0.5), None)
    }
}

fn exact_numeric_data(a: &Value, b: &Value) {
    match (a, b) {
        // JSON spelling (1 versus 1.0) is not a geometric difference. Require
        // exactly equal binary64 values, not a tolerance or rounded records.
        (Value::Number(a), Value::Number(b)) => assert_eq!(a.as_f64(), b.as_f64()),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                exact_numeric_data(a, b);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
            for (key, a) in a {
                exact_numeric_data(a, &b[key]);
            }
        }
        _ => assert_eq!(a, b),
    }
}

fn assert_input_trims(operation: &Value, recorded: &Value) {
    let loops = operation["boundaries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|boundary| {
            let mut definition = boundary["parameter_curve"].clone();
            let p = definition["degree"].as_u64().unwrap() as usize;
            let n = definition["control_points"].as_array().unwrap().len();
            definition["domain"] = json!([definition["knots"][p], definition["knots"][n]]);
            for c in definition["control_points"].as_array_mut().unwrap() {
                c["point"].as_array_mut().unwrap().truncate(2);
            }
            json!([definition])
        })
        .collect::<Vec<_>>();
    let faces = if operation["cap_surface"].is_null() {
        1
    } else {
        2
    };
    exact_numeric_data(recorded, &json!(vec![loops; faces]));
}

#[test]
fn mass_commands_preserve_trim_frames_and_match_analytic_geometry_at_extreme_domains() {
    let request: ProbeRequest = serde_json::from_str(REQUEST).unwrap();
    let definitions: Value = serde_json::from_str(REQUEST).unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 35);
    for (result, operation) in actual
        .results
        .iter()
        .zip(definitions["operations"].as_array().unwrap())
    {
        assert_eq!(result.id, operation["id"]);
        assert_input_trims(operation, &result.value["trim_curves"]);
        let (area, volume) = expected(&result.id);
        assert!(
            (result.value["area"].as_f64().unwrap() - area).abs() < 1e-12,
            "{}",
            result.id
        );
        assert_eq!(result.value["is_solid"], volume.is_some());
        match volume {
            Some(v) => assert!(
                (result.value["volume"].as_f64().unwrap() - v).abs() < 1e-12,
                "{}",
                result.id
            ),
            None => assert!(result.value["volume"].is_null()),
        }
    }
}

#[test]
fn recorded_rhino_integrals_retain_exact_requested_trim_frames_and_numeric_residuals() {
    let definitions: Value = serde_json::from_str(REQUEST).unwrap();
    let reference: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/trim_parameter_frames.json"
    ))
    .unwrap();
    let records = reference["results"].as_array().unwrap();
    assert_eq!(records.len(), 35);
    let mut largest_residual = 0_f64;
    for (record, operation) in records
        .iter()
        .zip(definitions["operations"].as_array().unwrap())
    {
        assert_eq!(record["id"], operation["id"]);
        assert_input_trims(operation, &record["value"]["trim_curves"]);
        let (area, volume) = expected(record["id"].as_str().unwrap());
        let area_error = (record["value"]["area"].as_f64().unwrap() - area).abs();
        largest_residual = largest_residual.max(area_error);
        assert!(area_error < 1e-8);
        assert_eq!(record["value"]["is_solid"], volume.is_some());
        if let Some(v) = volume {
            let error = (record["value"]["volume"].as_f64().unwrap() - v).abs();
            assert!(error < 1e-8);
            largest_residual = largest_residual.max(error);
        } else {
            assert!(record["value"]["volume"].is_null());
        }
    }
    // Preserve Rhino's raw integration residual; this is not 1e-12 parity.
    assert!(largest_residual > 1e-9 && largest_residual < 1.3e-9);
}
