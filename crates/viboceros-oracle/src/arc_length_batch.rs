//! Independent normalized-length batches through the shared prepared sampler.
use super::*;
use viboceros_geometry::ArcLengthSampler;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub curve: NurbsCurveDefinition,
    pub fractions: Vec<f64>,
}

pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let curve = nurbs_curve_from_definition(&f.curve)?;
        let mut sampler = ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), tolerance)?;
        sampler.prepare_budgeted_repeated_sampling(64)?;
        let parameters = sampler.parameters_at_normalized_lengths(&f.fractions)?;
        let points = parameters
            .iter()
            .map(|&t| curve.evaluate(t).map(|p| p.to_array()))
            .collect::<Result<Vec<_>, GeometryError>>()?;
        Ok(
            json!({"parameters":parameters,"points":points,"length":sampler.total_length(),"domain":[*curve.domain().start(),*curve.domain().end()]}),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arc_length_batches_match_native_geometry_and_preserve_the_stationary_analytic_inverse() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/arc_length_batch.json"
        ))
        .unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/arc_length_batch_rhino_20261016.json"
        ))
        .unwrap();
        let actual = run_request(&request).unwrap();
        assert_eq!(actual.results.len(), 14);
        assert_eq!(
            actual.results.len(),
            native["results"].as_array().unwrap().len()
        );
        let mut requests = 0;
        let mut parameter_mismatch_cases = 0;
        for ((a, n), op) in actual
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
            .zip(&request.operations)
        {
            assert_eq!(a.id, n["id"]);
            assert_eq!(a.value["domain"], n["value"]["domain"]);
            let Operation::ArcLengthBatch { fixture, .. } = op else {
                panic!("unexpected batch recipe")
            };
            requests += fixture.fractions.len();
            let x = a.value["parameters"].as_array().unwrap();
            let y = n["value"]["parameters"].as_array().unwrap();
            assert_eq!(x.len(), fixture.fractions.len());
            assert_eq!(x.len(), y.len());
            let stationary = a.id == "stationary-end";
            if stationary {
                assert_eq!(fixture.curve.degree, 2);
                assert!(
                    fixture
                        .curve
                        .control_points
                        .iter()
                        .zip([[0., 0., 0.], [0., 0., 0.], [1., 0., 0.]])
                        .all(|(p, q)| p.point == q && p.weight == 1.)
                );
            }
            parameter_mismatch_cases += usize::from(
                x.iter()
                    .zip(y)
                    .any(|(x, y)| (x.as_f64().unwrap() - y.as_f64().unwrap()).abs() > 1e-8),
            );
            for ((x, y), f) in x.iter().zip(y).zip(&fixture.fractions) {
                let expected = if stationary {
                    f.sqrt()
                } else {
                    y.as_f64().unwrap()
                };
                assert!(
                    (x.as_f64().unwrap() - expected).abs() <= 1e-8,
                    "{} parameter {f}: {x} != {expected}",
                    a.id
                );
            }
            let x = a.value["points"].as_array().unwrap();
            let y = n["value"]["points"].as_array().unwrap();
            assert_eq!(x.len(), fixture.fractions.len());
            assert_eq!(x.len(), y.len());
            for (x, y) in x.iter().zip(y) {
                for j in 0..3 {
                    assert!(
                        (x[j].as_f64().unwrap() - y[j].as_f64().unwrap()).abs()
                            <= if stationary { 1e-6 } else { 1e-8 },
                        "{} point: {x} != {y}",
                        a.id
                    );
                }
            }
            assert!(
                (a.value["length"].as_f64().unwrap() - n["value"]["length"].as_f64().unwrap())
                    .abs()
                    <= 1e-8,
                "{} length",
                a.id
            );
        }
        assert_eq!(requests, 1815);
        assert_eq!(parameter_mismatch_cases, 1);
    }
}
