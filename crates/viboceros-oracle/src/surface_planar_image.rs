//! Local complete planar and ruled image bounds for supplied tensor surfaces.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub first: NurbsSurfaceDefinition,
    pub second: NurbsSurfaceDefinition,
    #[serde(default)]
    pub absolute_tolerance: Option<f64>,
}

pub(super) fn run(
    fixture: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    run_bound(
        fixture,
        iterations,
        tolerance,
        NurbsSurface::planar_surface_image_deviation_bound,
    )
}

pub(super) fn run_ruled(
    fixture: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let (mut value, elapsed) = run_bound(
        fixture,
        iterations,
        tolerance,
        NurbsSurface::ruled_surface_image_deviation_bound,
    )?;
    value["u_correspondence_certified"] = json!(false);
    Ok((value, elapsed))
}

fn run_bound(
    fixture: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
    method: fn(&NurbsSurface, &NurbsSurface, f64) -> Result<Option<f64>, GeometryError>,
) -> Result<(Value, u64), ProbeError> {
    let first = nurbs_surface_from_definition(&fixture.first)?;
    let second = nurbs_surface_from_definition(&fixture.second)?;
    let limit = fixture.absolute_tolerance.unwrap_or(tolerance.absolute());
    let build = || method(&first, &second, limit);
    let mut bound = build()?;
    let started = Instant::now();
    for _ in 0..iterations {
        bound = black_box(build()?);
    }
    let elapsed =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    Ok((
        json!({"maximum_deviation_bound":bound,"certified":bound.is_some()}),
        elapsed,
    ))
}
