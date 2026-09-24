//! Shared curve-input probes for public RhinoCommon curve blends.

use serde::Deserialize;
use serde_json::{Value, json};
use viboceros_geometry::{
    Curve3, CurveBlendContinuity, CurveBlendOptions, LineSegment, Point3, Tolerance,
    try_blend_curve,
};

use crate::{ProbeError, curve_join_close::CurveInput};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum BlendPickEnd {
    Start,
    End,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct BlendCurveFixture {
    #[serde(default)]
    first: Option<[[f64; 3]; 2]>,
    #[serde(default)]
    second: Option<[[f64; 3]; 2]>,
    #[serde(default)]
    source_first: Option<CurveInput>,
    #[serde(default)]
    source_second: Option<CurveInput>,
    continuity: String,
    #[serde(default)]
    continuity_first: Option<String>,
    #[serde(default)]
    continuity_second: Option<String>,
    #[serde(default)]
    pick_first: Option<BlendPickEnd>,
    #[serde(default)]
    pick_second: Option<BlendPickEnd>,
}

pub fn run(fixture: &BlendCurveFixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    let continuity = |name: &str| -> Result<CurveBlendContinuity, ProbeError> {
        Ok(match name {
            "position" => CurveBlendContinuity::Position,
            "tangency" => CurveBlendContinuity::Tangency,
            "curvature" => CurveBlendContinuity::Curvature,
            _ => return Err(ProbeError::FixtureInvariant("invalid blend continuity")),
        })
    };
    let modes = [
        continuity(
            fixture
                .continuity_first
                .as_deref()
                .unwrap_or(&fixture.continuity),
        )?,
        continuity(
            fixture
                .continuity_second
                .as_deref()
                .unwrap_or(&fixture.continuity),
        )?,
    ];
    let source =
        |definition: Option<&CurveInput>, line: Option<[[f64; 3]; 2]>| match (definition, line) {
            (Some(definition), None) => Ok(definition.geometry()?),
            (None, Some([start, end])) => Ok(Curve3::Line(LineSegment::try_new(
                Point3::try_from(start)?,
                Point3::try_from(end)?,
                tolerance,
            )?)),
            _ => Err(ProbeError::FixtureInvariant(
                "blend source requires exactly one curve definition",
            )),
        };
    let first = source(fixture.source_first.as_ref(), fixture.first)?;
    let second = source(fixture.source_second.as_ref(), fixture.second)?;
    let first_reference = first.as_ref();
    let second_reference = second.as_ref();
    let first_parameter = match fixture.pick_first.unwrap_or(BlendPickEnd::End) {
        BlendPickEnd::Start => *first_reference.domain().start(),
        BlendPickEnd::End => *first_reference.domain().end(),
    };
    let second_parameter = match fixture.pick_second.unwrap_or(BlendPickEnd::Start) {
        BlendPickEnd::Start => *second_reference.domain().start(),
        BlendPickEnd::End => *second_reference.domain().end(),
    };
    let first_pick = first_reference.evaluate(first_parameter)?;
    let second_pick = second_reference.evaluate(second_parameter)?;
    let blend = try_blend_curve(
        &first,
        first_pick,
        &second,
        second_pick,
        CurveBlendOptions {
            continuity: modes,
            endpoint_specific: fixture.continuity_first.is_some()
                || fixture.continuity_second.is_some()
                || fixture.pick_first.is_some()
                || fixture.pick_second.is_some(),
            ..Default::default()
        },
        tolerance,
    )?;
    let domain = blend.domain();
    Ok((
        json!({
            "degree": blend.degree(),
            "domain": [*domain.start(), *domain.end()],
            "knots": blend.knots(),
            "control_points": blend.control_points().iter().map(|control| json!({
                "point": control.point().to_array(),
                "weight": control.weight(),
            })).collect::<Vec<_>>(),
        }),
        0,
    ))
}

#[cfg(test)]
mod tests {
    use crate::{Operation, ProbeRequest, ProbeResponse, run_request};

    #[test]
    fn line_blends_match_saved_rhino_control_shapes() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/blend_lines.json"
        ))
        .unwrap();
        let rhino: ProbeResponse = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/blend_lines.json"
        ))
        .unwrap();
        let native = run_request(&request).unwrap();
        assert_eq!(native.results.len(), rhino.results.len());
        for (index, (actual, expected)) in native.results.iter().zip(&rhino.results).enumerate() {
            let Operation::BlendCurve { fixture, .. } = &request.operations[index] else {
                panic!("blend line fixture contains another operation");
            };
            let first_mode = fixture
                .continuity_first
                .as_deref()
                .unwrap_or(&fixture.continuity);
            let second_mode = fixture
                .continuity_second
                .as_deref()
                .unwrap_or(&fixture.continuity);
            let same_continuity = first_mode == second_mode;
            assert_eq!(actual.id, expected.id);
            assert_eq!(actual.value["degree"], expected.value["degree"]);
            let actual_controls = actual.value["control_points"].as_array().unwrap();
            let expected_controls = expected.value["control_points"].as_array().unwrap();
            assert_eq!(actual_controls.len(), expected_controls.len());
            for (actual, expected) in actual_controls.iter().zip(expected_controls) {
                for coordinate in 0..3 {
                    let difference = (actual["point"][coordinate].as_f64().unwrap()
                        - expected["point"][coordinate].as_f64().unwrap())
                    .abs();
                    assert!(
                        difference < 1e-12,
                        "{} control difference {difference}",
                        actual
                    );
                }
                assert_eq!(actual["weight"], expected["weight"]);
            }
            let actual_end = actual.value["domain"][1].as_f64().unwrap();
            let expected_end = expected.value["domain"][1].as_f64().unwrap();
            let maximum = if same_continuity { 3e-8 } else { 1e-12 };
            assert!(
                (actual_end - expected_end).abs() < maximum,
                "{} domain difference {}",
                actual.id,
                actual_end - expected_end
            );
        }
    }

    #[test]
    fn parallel_source_mixed_blends_match_saved_rhino_control_shapes() {
        assert_full_line_blend_parity(
            include_str!("../../../tools/rhino_oracle/fixtures/blend_parallel_lines.json"),
            include_str!("../../../tools/rhino_oracle/observations/blend_parallel_lines.json"),
            48,
        );
    }

    #[test]
    fn spatial_mixed_blends_match_saved_rhino_control_shapes() {
        assert_full_line_blend_parity(
            include_str!("../../../tools/rhino_oracle/fixtures/blend_mixed_spatial.json"),
            include_str!("../../../tools/rhino_oracle/observations/blend_mixed_spatial.json"),
            96,
        );
    }

    #[test]
    fn alternate_endpoint_picks_match_saved_rhino_control_shapes() {
        assert_full_line_blend_parity(
            include_str!("../../../tools/rhino_oracle/fixtures/blend_endpoint_picks.json"),
            include_str!("../../../tools/rhino_oracle/observations/blend_endpoint_picks.json"),
            30,
        );
    }

    #[test]
    fn arc_source_blends_match_saved_rhino_control_shapes() {
        assert_curved_source_blend_parity(
            include_str!("../../../tools/rhino_oracle/fixtures/blend_curved_sources.json"),
            include_str!("../../../tools/rhino_oracle/observations/blend_curved_sources.json"),
            21,
        );
    }

    #[test]
    fn nurbs_source_blends_match_saved_rhino_control_shapes() {
        assert_curved_source_blend_parity(
            include_str!("../../../tools/rhino_oracle/fixtures/blend_nurbs_sources.json"),
            include_str!("../../../tools/rhino_oracle/observations/blend_nurbs_sources.json"),
            28,
        );
    }

    #[test]
    fn segmented_source_blends_match_saved_rhino_control_shapes() {
        assert_curved_source_blend_parity(
            include_str!("../../../tools/rhino_oracle/fixtures/blend_polycurve_sources.json"),
            include_str!("../../../tools/rhino_oracle/observations/blend_polycurve_sources.json"),
            21,
        );
    }

    fn assert_curved_source_blend_parity(
        request_json: &str,
        rhino_json: &str,
        expected_count: usize,
    ) {
        let request: ProbeRequest = serde_json::from_str(request_json).unwrap();
        let rhino: ProbeResponse = serde_json::from_str(rhino_json).unwrap();
        let native = run_request(&request).unwrap();
        assert_eq!(native.results.len(), expected_count);
        assert_eq!(native.results.len(), rhino.results.len());
        for (actual, expected) in native.results.iter().zip(&rhino.results) {
            assert_eq!(actual.id, expected.id);
            assert_eq!(actual.value["degree"], expected.value["degree"]);
            let actual_controls = actual.value["control_points"].as_array().unwrap();
            let expected_controls = expected.value["control_points"].as_array().unwrap();
            assert_eq!(actual_controls.len(), expected_controls.len());
            for (actual, expected) in actual_controls.iter().zip(expected_controls) {
                assert_eq!(actual["weight"], expected["weight"]);
                for coordinate in 0..3 {
                    let difference = (actual["point"][coordinate].as_f64().unwrap()
                        - expected["point"][coordinate].as_f64().unwrap())
                    .abs();
                    assert!(difference < 1e-12, "control difference {difference}");
                }
            }
            let maximum = if actual.id.ends_with("curvature-curvature") {
                3e-8
            } else {
                1e-12
            };
            for field in ["domain", "knots"] {
                let actual_values = actual.value[field].as_array().unwrap();
                let expected_values = expected.value[field].as_array().unwrap();
                assert_eq!(actual_values.len(), expected_values.len());
                for (actual_value, expected_value) in actual_values.iter().zip(expected_values) {
                    let difference =
                        (actual_value.as_f64().unwrap() - expected_value.as_f64().unwrap()).abs();
                    assert!(difference < maximum, "{field} difference {difference}");
                }
            }
        }
    }

    fn assert_full_line_blend_parity(request_json: &str, rhino_json: &str, expected_count: usize) {
        let request: ProbeRequest = serde_json::from_str(request_json).unwrap();
        let rhino: ProbeResponse = serde_json::from_str(rhino_json).unwrap();
        let native = run_request(&request).unwrap();
        assert_eq!(native.results.len(), expected_count);
        assert_eq!(native.results.len(), rhino.results.len());
        for (actual, expected) in native.results.iter().zip(&rhino.results) {
            assert_eq!(actual.id, expected.id);
            assert_eq!(actual.value["degree"], expected.value["degree"]);
            assert_eq!(actual.value["knots"], expected.value["knots"]);
            assert_eq!(actual.value["domain"], expected.value["domain"]);
            let actual_controls = actual.value["control_points"].as_array().unwrap();
            let expected_controls = expected.value["control_points"].as_array().unwrap();
            assert_eq!(actual_controls.len(), expected_controls.len());
            for (actual, expected) in actual_controls.iter().zip(expected_controls) {
                assert_eq!(actual["weight"], expected["weight"]);
                for coordinate in 0..3 {
                    let difference = (actual["point"][coordinate].as_f64().unwrap()
                        - expected["point"][coordinate].as_f64().unwrap())
                    .abs();
                    assert!(difference < 1e-12, "control difference {difference}");
                }
            }
        }
    }
}
