//! Explicit native edge assembly against public automatic Rhino JoinBreps.
//! Fixtures must describe one connected output with unambiguous full-edge pairs.
use super::*;
use crate::brep_source::{BrepSourceFixture, write_shared_artifact};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct BrepJoinFixture {
    sources: Vec<BrepSourceFixture>,
    pairs: Vec<(usize, usize, bool)>,
    join_tolerance: f64,
    artifact_paths: Option<Vec<String>>,
}

pub(super) fn run(f: &BrepJoinFixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    if !f.join_tolerance.is_finite() || f.join_tolerance < 0. {
        return Err(ProbeError::FixtureInvariant(
            "B-rep Join needs a finite nonnegative distance",
        ));
    }
    if f.sources.is_empty()
        || f.artifact_paths
            .as_ref()
            .is_some_and(|p| p.len() != f.sources.len())
    {
        return Err(ProbeError::FixtureInvariant(
            "B-rep Join needs one artifact per source",
        ));
    }
    let sources = f
        .sources
        .iter()
        .map(|s| s.build(tolerance))
        .collect::<Result<Vec<_>, ProbeError>>()?;
    let inputs = sources
        .iter()
        .map(|s| geometry_record(s, tolerance))
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(paths) = &f.artifact_paths {
        for (source, path) in sources.iter().zip(paths) {
            write_shared_artifact(&Geometry::Brep(source.clone()), path, tolerance)?;
        }
    }
    let combined = Brep::try_combine(sources, tolerance)?;
    let joined = combined.try_join_edge_pairs(&f.pairs, f.join_tolerance, tolerance)?;
    Ok((
        json!({"inputs":inputs,"outputs":[geometry_record(&joined,tolerance)?]}),
        0,
    ))
}

pub(super) fn geometry_record(brep: &Brep, tolerance: Tolerance) -> Result<Value, ProbeError> {
    let mut record = crate::cap_command::geometry_record(brep, tolerance)?;
    record["vertex_tolerances"] = json!(
        brep.vertices()
            .iter()
            .map(|v| v.tolerance())
            .collect::<Vec<_>>()
    );
    record["edge_tolerances"] = json!(
        brep.edges()
            .iter()
            .map(|e| e.tolerance())
            .collect::<Vec<_>>()
    );
    record["surfaces"] = json!(
        brep.faces()
            .iter()
            .map(|f| nurbs_surface_definition_value(f.surface()))
            .collect::<Vec<_>>()
    );
    record["face_reversed"] = json!(
        brep.faces()
            .iter()
            .map(|f| f.is_reversed())
            .collect::<Vec<_>>()
    );
    record["trim_curves"] = json!(
        brep.faces()
            .iter()
            .map(|f| f
                .loops()
                .iter()
                .map(|l| l
                    .trims()
                    .iter()
                    .map(|t| nurbs_curve2_definition_value(t.curve()))
                    .collect::<Vec<_>>())
                .collect::<Vec<_>>())
            .collect::<Vec<_>>()
    );
    Ok(record)
}

#[cfg(test)]
mod tests;
