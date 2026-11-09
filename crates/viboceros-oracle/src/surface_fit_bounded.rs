//! Automatic fitting with complete input displacement bounds, not SDK Fit policy.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub surface: NurbsSurfaceDefinition,
    pub degree_u: usize,
    pub degree_v: usize,
    #[serde(default = "maximum")]
    pub maximum_control_count: [usize; 2],
    #[serde(default)]
    pub absolute_tolerance: Option<f64>,
}
fn maximum() -> [usize; 2] {
    [64; 2]
}

pub(super) fn run(
    fixture: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    run_with_metric(fixture, iterations, tolerance, false)
}

pub(super) fn run_image(
    fixture: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    run_with_metric(fixture, iterations, tolerance, true)
}

fn run_with_metric(
    fixture: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
    image: bool,
) -> Result<(Value, u64), ProbeError> {
    let source = nurbs_surface_from_definition(&fixture.surface)?;
    let options = viboceros_geometry::SurfaceFitOptions {
        degree: [fixture.degree_u, fixture.degree_v],
        maximum_control_count: fixture.maximum_control_count,
    };
    let limit = fixture.absolute_tolerance.unwrap_or(tolerance.absolute());
    let build = || -> Result<Option<(NurbsSurface, f64)>, ProbeError> {
        #[cfg(feature = "native-smlib")]
        {
            if image {
                viboceros_smlib::fit_surface_image_with_bound(&source, options, limit)
                    .map(|r| r.map(|r| (r.surface, r.maximum_deviation_bound)))
            } else {
                viboceros_smlib::fit_surface_with_bound(&source, options, limit)
                    .map(|r| r.map(|r| (r.surface, r.maximum_deviation_bound)))
            }
            .map_err(CommandError::from)
            .map_err(ProbeError::from)
        }
        #[cfg(not(feature = "native-smlib"))]
        {
            if image {
                Ok(
                    viboceros_geometry::try_fit_surface_image_with_bound(&source, options, limit)?
                        .map(|r| (r.surface, r.maximum_deviation_bound)),
                )
            } else {
                Ok(
                    viboceros_geometry::try_fit_surface_with_bound(&source, options, limit)?
                        .map(|r| (r.surface, r.maximum_deviation_bound)),
                )
            }
        }
    };
    let mut result = build()?;
    let started = Instant::now();
    for _ in 0..iterations {
        result = black_box(build()?);
    }
    let elapsed =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    Ok((
        json!({"surface":result.as_ref().map(|r|nurbs_surface_definition_value(&r.0)),
        "maximum_deviation_bound":result.as_ref().map(|r|r.1),
        "before":nurbs_surface_definition_value(&source),"after":nurbs_surface_definition_value(&source)}),
        elapsed,
    ))
}
