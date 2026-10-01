//! Public hole-removal API replay on exact, shared source definitions.
use super::*;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct RemoveHolesFixture {
    source: crate::brep_source::BrepSourceFixture,
    loops: Option<Vec<(usize, usize)>>,
}

pub(super) fn run(
    f: &RemoveHolesFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if f.loops.as_ref().is_some_and(|loops| loops.len() > 1000) {
        return Err(ProbeError::FixtureInvariant(
            "hole removal allows at most 1000 selected loops",
        ));
    }
    let source = f.source.build(tolerance)?;
    if let Some(path) = &f.source.artifact_path {
        crate::brep_source::write_shared_artifact(
            &Geometry::Brep(source.clone()),
            path,
            tolerance,
        )?;
    }
    let result = match &f.loops {
        Some(loops) => source.try_remove_holes(loops, tolerance)?,
        None => source.try_remove_all_holes(tolerance)?,
    };
    Ok((
        json!({
            "before": crate::brep_interchange::definition_record(&source)?,
            "after": result.as_ref().map(crate::brep_interchange::definition_record).transpose()?,
        }),
        0,
    ))
}
