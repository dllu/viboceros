//! Trim-aware isocurve probes with parameter-domain-independent sampling.
use super::{ProbeError, TrimmedBrepFixture, measure, trimmed_brep};
use serde_json::Value;
use viboceros_geometry::{GeometryError, NurbsCurve, Tolerance};

pub(super) fn run(
    fixture: &TrimmedBrepFixture,
    parameters: &[[f64; 2]],
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if parameters.is_empty() || parameters.len() > 64 {
        return Err(ProbeError::FixtureInvariant(
            "isocurve probes require 1..=64 UV pairs",
        ));
    }
    let brep = trimmed_brep::build(fixture, tolerance)?;
    let (records, elapsed) = measure(iterations, || {
        brep.faces()
            .iter()
            .map(|face| {
                parameters
                    .iter()
                    .map(|&[u, v]| {
                        Ok([
                            sample(face.isocurve_u_segments(v, tolerance)?)?,
                            sample(face.isocurve_v_segments(u, tolerance)?)?,
                        ])
                    })
                    .collect::<Result<Vec<_>, GeometryError>>()
            })
            .collect::<Result<Vec<_>, GeometryError>>()
    })?;
    Ok((serde_json::to_value(records)?, elapsed))
}

fn sample(curves: Vec<NurbsCurve>) -> Result<Vec<Vec<[f64; 3]>>, GeometryError> {
    let mut records = Vec::with_capacity(curves.len());
    for curve in curves {
        // This changes only the output curve's parameterization. Extraction
        // is already complete; it cannot recover rounded trim intersections.
        let curve = curve.try_reparameterized(0.0..=1.0)?;
        let mut points = [0., 0.125, 0.3, 0.5, 0.875, 1.]
            .into_iter()
            .map(|t| curve.evaluate(t).map(|p| p.to_array()))
            .collect::<Result<Vec<_>, _>>()?;
        if points.first() > points.last() {
            // Resample backwards instead of reversing a nonsymmetric sample
            // sequence: the records always use the same normalized stations.
            points = [0., 0.125, 0.3, 0.5, 0.875, 1.]
                .into_iter()
                .map(|t| curve.evaluate(1. - t).map(|p| p.to_array()))
                .collect::<Result<Vec<_>, _>>()?;
        }
        records.push(points);
    }
    records.sort_by(|a, b| a.partial_cmp(b).expect("evaluated geometry is finite"));
    Ok(records)
}

#[cfg(test)]
mod tests {
    use crate::{ProbeRequest, run_request};

    #[test]
    fn trimmed_isocurve_fixtures_match_analytic_paraboloid_segments() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/brep_isocurve_frames.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        assert_eq!(response.results.len(), 6);
        for result in response.results {
            let annulus = result.id.contains("annulus");
            let queries = result.value[0].as_array().unwrap();
            assert_eq!(queries.len(), 4);
            for (query, [u, v]) in
                queries
                    .iter()
                    .zip([[0.125, 0.375], [0.375, 0.125], [0., 0.], [0.75, 0.75]])
            {
                for (axis, fixed) in [v, u].into_iter().enumerate() {
                    let segments = query[axis].as_array().unwrap();
                    if fixed > 0.5 {
                        assert!(segments.is_empty());
                        continue;
                    }
                    let outer = (0.25_f64 - fixed * fixed).sqrt();
                    let intervals = if annulus && fixed < 0.25 {
                        let inner = (0.0625 - fixed * fixed).sqrt();
                        vec![[-outer, -inner], [inner, outer]]
                    } else {
                        vec![[-outer, outer]]
                    };
                    assert_eq!(segments.len(), intervals.len());
                    for (segment, [start, end]) in segments.iter().zip(intervals) {
                        for (sample, t) in segment
                            .as_array()
                            .unwrap()
                            .iter()
                            .zip([0., 0.125, 0.3, 0.5, 0.875, 1.])
                        {
                            let mut expected = [fixed, fixed, 0.];
                            expected[axis] = start * (1. - t) + end * t;
                            expected[2] = expected[0].powi(2) + expected[1].powi(2);
                            for i in 0..3 {
                                assert!(
                                    (sample[i].as_f64().unwrap() - expected[i]).abs() < 2e-12,
                                    "{}: {sample} != {expected:?}",
                                    result.id
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
