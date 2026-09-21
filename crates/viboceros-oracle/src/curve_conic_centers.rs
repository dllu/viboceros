//! Full-coordinate conic recognition, distinct from a constrained UI snap.
use super::{NurbsCurveDefinition, ProbeError, measure, nurbs_curve_from_definition};
use serde_json::{Value, json};
use viboceros_geometry::Tolerance;

pub(super) fn run(
    definition: &NurbsCurveDefinition,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let curve = nurbs_curve_from_definition(definition)?;
    let ((circle, ellipse), elapsed) = measure(iterations, || {
        Ok((
            curve.circular_center(tolerance)?,
            curve.elliptical_center(tolerance)?,
        ))
    })?;
    Ok((
        json!({
            "circle_center": circle.map(|p| p.to_array()),
            "ellipse_center": ellipse.map(|p| p.to_array()),
        }),
        elapsed,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProbeRequest, run_request};

    #[test]
    fn full_coordinate_conic_api_replay_retains_far_origin_and_short_arc_differences() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/conic_centers.json"
        ))
        .unwrap();
        let observed: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/conic_centers.json"
        ))
        .unwrap();
        let actual = run_request(&request).unwrap();
        assert_eq!(actual.results.len(), 32);
        assert_eq!(observed["results"].as_array().unwrap().len(), 32);
        let (mut matches, mut differences) = (0, 0);
        for (a, b) in actual
            .results
            .iter()
            .zip(observed["results"].as_array().unwrap())
        {
            assert_eq!(a.id, b["id"].as_str().unwrap());
            if a.id == "far-origin" {
                assert_eq!(
                    a.value["ellipse_center"],
                    json!([1e12 + 4., -1e12 - 4., 1e12])
                );
                assert!(b["value"]["ellipse_center"].is_null());
                assert!(
                    a.value["circle_center"].is_null() && b["value"]["circle_center"].is_null()
                );
                differences += 1;
            } else if ["short-arc-0.01", "short-arc-0.001"].contains(&a.id.as_str()) {
                let actual = a.value["ellipse_center"].as_array().unwrap();
                for (coordinate, expected) in actual.iter().zip([4., -4., 0.]) {
                    assert!((coordinate.as_f64().unwrap() - expected).abs() < 1e-9);
                }
                assert!((b["value"]["ellipse_center"][0].as_f64().unwrap() - 4.).abs() > 1e-7);
                assert!(
                    a.value["circle_center"].is_null() && b["value"]["circle_center"].is_null()
                );
                differences += 1;
            } else {
                // Absolute-only center epsilon: large origins must not widen
                // the permitted model-space error through a relative tolerance.
                crate::test_json::close(&a.value, &b["value"], &a.id, 1e-9, 0.);
                matches += 1;
            }
        }
        assert_eq!((matches, differences), (29, 3));
    }
}
