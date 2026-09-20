//! Fractional curve samples compared to a unit-domain Rhino shape reference.
use super::{NurbsCurveDefinition, ProbeError, measure, nurbs_curve_from_definition};
use serde_json::{Value, json};
use viboceros_geometry::GeometryError;

pub(super) fn run(
    definition: &NurbsCurveDefinition,
    fractions: &[f64],
    iterations: u32,
) -> Result<(Value, u64), ProbeError> {
    let curve = nurbs_curve_from_definition(definition)?;
    let ((points, span_points), elapsed) = measure(iterations, || {
        let sampler = curve.parameter_sampler()?;
        let points = fractions
            .iter()
            .map(|t| sampler.evaluate(*t).map(|p| p.to_array()))
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let span_points = sampler
            .spans()
            .map(|span| {
                fractions
                    .iter()
                    .map(|t| span.evaluate(*t).map(|p| p.to_array()))
                    .collect::<Result<Vec<_>, GeometryError>>()
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        Ok((points, span_points))
    })?;
    Ok((
        json!({"domain":[*curve.domain().start(), *curve.domain().end()], "points":points, "span_points":span_points}),
        elapsed,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paired_curve_sampling_fixture_preserves_shape_and_native_domains() {
        let request: super::super::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/curve_parameter_samples.json"
        ))
        .unwrap();
        let response = super::super::run_request(&request).unwrap();
        assert_eq!(response.results.len(), 12);
        let definitions: Vec<_> = request
            .operations
            .iter()
            .map(|operation| match operation {
                super::super::Operation::NurbsCurveParameterSamples { curve, .. } => curve,
                _ => panic!("unexpected fixture operation"),
            })
            .collect();
        for family in definitions.chunks_exact(3) {
            for curve in &family[1..] {
                assert_eq!(curve.degree, family[0].degree);
                assert_eq!(curve.control_points, family[0].control_points);
                assert_eq!(curve.knots.len(), family[0].knots.len());
                let origin = curve.knots[curve.degree] - family[0].knots[curve.degree];
                for (actual, original) in curve.knots.iter().zip(&family[0].knots) {
                    assert_eq!(
                        actual - origin,
                        *original,
                        "fixture translation must be exact"
                    );
                }
            }
        }
        for (curve, record) in definitions.iter().zip(&response.results) {
            assert_eq!(
                record.value["domain"],
                json!([
                    curve.knots[curve.degree],
                    curve.knots[curve.control_points.len()],
                ])
            );
        }
        fn compare(a: &Value, b: &Value) {
            if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
                assert_eq!(a.len(), b.len());
                for (a, b) in a.iter().zip(b) {
                    compare(a, b);
                }
            } else {
                assert!(
                    (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() < 4e-15,
                    "{a} != {b}"
                );
            }
        }
        for family in response.results.chunks_exact(3) {
            let expected = &family[0].value;
            for record in &family[1..] {
                assert_ne!(record.value["domain"], expected["domain"]);
                for field in ["points", "span_points"] {
                    compare(&record.value[field], &expected[field]);
                }
            }
        }
    }
}
