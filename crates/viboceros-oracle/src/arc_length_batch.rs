//! Independent normalized-length batches through the shared prepared sampler.
use super::*;
use viboceros_geometry::ArcLengthSampler;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub curve: NurbsCurveDefinition,
    pub fractions: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct QuadratureFixture {
    pub curve: NurbsCurveDefinition,
    pub fractions: Vec<f64>,
    /// Restrict lengths to this source-parameter interval, without changing
    /// the source definition or the domain of returned parameters.
    #[serde(default)]
    pub subdomain: Option<[f64; 2]>,
    #[serde(default = "fit_clock_tolerance")]
    pub fractional_tolerance: f64,
}

fn fit_clock_tolerance() -> f64 {
    1e-4
}

pub(super) fn quadrature(
    f: &QuadratureFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if f.fractions.len() > 4096 {
        return Err(ProbeError::FixtureInvariant(
            "length parameters accept at most 4096 fractions",
        ));
    }
    if !f.fractional_tolerance.is_finite()
        || f.fractional_tolerance <= 0.
        || f.fractional_tolerance >= 1.
    {
        return Err(GeometryError::InvalidArcLengthIntegrationTolerance.into());
    }
    let curve = nurbs_curve_from_definition(&f.curve)?;
    let trimmed = match f.subdomain {
        Some([start, end]) => {
            if !start.is_finite()
                || !end.is_finite()
                || start >= end
                || !curve.domain().contains(&start)
                || !curve.domain().contains(&end)
            {
                return Err(ProbeError::FixtureInvariant(
                    "invalid scalar length subdomain",
                ));
            }
            Some(curve.try_trimmed(start..=end)?)
        }
        None => None,
    };
    let active = trimmed.as_ref().unwrap_or(&curve);
    let sampler = ArcLengthSampler::try_new(CurveRef::NurbsCurve(active), tolerance)?;
    let before = nurbs_curve_definition_value(&curve);
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let parameters = sampler
            .parameters_at_quadrature_normalized_lengths(&f.fractions, f.fractional_tolerance)?;
        let points = parameters
            .iter()
            .map(|&t| curve.evaluate(t).map(Point3::to_array))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(
            json!({"parameters":parameters,"points":points,"length":sampler.quadrature_length_estimate(f.fractional_tolerance)?,
            "domain":[*active.domain().start(),*active.domain().end()],"before":before,"after":nurbs_curve_definition_value(&curve)}),
        )
    })
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
