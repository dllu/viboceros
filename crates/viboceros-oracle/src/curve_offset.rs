//! Geometry probes for offsets whose output parameterization can differ from Rhino.

use super::{ProbeError, curve_join_close::CurveInput, measure, point};
use serde::Deserialize;
use serde_json::{Value, json};
use viboceros_geometry::{Curve3, CurveOffsetCornerStyle, Point3, Tolerance, Vector3};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    curve: CurveInput,
    side: [f64; 3],
    normal: [f64; 3],
    distance: f64,
    corner: String,
    source_fractions: Vec<f64>,
    queries: Vec<[f64; 3]>,
}

pub(super) fn run(
    fixture: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if !fixture.distance.is_finite()
        || fixture.distance <= 0.0
        || fixture.source_fractions.is_empty()
        || fixture.source_fractions.len() > 128
        || fixture.queries.len() > 128
        || fixture
            .source_fractions
            .iter()
            .any(|&fraction| !fraction.is_finite() || !(0.0..=1.0).contains(&fraction))
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid curve offset geometry fixture",
        ));
    }
    let corner = match fixture.corner.as_str() {
        "Sharp" => CurveOffsetCornerStyle::Sharp,
        "Chamfer" => CurveOffsetCornerStyle::Chamfer,
        "Round" => CurveOffsetCornerStyle::Round,
        "None" => CurveOffsetCornerStyle::None,
        _ => {
            return Err(ProbeError::FixtureInvariant(
                "unknown curve offset corner style",
            ));
        }
    };
    let source = fixture.curve.geometry()?;
    let Curve3::NurbsCurve(_) = source else {
        return Err(ProbeError::FixtureInvariant(
            "curve offset geometry probe requires a NURBS source",
        ));
    };
    let normal = Vector3::try_from(fixture.normal)?.normalized(tolerance)?;
    let signed = source.offset_side(point(fixture.side)?, normal, tolerance)? * fixture.distance;
    let (outputs, elapsed) = measure(iterations, || {
        source.try_offset_parts(signed, normal, tolerance, corner)
    })?;
    let nearest = |query: Point3| -> Result<[f64; 3], ProbeError> {
        let mut best = None;
        for output in &outputs {
            let curve = output.as_ref();
            let parameter = curve.closest_parameter(query, tolerance)?;
            let location = curve.evaluate(parameter)?;
            let distance = query.distance_to(location)?;
            if best.is_none_or(|(previous, _)| distance < previous) {
                best = Some((distance, location));
            }
        }
        Ok(best
            .expect("an offset has at least one output")
            .1
            .to_array())
    };
    let samples = fixture
        .source_fractions
        .iter()
        .map(|&fraction| {
            nearest(
                source
                    .as_ref()
                    .evaluate(source.as_ref().parameter_at(fraction)?)?,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let queries = fixture
        .queries
        .iter()
        .map(|&query| nearest(point(query)?))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((
        json!({
            "closed": outputs.len() == 1 && outputs[0].as_ref().is_closed()?,
            "samples": samples,
            "queries": queries,
        }),
        elapsed,
    ))
}

#[cfg(test)]
mod tests {
    use crate::{ProbeRequest, ProbeResponse, run_request};

    #[test]
    fn curved_offset_fixture_samples_all_corner_styles() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/curve_offset_corners.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        assert_eq!(response.results.len(), 4);
        for result in &response.results {
            assert_eq!(result.value["samples"].as_array().unwrap().len(), 8);
            assert_eq!(result.value["queries"].as_array().unwrap().len(), 3);
        }
        let corner_points = response
            .results
            .iter()
            .map(|result| result.value["queries"][0].clone())
            .collect::<Vec<_>>();
        for (index, point) in corner_points.iter().enumerate() {
            assert!(!corner_points[..index].contains(point));
        }
    }

    #[test]
    fn curved_offset_corner_samples_match_saved_rhino() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/curve_offset_corners.json"
        ))
        .unwrap();
        let rhino: ProbeResponse = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/curve_offset_corners.json"
        ))
        .unwrap();
        let native = run_request(&request).unwrap();
        assert_eq!(native.results.len(), 4);
        for (actual, expected) in native.results.iter().zip(&rhino.results) {
            assert_eq!(actual.id, expected.id);
            assert_eq!(actual.value["closed"], expected.value["closed"]);
            for field in ["samples", "queries"] {
                let actual_points = actual.value[field].as_array().unwrap();
                let expected_points = expected.value[field].as_array().unwrap();
                assert_eq!(actual_points.len(), expected_points.len());
                for (actual_point, expected_point) in actual_points.iter().zip(expected_points) {
                    for coordinate in 0..3 {
                        let error = (actual_point[coordinate].as_f64().unwrap()
                            - expected_point[coordinate].as_f64().unwrap())
                        .abs();
                        assert!(error < 1e-7, "{} {field} error {error}", actual.id);
                    }
                }
            }
        }
    }
}
