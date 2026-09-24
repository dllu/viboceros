//! Shared line-input probes for public RhinoCommon curve blends.

use serde::Deserialize;
use serde_json::{Value, json};
use viboceros_geometry::{
    Curve3, CurveBlendContinuity, CurveBlendOptions, LineSegment, Point3, Tolerance,
    try_blend_curve,
};

use crate::ProbeError;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct BlendCurveFixture {
    first: [[f64; 3]; 2],
    second: [[f64; 3]; 2],
    continuity: String,
    #[serde(default)]
    continuity_first: Option<String>,
    #[serde(default)]
    continuity_second: Option<String>,
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
    let points = |pair: [[f64; 3]; 2]| -> Result<[Point3; 2], ProbeError> {
        Ok([Point3::try_from(pair[0])?, Point3::try_from(pair[1])?])
    };
    let [first_start, first_end] = points(fixture.first)?;
    let [second_start, second_end] = points(fixture.second)?;
    let first = Curve3::Line(LineSegment::try_new(first_start, first_end, tolerance)?);
    let second = Curve3::Line(LineSegment::try_new(second_start, second_end, tolerance)?);
    let blend = try_blend_curve(
        &first,
        first_end,
        &second,
        second_start,
        CurveBlendOptions {
            continuity: modes,
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
