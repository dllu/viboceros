//! Compare planar ellipse offsets at source-curve stations across curve types.

use super::{ProbeError, curve_join_close::CurveInput, measure, point};
use serde_json::{Value, json};
use std::f64::consts::TAU;
use viboceros_geometry::{Curve3, Tolerance};

pub(super) fn run(
    input: &CurveInput,
    side: [f64; 3],
    distance: f64,
    samples: usize,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if !distance.is_finite() || distance <= 0.0 || !(8..=513).contains(&samples) {
        return Err(ProbeError::FixtureInvariant(
            "invalid ellipse offset distance or sample count",
        ));
    }
    let Curve3::Ellipse(ellipse) = input.geometry()? else {
        return Err(ProbeError::FixtureInvariant(
            "ellipse offset probe requires an ellipse",
        ));
    };
    let source = Curve3::Ellipse(ellipse);
    let normal = ellipse.normal()?;
    let signed = source.offset_side(point(side)?, normal, tolerance)? * distance;
    let (output, elapsed) = measure(iterations, || source.try_offset(signed, normal, tolerance))?;
    let mut stations = Vec::with_capacity(samples);
    for index in 0..samples {
        let angle = TAU * index as f64 / (samples - 1) as f64;
        let closest = match &output {
            // The native offset preserves the source's angle domain. Rhino's
            // independently parameterized output is sampled by closest point.
            Curve3::Ellipse(curve) => curve.point_at_angle(angle)?,
            Curve3::NurbsCurve(curve) => curve.evaluate(angle)?,
            _ => {
                return Err(ProbeError::FixtureInvariant(
                    "ellipse offset returned an unexpected curve",
                ));
            }
        };
        stations.push(closest.to_array());
    }
    Ok((
        json!({"closed": output.as_ref().is_closed()?, "samples": stations}),
        elapsed,
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn ellipse_offset_fixture_samples_both_sides_without_parameter_assumptions() {
        let request: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/ellipse_offset.json"
        ))
        .unwrap();
        let response = crate::run_request(&request).unwrap();
        assert_eq!(response.results.len(), 4);
        for row in &response.results {
            assert_eq!(row.value["closed"], true);
            assert_eq!(row.value["samples"].as_array().unwrap().len(), 65);
        }
    }
}
