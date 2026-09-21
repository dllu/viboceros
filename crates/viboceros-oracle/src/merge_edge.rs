//! Selected-chain merging against the public BrepEdgeList.MergeEdge API.
use super::*;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MergeEdgeFixture {
    source: crate::brep_source::BrepSourceFixture,
    edge: usize,
    angle: f64,
}

pub(super) fn run(f: &MergeEdgeFixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    let source = f.source.build(tolerance)?;
    if let Some(path) = &f.source.artifact_path {
        crate::brep_source::write_shared_artifact(
            &Geometry::Brep(source.clone()),
            path,
            tolerance,
        )?;
    }
    let result = source.try_merge_edge(f.edge, f.angle, tolerance)?;
    let removed = source.edges().len() - result.edges().len();
    Ok((
        json!({
            "before": crate::brep_join::geometry_record(&source, tolerance)?,
            "after": crate::brep_join::geometry_record(&result, tolerance)?,
        "removed": removed,
        // Rhino's measured public return includes the seed, even for a no-op.
        // Keep it separate from the actual topology reduction.
        "api_return": removed + 1,
        }),
        0,
    ))
}
