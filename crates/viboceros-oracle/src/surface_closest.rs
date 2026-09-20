//! Closest-point probes time only the search, not construction or result formatting.
use super::*;

pub(super) fn run(
    definition: &NurbsSurfaceDefinition,
    queries: &[[f64; 3]],
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let surface = nurbs_surface_from_definition(definition)?;
    let queries = queries
        .iter()
        .copied()
        .map(Point3::try_from)
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let (parameters, elapsed) = measure(iterations, || {
        queries
            .iter()
            .map(|&target| surface.closest_parameters(black_box(target), tolerance))
            .collect::<Result<Vec<_>, GeometryError>>()
    })?;
    let results = queries
        .iter()
        .zip(parameters)
        .map(|(target, (u, v))| {
            let point = surface.evaluate(u, v)?;
            Ok(json!({
                "parameters": [u,v], "normalized_parameters": surface.normalized_parameters(u,v)?,
                "point": point.to_array(), "distance": point.distance_to(*target)?,
            }))
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    Ok((json!(results), elapsed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closest_surface_fixture_matches_recorded_rhino_parameters_points_and_distances() {
        check_fixture(
            include_str!("../../../tools/rhino_oracle/fixtures/surface-closest-point.json"),
            include_str!("../../../docs/surface-closest-point-rhino-reference.json"),
            1e-8,
            17,
        );
    }

    #[test]
    fn closest_surface_curvature_fixture_matches_recorded_rhino() {
        // Rhino's upper-edge paraboloid result differs from the independently
        // analytic minimum by 8.71e-8. Keep the native analytic tests at 1e-8,
        // but allow 1e-7 for this observed oracle discrepancy.
        check_fixture(
            include_str!("../../../tools/rhino_oracle/fixtures/surface-closest-curvature.json"),
            include_str!("../../../docs/surface-closest-curvature-rhino-reference.json"),
            1e-7,
            21,
        );
    }

    #[test]
    fn closest_signed_surface_fixture_matches_recorded_rhino() {
        // One signed Bernstein coefficient, but an independently certified
        // positive denominator. Include endpoints and off-surface targets.
        check_fixture(
            include_str!("../../../tools/rhino_oracle/fixtures/surface-closest-signed.json"),
            include_str!("../../../docs/surface-closest-signed-rhino-reference.json"),
            1e-9,
            5,
        );
    }

    #[test]
    fn closest_candidate_fixture_matches_model_points_with_a_recorded_parameter_discrepancy() {
        let request = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/surface-closest-candidates.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        let reference: Value = serde_json::from_str(include_str!(
            "../../../docs/surface-candidates-rhino-reference.json"
        ))
        .unwrap();
        assert_eq!(response.results.len(), 2);
        let x = 1e16 + 2.;
        for (index, point) in [[x + 2., 0., 0.], [0.5, 0.25, 1.]].into_iter().enumerate() {
            let actual = &response.results[index];
            let expected = &reference["results"][index];
            assert_eq!(actual.id, expected["id"].as_str().unwrap());
            assert_eq!(actual.value[0]["point"], json!(point));
            assert_eq!(actual.value[0]["point"], expected["value"][0]["point"]);
            assert_eq!(
                actual.value[0]["distance"],
                expected["value"][0]["distance"]
            );
        }
        let first = &response.results[0].value[0];
        assert_eq!(first["distance"], json!(0.));
        let (u, v) = (
            first["parameters"][0].as_f64().unwrap(),
            first["parameters"][1].as_f64().unwrap(),
        );
        assert!(u > 0. && u < 1.);
        assert_eq!(v, 0.);
        let Operation::SurfaceClosestPoint { surface, .. } = &request.operations[0] else {
            panic!("surface closest-point operation expected")
        };
        let surface = nurbs_surface_from_definition(surface).unwrap();
        assert_eq!(surface.evaluate(u, v).unwrap().to_array(), [x + 2., 0., 0.]);
        assert_eq!(surface.evaluate(0., 0.).unwrap().to_array(), [x, 0., 0.]);
        // Rhino reports the target at U=0; the stored native controls evaluate
        // differently there. Keep this discrepancy explicit, not a widened UV
        // tolerance or a claim that the full response comparison passes.
        assert_eq!(
            reference["results"][0]["value"][0]["parameters"],
            json!([0., 0.])
        );
        assert_ne!(
            first["parameters"],
            reference["results"][0]["value"][0]["parameters"]
        );
        assert_eq!(
            response.results[1].value[0]["parameters"],
            json!([0.5, 0.5])
        );
        assert_eq!(
            response.results[1].value[0]["parameters"],
            reference["results"][1]["value"][0]["parameters"]
        );
    }

    fn check_fixture(request: &str, reference: &str, epsilon: f64, expected_count: usize) {
        let request = serde_json::from_str(request).unwrap();
        let response = run_request(&request).unwrap();
        let reference: Value = serde_json::from_str(reference).unwrap();
        let expected = reference["results"].as_array().unwrap();
        assert_eq!(response.results.len(), request.operations.len());
        assert_eq!(response.results.len(), expected.len());
        let mut count = 0;
        for ((actual, expected), operation) in response
            .results
            .iter()
            .zip(expected)
            .zip(&request.operations)
        {
            assert_eq!(actual.id, expected["id"].as_str().unwrap());
            let Operation::SurfaceClosestPoint { surface, .. } = operation else {
                panic!("closest-point operation expected")
            };
            let surface = nurbs_surface_from_definition(surface).unwrap();
            let widths = [surface.domain_u(), surface.domain_v()].map(|d| *d.end() - *d.start());
            let results = actual.value.as_array().unwrap();
            let records = expected["value"].as_array().unwrap();
            assert_eq!(results.len(), records.len());
            for (a, b) in results.iter().zip(records) {
                for (field, size) in [
                    ("point", 3),
                    ("parameters", 2),
                    ("normalized_parameters", 2),
                ] {
                    let a = a[field].as_array().unwrap();
                    let b = b[field].as_array().unwrap();
                    assert_eq!(a.len(), size);
                    assert_eq!(b.len(), size);
                    for (axis, (a, b)) in a.iter().zip(b).enumerate() {
                        let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
                        let epsilon = if field == "parameters" {
                            epsilon * widths[axis] + 16. * f64::EPSILON * a.abs().max(b.abs())
                        } else {
                            epsilon
                        };
                        assert!(
                            (a - b).abs() <= epsilon,
                            "{} {field}[{axis}]: {a} != {b}",
                            actual.id
                        );
                    }
                }
                assert!(
                    (a["distance"].as_f64().unwrap() - b["distance"].as_f64().unwrap()).abs()
                        <= epsilon
                );
                count += 1;
            }
        }
        assert_eq!(count, expected_count);
    }
}
