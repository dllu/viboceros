//! Exact native-parameter rational surface partials and knot-line limits.

use super::{NurbsSurfaceDefinition, ProbeError, nurbs_surface_from_definition};
use serde::Deserialize;
use serde_json::{Value, json};
use viboceros_geometry::{
    AffineTransform3, GeometryError, ParameterSide, SurfaceCurvature, Vector3,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permanent_curvature_fixtures_cover_shape_operators_and_actual_marker_commands() {
        for (text, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/surface_curvature.json"),
                27,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/surface_curvature_umbilic.json"),
                1,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/curvature_command.json"),
                17,
            ),
        ] {
            let request: crate::ProbeRequest = serde_json::from_str(text).unwrap();
            let response = crate::run_request(&request).unwrap();
            assert_eq!(response.results.len(), count);
            if count == 1 {
                for sample in response.results[0].value["samples"].as_array().unwrap() {
                    for k in sample["principal"].as_array().unwrap() {
                        assert!((k.as_f64().unwrap() + 0.5).abs() < 1e-12);
                    }
                }
            }
        }
    }

    #[test]
    fn permanent_fixture_checks_second_partials_and_crossed_limits() {
        let request: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/surface_jets.json"
        ))
        .unwrap();
        let response = crate::run_request(&request).unwrap();
        assert_eq!(response.results.len(), 20);
        let first = &response.results[0].value["samples"][0];
        for (field, expected) in [
            ("duu", [-2.0, 0.0, 0.0]),
            ("duv", [-2.0, -1.0, 1.0]),
            ("dvv", [0.0, -4.0, 0.0]),
        ] {
            for (axis, expected) in expected.into_iter().enumerate() {
                assert!((first[field][axis].as_f64().unwrap() - expected).abs() < 2e-14);
            }
        }
        let limits = response
            .results
            .iter()
            .find(|r| r.id == "surface-jets-crossed-limits")
            .unwrap();
        let samples = limits.value["samples"].as_array().unwrap();
        assert_eq!(samples.len(), 12);
        assert_eq!(samples[0]["point"], samples[3]["point"]);
        assert_ne!(samples[0]["du"], samples[3]["du"]);
        assert_ne!(samples[0]["dv"], samples[3]["dv"]);
    }

    #[test]
    fn translated_fixture_has_identical_differentials_to_untranslated_controls() {
        let request: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/surface_translated_jets.json"
        ))
        .unwrap();
        for operation in &request.operations {
            let crate::Operation::SurfaceJets { fixture, .. } = operation else {
                panic!("surface jets required")
            };
            let (actual, _) = run(fixture, 1).unwrap();
            let mut base = fixture.clone();
            base.translation = None;
            let (expected, _) = run(&base, 1).unwrap();
            for (a, b) in actual["samples"]
                .as_array()
                .unwrap()
                .iter()
                .zip(expected["samples"].as_array().unwrap())
            {
                for field in ["du", "dv", "duu", "duv", "dvv"] {
                    assert_eq!(a[field], b[field]);
                }
                for (axis, offset) in fixture.translation.unwrap().into_iter().enumerate() {
                    assert_eq!(
                        a["point"][axis].as_f64().unwrap(),
                        b["point"][axis].as_f64().unwrap() + offset
                    );
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Left,
    #[default]
    Right,
}

impl Side {
    fn geometry(self) -> ParameterSide {
        match self {
            Self::Left => ParameterSide::Left,
            Self::Right => ParameterSide::Right,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Sample {
    pub parameter: [f64; 2],
    #[serde(default)]
    pub side_u: Side,
    #[serde(default)]
    pub side_v: Side,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SurfaceJetsFixture {
    pub surface: NurbsSurfaceDefinition,
    /// Omitted samples use a five-by-five grid including domain boundaries.
    #[serde(default)]
    pub samples: Option<Vec<Sample>>,
    #[serde(default)]
    pub reverse_u: bool,
    #[serde(default)]
    pub reverse_v: bool,
    #[serde(default)]
    pub swap_uv: bool,
    #[serde(default)]
    pub translation: Option<[f64; 3]>,
    #[serde(default)]
    pub extended: bool,
}

pub(super) fn run(
    fixture: &SurfaceJetsFixture,
    iterations: u32,
) -> Result<(Value, u64), ProbeError> {
    run_mode(fixture, iterations, false)
}

pub(super) fn run_curvature(
    fixture: &SurfaceJetsFixture,
    iterations: u32,
) -> Result<(Value, u64), ProbeError> {
    run_mode(fixture, iterations, true)
}

fn run_mode(
    fixture: &SurfaceJetsFixture,
    iterations: u32,
    curvature: bool,
) -> Result<(Value, u64), ProbeError> {
    let mut surface = nurbs_surface_from_definition(&fixture.surface)?;
    if fixture.reverse_u {
        surface = surface.try_reversed_u()?;
    }
    if fixture.reverse_v {
        surface = surface.try_reversed_v()?;
    }
    if fixture.swap_uv {
        surface = surface.try_swapped_uv()?;
    }
    if let Some(offset) = fixture.translation {
        surface = surface.transformed(AffineTransform3::from_translation(Vector3::try_from(
            offset,
        )?))?;
    }
    let domain_u = [*surface.domain_u().start(), *surface.domain_u().end()];
    let domain_v = [*surface.domain_v().start(), *surface.domain_v().end()];
    let samples = fixture.samples.clone().unwrap_or_else(|| {
        (0..=4)
            .flat_map(|j| {
                (0..=4).map(move |i| Sample {
                    parameter: [
                        domain_u[0] + (domain_u[1] - domain_u[0]) * (i as f64 / 4.0),
                        domain_v[0] + (domain_v[1] - domain_v[0]) * (j as f64 / 4.0),
                    ],
                    side_u: Side::Right,
                    side_v: Side::Right,
                })
            })
            .collect()
    });
    if samples.is_empty()
        || (fixture.extended
            && samples
                .iter()
                .any(|s| s.side_u == Side::Left || s.side_v == Side::Left))
    {
        return Err(ProbeError::FixtureInvariant(
            "surface jets need samples; continuation uses right sides",
        ));
    }
    let compute = || {
        samples
            .iter()
            .map(|sample| {
                let [u, v] = sample.parameter;
                if fixture.extended {
                    surface.evaluate_extended_with_second_derivatives(u, v)
                } else {
                    surface.evaluate_with_second_derivatives_on_sides(
                        u,
                        v,
                        sample.side_u.geometry(),
                        sample.side_v.geometry(),
                    )
                }
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let evaluate = || {
        let jets = compute()?;
        let curvatures = if curvature {
            Some(
                jets.iter()
                    .map(|j| match j.curvature() {
                        Ok(value) => Ok(Some(value)),
                        Err(GeometryError::Degenerate { .. }) => Ok(None),
                        Err(error) => Err(error),
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )
        } else {
            None
        };
        Ok::<_, GeometryError>((jets, curvatures))
    };
    let mut result = evaluate()?;
    let started = std::time::Instant::now();
    for _ in 0..iterations {
        result = std::hint::black_box(evaluate()?);
    }
    let elapsed =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    let (jets, curvatures) = result;
    let records = samples
        .iter()
        .zip(jets)
        .enumerate()
        .map(|(i, (sample, jet))| {
            if let Some(values) = &curvatures {
                let mut value = curvature_value(values[i])?;
                value["parameter"] = json!(sample.parameter);
                return Ok(value);
            }
            Ok(json!({
                "parameter": sample.parameter, "point": jet.point.to_array(),
                "du": jet.derivative_u.to_array(), "dv": jet.derivative_v.to_array(),
                "duu": jet.derivative_uu.to_array(), "duv": jet.derivative_uv.to_array(),
                "dvv": jet.derivative_vv.to_array(),
            }))
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    Ok((
        json!({"domain_u": domain_u, "domain_v": domain_v, "samples": records}),
        elapsed,
    ))
}

fn curvature_value(value: Option<SurfaceCurvature>) -> Result<Value, GeometryError> {
    let Some(c) = value else {
        return Ok(json!({"available":false}));
    };
    let directions = c.directions.map(|d| d.as_vector().to_array());
    let operator: [[f64; 3]; 3] = std::array::from_fn(|i| {
        std::array::from_fn(|j| {
            (0..2)
                .map(|k| c.principal[k] * directions[k][i] * directions[k][j])
                .sum()
        })
    });
    let mut principal = c.principal;
    principal.sort_by(|a, b| b.total_cmp(a));
    Ok(
        json!({"available":true,"point":c.point.to_array(),"normal":c.normal.as_vector().to_array(),
        "principal":principal,"mean":c.mean(),"gaussian":c.gaussian()?,"shape_operator":operator}),
    )
}
