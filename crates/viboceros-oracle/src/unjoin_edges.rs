//! Public edge-separation API on exactly shared source definitions.
use super::*;
#[cfg(test)]
mod tests;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UnjoinEdgesFixture {
    source: crate::brep_source::BrepSourceFixture,
    edges: Vec<usize>,
}
pub(super) fn run(
    f: &UnjoinEdgesFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = f.source.build(tolerance)?;
    if let Some(path) = &f.source.artifact_path {
        crate::brep_source::write_shared_artifact(
            &Geometry::Brep(source.clone()),
            path,
            tolerance,
        )?;
    }
    let parts = source.try_unjoin_edges(&f.edges, tolerance)?;
    let before = crate::brep_interchange::definition_record(&source)?;
    Ok((
        json!({"before":before,"working":before,"after":parts.iter().map(crate::brep_interchange::definition_record).collect::<Result<Vec<_>,_>>()?}),
        0,
    ))
}
