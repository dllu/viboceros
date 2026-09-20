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
        let request = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/surface-closest-point.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        let reference: Value = serde_json::from_str(include_str!(
            "../../../docs/surface-closest-point-rhino-reference.json"
        ))
        .unwrap();
        let expected = reference["results"].as_array().unwrap();
        assert_eq!(response.results.len(), 6);
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
                            1e-8 * widths[axis] + 16. * f64::EPSILON * a.abs().max(b.abs())
                        } else {
                            1e-8
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
                        <= 1e-8
                );
                count += 1;
            }
        }
        assert_eq!(count, 17);
    }
}
