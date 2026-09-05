//! Surface fitting is compared independently from the exact point map.

use super::{NurbsSurfaceDefinition, ProbeError, nurbs_surface_from_definition};
use serde::Deserialize;
use serde_json::{Value, json};
use viboceros_geometry::{Frame3, Point3, PointMorph, SurfacePointMorph, Tolerance, Vector3};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SurfaceMorphFixture {
    pub source: NurbsSurfaceDefinition,
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
    fixture: &SurfaceMorphFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let tolerance = Tolerance::try_new(
        fixture.fit_tolerance.unwrap_or(tolerance.absolute()),
        tolerance.relative(),
        tolerance.angular(),
    )?;
    let source = nurbs_surface_from_definition(&fixture.source)?;
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
    let mut parameters = Vec::with_capacity(33 * 33);
    // An offset interior grid avoids coinciding with all dyadic Greville rows.
    let fraction = |i| match i {
        0 => 0.0,
        32 => 1.0,
        _ => (i as f64 - 0.3819660112501051) / 32.0,
    };
    for j in 0..=32 {
        for i in 0..=32 {
            parameters.push([
                source.parameter_at_u(fraction(i))?,
                source.parameter_at_v(fraction(j))?,
            ]);
        }
    }
    let exact = parameters
        .iter()
        .map(|&[u, v]| morph.morph_point(source.evaluate(u, v)?))
        .collect::<Result<Vec<_>, _>>()?;
    let mut fitted = morph.morph_nurbs_surface(&source, tolerance)?;
    let started = std::time::Instant::now();
    for _ in 0..iterations {
        fitted = std::hint::black_box(morph.morph_nurbs_surface(&source, tolerance)?);
    }
    let elapsed =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    let samples = parameters
        .iter()
        .map(|&[u, v]| fitted.evaluate(u, v))
        .collect::<Result<Vec<_>, _>>()?;
    for (actual, target) in samples.iter().zip(&exact) {
        if actual.distance_to(*target)? > tolerance.absolute() {
            return Err(ProbeError::FixtureInvariant(
                "native surface morph exceeds independent sampled fit tolerance",
            ));
        }
    }
    Ok((
        json!({
            "domain_u": [*fitted.domain_u().start(), *fitted.domain_u().end()],
            "domain_v": [*fitted.domain_v().start(), *fitted.domain_v().end()],
            "exact_samples": exact.iter().map(|p| p.to_array()).collect::<Vec<_>>(),
            "fitted_samples": samples.iter().map(|p| p.to_array()).collect::<Vec<_>>(),
        }),
        elapsed,
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn permanent_fixture_checks_native_fits_against_the_direct_point_map() {
        let request: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/surface_surface_morph.json"
        ))
        .unwrap();
        let response = crate::run_request(&request).unwrap();
        assert_eq!(response.results.len(), 5);
        for result in response.results {
            assert_eq!(
                result.value["exact_samples"].as_array().unwrap().len(),
                1089
            );
            assert_eq!(
                result.value["fitted_samples"].as_array().unwrap().len(),
                1089
            );
        }
    }
}
