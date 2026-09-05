//! Cross-engine checks for exact piecewise curves and their parameter maps.

use serde::Deserialize;
use serde_json::{Value, json};
use viboceros_geometry::{CurveRef, GeometryError, ParameterSide, PolyCurve3, Tolerance};

use super::{
    NurbsCurveDefinition, ProbeError, measure, nurbs_curve_definition_value,
    nurbs_curve_from_definition,
};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PolyCurveFixture {
    pub segments: Vec<NurbsCurveDefinition>,
    #[serde(default)]
    pub domain: Option<[f64; 2]>,
    #[serde(default)]
    pub reversed: bool,
    #[serde(default)]
    pub trim: Option<[f64; 2]>,
    #[serde(default)]
    pub split: Option<f64>,
}

pub(super) fn run(
    fixture: &PolyCurveFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = PolyCurve3::try_new(
        fixture
            .segments
            .iter()
            .map(nurbs_curve_from_definition)
            .collect::<Result<_, _>>()?,
    )?;
    measure(iterations, || {
        let mut curve = source.clone();
        if let Some([start, end]) = fixture.domain {
            curve = curve.try_reparameterized_by_length(start..=end, tolerance)?;
        }
        if fixture.reversed {
            curve = curve.reversed()?;
        }
        if let Some([start, end]) = fixture.trim {
            curve = curve.try_trimmed(start..=end)?;
        }
        let curves = if let Some(parameter) = fixture.split {
            let (first, second) = curve.try_split(parameter)?;
            vec![first, second]
        } else {
            vec![curve]
        };
        Ok(
            json!({"curves": curves.iter().map(|curve| record(curve, tolerance)).collect::<Result<Vec<_>, GeometryError>>()?}),
        )
    })
}

fn record(curve: &PolyCurve3, tolerance: Tolerance) -> Result<Value, GeometryError> {
    let nurbs = curve.to_nurbs()?;
    let mut parameters = (0..=32)
        .map(|i| curve.parameter_at(i as f64 / 32.0))
        .collect::<Result<Vec<_>, _>>()?;
    parameters.extend_from_slice(curve.parameters());
    parameters.sort_by(f64::total_cmp);
    parameters.dedup();
    let samples = parameters.iter().map(|&parameter| {
        let (point, first, second) = curve.evaluate_with_second_derivative(parameter, ParameterSide::Right)?;
        if point.distance_to(nurbs.evaluate(parameter)?)? > 1e-9 {
            return Err(GeometryError::InvalidPolyCurve { context: "polycurve NURBS conversion changed the curve" });
        }
        Ok(json!({"parameter":parameter,"point":point.to_array(),"first":first.to_array(),"second":second.to_array()}))
    }).collect::<Result<Vec<_>, GeometryError>>()?;
    let segments = curve
        .segments()
        .iter()
        .enumerate()
        .map(|(index, segment)| {
            Ok(nurbs_curve_definition_value(
                &segment
                    .try_reparameterized(curve.segment_domain(index)?)?
                    .to_nurbs()?,
            ))
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    Ok(json!({
        "domain":[*curve.domain().start(),*curve.domain().end()],
        "segment_domains":curve.parameters().windows(2).collect::<Vec<_>>(),
        "segments":segments, "samples":samples,
        "closed":curve.is_closed()?, "length":curve.length(tolerance)?,
        "division_count_without_ends":CurveRef::PolyCurve(curve).divide_by_count(17, false, tolerance)?.len(),
        "division_points":CurveRef::PolyCurve(curve).divide_by_count(17, true, tolerance)?.iter().map(|p| p.to_array()).collect::<Vec<_>>()
    }))
}

#[cfg(test)]
mod tests {
    use crate::{ProbeRequest, run_request};

    #[test]
    fn permanent_fixture_checks_exact_mixed_segments_and_closed_division_topology() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/polycurve.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        assert_eq!(response.results.len(), 14);
        let natural = &response.results[0].value["curves"][0];
        assert!(
            (natural["length"].as_f64().unwrap() - 3.0 - std::f64::consts::FRAC_PI_2).abs() < 1e-12
        );
        assert_eq!(natural["segments"].as_array().unwrap().len(), 3);
        assert_eq!(natural["division_points"].as_array().unwrap().len(), 18);
        assert_eq!(natural["division_count_without_ends"], 16);
        let closed = &response.results[9].value["curves"][0];
        assert_eq!(closed["closed"], true);
        assert_eq!(closed["division_points"].as_array().unwrap().len(), 17);
        assert_eq!(closed["division_count_without_ends"], 16);
        assert_eq!(
            response.results[6].value["curves"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn division_contract_fixture_checks_both_endpoint_flags_and_single_divisions() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/curve_division_contract.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        let expected = [
            vec![
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [4.0, 0.0, 0.0],
                [6.0, 0.0, 0.0],
                [8.0, 0.0, 0.0],
            ],
            vec![[2.0, 0.0, 0.0], [4.0, 0.0, 0.0], [6.0, 0.0, 0.0]],
            vec![
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [2.0, 2.0, 0.0],
                [0.0, 2.0, 0.0],
            ],
            vec![[2.0, 0.0, 0.0], [2.0, 2.0, 0.0], [0.0, 2.0, 0.0]],
            vec![],
            vec![[0.0, 0.0, 0.0]],
        ];
        assert_eq!(response.results.len(), expected.len());
        for (result, expected) in response.results.iter().zip(expected) {
            let actual = result.value.as_array().unwrap();
            assert_eq!(actual.len(), expected.len(), "{}", result.id);
            for (point, expected) in actual.iter().zip(expected) {
                for (coordinate, expected) in point.as_array().unwrap().iter().zip(expected) {
                    assert!((coordinate.as_f64().unwrap() - expected).abs() < 1e-12);
                }
            }
        }
    }
}
