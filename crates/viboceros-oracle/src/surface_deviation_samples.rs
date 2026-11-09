//! Local sampled diagnostics, separate from complete certified bounds.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub first: NurbsSurfaceDefinition,
    pub second: NurbsSurfaceDefinition,
    #[serde(default = "divisions")]
    pub divisions: [usize; 2],
}
fn divisions() -> [usize; 2] {
    [16; 2]
}

/// JSON encoding of diagnostic point-pair witnesses, never accuracy admission.
fn sample_surface_deviation(
    first: &NurbsSurface,
    second: &NurbsSurface,
    divisions: [usize; 2],
    tolerance: Tolerance,
) -> Result<Value, GeometryError> {
    let result = first.deviation_samples(second, divisions, tolerance)?;
    let value = |summary: viboceros_geometry::SurfaceDeviationSampleSummary| {
        let p = summary.maximum;
        json!({"maximum_witness_distance":p.distance,"sample_count":summary.sample_count,
            "refinement_failures":summary.refinement_failures,
            "witness":{"distance":p.distance,"source_parameters":p.source_parameters,"target_parameters":p.target_parameters,
                "source_point":p.source_point.to_array(),"target_point":p.target_point.to_array()}})
    };
    Ok(
        json!({"certified":false,"divisions":divisions,"same_parameters":value(result.same_parameters),
        "first_to_second":value(result.first_to_second),"second_to_first":value(result.second_to_first)}),
    )
}

pub(super) fn run(
    fixture: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let a = nurbs_surface_from_definition(&fixture.first)?;
    let b = nurbs_surface_from_definition(&fixture.second)?;
    let build = || sample_surface_deviation(&a, &b, fixture.divisions, tolerance);
    let mut value = build()?;
    let start = Instant::now();
    for _ in 0..iterations {
        value = black_box(build()?);
    }
    let elapsed =
        u64::try_from(start.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    Ok((value, elapsed))
}
