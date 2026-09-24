//! Actual Connect command probes for line and smooth NURBS extensions.

use crate::{ProbeError, curve_join_close::CurveInput};
use serde::Deserialize;
use serde_json::{Value, json};
use viboceros_geometry::{
    Curve3, CurveArcExtensionStyle, CurveOtherExtensionStyle, Tolerance,
    try_connect_curves_parts_with_styles,
};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    source_first: CurveInput,
    source_second: CurveInput,
    other_extension: String,
}

pub fn run(fixture: &Fixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    let style = match fixture.other_extension.as_str() {
        "Line" => CurveOtherExtensionStyle::Line,
        "Smooth" => CurveOtherExtensionStyle::Smooth,
        _ => {
            return Err(ProbeError::FixtureInvariant(
                "invalid Connect extension style",
            ));
        }
    };
    let first = fixture.source_first.geometry()?;
    let second = fixture.source_second.geometry()?;
    let first_pick = first.as_ref().end_point()?;
    let second_pick = second.as_ref().start_point()?;
    let outputs = try_connect_curves_parts_with_styles(
        &first,
        first_pick,
        &second,
        second_pick,
        CurveArcExtensionStyle::Arc,
        style,
        tolerance,
    )?;
    let mut curves = outputs
        .into_iter()
        .map(|curve| -> Result<Value, ProbeError> {
            let kind = if matches!(curve, Curve3::Line(_)) {
                "line"
            } else {
                "nurbs"
            };
            let representation = curve.as_ref().to_nurbs()?;
            let controls = representation
                .control_points()
                .iter()
                .map(|control| control.point().to_array())
                .collect::<Vec<_>>();
            Ok(json!({
                "kind": kind,
                "start": curve.as_ref().start_point()?.to_array(),
                "end": curve.as_ref().end_point()?.to_array(),
                "control_points": controls,
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    curves.sort_by(|first, second| {
        let kind_order = (first["kind"] != "nurbs").cmp(&(second["kind"] != "nurbs"));
        if !kind_order.is_eq() {
            return kind_order;
        }
        (0..3)
            .map(|index| {
                first["start"][index]
                    .as_f64()
                    .unwrap()
                    .total_cmp(&second["start"][index].as_f64().unwrap())
            })
            .find(|order| !order.is_eq())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok((json!({"curves": curves}), 0))
}

#[cfg(test)]
mod tests {
    use crate::{Operation, ProbeRequest, ProbeResponse, run_request};

    #[test]
    fn line_and_smooth_connect_pairs_match_saved_rhino_command() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/connect_nurbs_line.json"
        ))
        .unwrap();
        let rhino: ProbeResponse = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/connect_nurbs_line.json"
        ))
        .unwrap();
        let native = run_request(&request).unwrap();
        assert_eq!(native.results.len(), 4);
        for (index, (actual, expected)) in native.results.iter().zip(&rhino.results).enumerate() {
            assert!(matches!(
                request.operations[index],
                Operation::ConnectCommand { .. }
            ));
            assert_eq!(actual.id, expected.id);
            let actual_curves = actual.value["curves"].as_array().unwrap();
            let expected_curves = expected.value["curves"].as_array().unwrap();
            assert_eq!(actual_curves.len(), expected_curves.len());
            for (actual_curve, expected_curve) in actual_curves.iter().zip(expected_curves) {
                assert_eq!(actual_curve["kind"], expected_curve["kind"]);
                for field in ["start", "end"] {
                    for coordinate in 0..3 {
                        let error = (actual_curve[field][coordinate].as_f64().unwrap()
                            - expected_curve[field][coordinate].as_f64().unwrap())
                        .abs();
                        assert!(error < 1e-10, "{} {field} error {error}", actual.id);
                    }
                }
                let actual_controls = actual_curve["control_points"].as_array().unwrap();
                let expected_controls = expected_curve["control_points"].as_array().unwrap();
                assert_eq!(actual_controls.len(), expected_controls.len());
                for (actual_control, expected_control) in
                    actual_controls.iter().zip(expected_controls)
                {
                    for coordinate in 0..3 {
                        let error = (actual_control[coordinate].as_f64().unwrap()
                            - expected_control[coordinate].as_f64().unwrap())
                        .abs();
                        assert!(error < 1e-10, "{} control error {error}", actual.id);
                    }
                }
            }
        }
    }
}
