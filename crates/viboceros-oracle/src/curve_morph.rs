//! Separate exact point-map agreement from independently fitted curve error.

use super::{
    NurbsCurveDefinition, NurbsSurfaceDefinition, ProbeError, nurbs_curve_from_definition,
    nurbs_surface_from_definition,
};
use serde::Deserialize;
use serde_json::{Value, json};
use viboceros_geometry::{Frame3, Point3, PointMorph, SurfacePointMorph, Tolerance, Vector3};

#[cfg(test)]
mod tests {
    #[test]
    fn permanent_fixtures_check_native_fits_against_the_direct_point_map() {
        for (fixture, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/curve_surface_morph.json"),
                8,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/curve_rational_morph.json"),
                7,
            ),
        ] {
            let request: crate::ProbeRequest = serde_json::from_str(fixture).unwrap();
            let response = crate::run_request(&request).unwrap();
            assert_eq!(response.results.len(), count);
            for result in response.results {
                assert_eq!(result.value["exact_samples"].as_array().unwrap().len(), 257);
                assert_eq!(
                    result.value["fitted_samples"].as_array().unwrap().len(),
                    257
                );
            }
        }
    }

    #[test]
    fn surface_orient_command_records_unrounded_geometry_and_document_state() {
        let request: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/surface_orient.json"
        ))
        .unwrap();
        let response = crate::run_request(&request).unwrap();
        let scenarios = response.results[0].value.as_object().unwrap();
        assert_eq!(scenarios.len(), 6);
        for value in scenarios.values() {
            assert_eq!(value["command_succeeded"], true);
            for curve in value["objects"].as_array().unwrap() {
                assert!(curve.get("controls").is_none());
                assert_eq!(curve["samples"].as_array().unwrap().len(), 257);
                assert!(curve.get("domain").is_some());
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CurveMorphFixture {
    pub curve: NurbsCurveDefinition,
    pub surface: NurbsSurfaceDefinition,
    pub source_origin: [f64; 3],
    pub source_x: [f64; 3],
    pub source_y: [f64; 3],
    pub uv: [f64; 2],
    pub scale: f64,
    pub angle: f64,
    #[serde(default)]
    pub fit_tolerance: Option<f64>,
}

pub(super) fn run(
    fixture: &CurveMorphFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let tolerance = Tolerance::try_new(
        fixture.fit_tolerance.unwrap_or(tolerance.absolute()),
        tolerance.relative(),
        tolerance.angular(),
    )?;
    let source = nurbs_curve_from_definition(&fixture.curve)?;
    let surface = nurbs_surface_from_definition(&fixture.surface)?;
    let frame = Frame3::try_from_directions(
        Point3::try_from(fixture.source_origin)?,
        Vector3::try_from(fixture.source_x)?,
        Vector3::try_from(fixture.source_y)?,
        tolerance,
    )?;
    let morph = SurfacePointMorph::try_new(
        frame,
        &surface,
        fixture.uv[0],
        fixture.uv[1],
        fixture.scale,
        fixture.angle,
        false,
        tolerance,
    )?;
    let parameters = (0..=256)
        .map(|i| source.parameter_at(i as f64 / 256.0))
        .collect::<Result<Vec<_>, _>>()?;
    let exact = parameters
        .iter()
        .map(|t| morph.morph_point(source.evaluate(*t)?))
        .collect::<Result<Vec<_>, _>>()?;
    let mut fitted = morph.morph_nurbs_curve(&source, tolerance)?;
    let started = std::time::Instant::now();
    for _ in 0..iterations {
        fitted = std::hint::black_box(morph.morph_nurbs_curve(&source, tolerance)?);
    }
    let elapsed =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    let samples = parameters
        .iter()
        .map(|t| fitted.evaluate(*t))
        .collect::<Result<Vec<_>, _>>()?;
    for (actual, target) in samples.iter().zip(&exact) {
        if actual.distance_to(*target)? > tolerance.absolute() {
            return Err(ProbeError::FixtureInvariant(
                "native curve morph exceeds independent sampled fit tolerance",
            ));
        }
    }
    Ok((
        json!({
            "domain": [*fitted.domain().start(), *fitted.domain().end()],
            "exact_samples": exact.iter().map(|p| p.to_array()).collect::<Vec<_>>(),
            "fitted_samples": samples.iter().map(|p| p.to_array()).collect::<Vec<_>>(),
        }),
        elapsed,
    ))
}
