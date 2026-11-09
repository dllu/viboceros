//! Complete bounds with supplied separable normalized U/V correspondences.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub first: NurbsSurfaceDefinition,
    pub second: NurbsSurfaceDefinition,
    pub correspondence: [Vec<[f64; 2]>; 2],
    #[serde(default)]
    pub absolute_tolerance: Option<f64>,
}

pub(super) fn run(
    fixture: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let first = nurbs_surface_from_definition(&fixture.first)?;
    let second = nurbs_surface_from_definition(&fixture.second)?;
    let limit = fixture.absolute_tolerance.unwrap_or(tolerance.absolute());
    let build = || {
        first.mapped_surface_deviation_bound(
            &second,
            fixture.correspondence.each_ref().map(Vec::as_slice),
            limit,
        )
    };
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
