//! Basis-complexity instrumentation for the opt-in fitting proposal policy.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub surface: NurbsSurfaceDefinition,
    pub degree_u: usize,
    pub degree_v: usize,
    #[serde(default)]
    pub candidate_controls: Option<[usize; 2]>,
}

pub(super) fn run(fixture: &Fixture, iterations: u32) -> Result<(Value, u64), ProbeError> {
    let source = nurbs_surface_from_definition(&fixture.surface)?;
    let build = || {
        viboceros_geometry::SurfaceFitRefinementBudget::try_from_surface(
            &source,
            [fixture.degree_u, fixture.degree_v],
        )
    };
    let mut budget = build()?;
    let started = Instant::now();
    for _ in 0..iterations {
        budget = black_box(build()?);
    }
    let elapsed =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    Ok((
        json!({"supported":budget.is_some(),
        "initial_controls":budget.map(|b|b.initial_control_count()),
        "maximum_controls":budget.map(|b|b.maximum_control_count()),
        "candidate_admitted":budget.and_then(|b|fixture.candidate_controls.map(|n|b.admits_control_count(n)))}),
        elapsed,
    ))
}
