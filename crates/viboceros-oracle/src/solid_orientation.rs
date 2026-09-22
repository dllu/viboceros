//! A spatial-orientation query on one shared, uninserted 3dm B-rep.
use super::*;
use crate::brep_source::{BrepSourceFixture, write_shared_artifact};
use viboceros_geometry::BrepSolidOrientation;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SolidOrientationFixture {
    sources: Vec<BrepSourceFixture>,
    #[serde(default)]
    flip_faces: Vec<usize>,
    artifact_path: Option<String>,
}

pub(super) fn run(
    f: &SolidOrientationFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1 || f.sources.is_empty() || f.sources.len() > 8 {
        return Err(ProbeError::FixtureInvariant(
            "solid orientation requires one iteration and one to eight B-rep sources",
        ));
    }
    let parts = f
        .sources
        .iter()
        .map(|s| s.build(tolerance))
        .collect::<Result<Vec<_>, _>>()?;
    let mut brep = Brep::try_combine(parts, tolerance)?;
    let mut faces = brep.faces().to_vec();
    let mut seen = std::collections::BTreeSet::new();
    for &index in &f.flip_faces {
        if index >= faces.len() || !seen.insert(index) {
            return Err(ProbeError::FixtureInvariant(
                "invalid individual face reversal",
            ));
        }
        let face = &faces[index];
        faces[index] = BrepFace::try_new(
            face.surface().clone(),
            !face.is_reversed(),
            face.loops().to_vec(),
        )?;
    }
    if !f.flip_faces.is_empty() {
        brep = Brep::try_new(
            brep.vertices().to_vec(),
            brep.edges().to_vec(),
            faces,
            tolerance,
        )?;
    }
    if let Some(path) = &f.artifact_path {
        write_shared_artifact(&Geometry::Brep(brep.clone()), path, tolerance)?;
    }
    let sense = match brep.solid_orientation()? {
        BrepSolidOrientation::NotSolid => "None",
        BrepSolidOrientation::Outward => "Outward",
        BrepSolidOrientation::Inward => "Inward",
        BrepSolidOrientation::Unknown => "Unknown",
    };
    Ok((
        json!({"orientation":sense,"solid":brep.is_solid(),"closed":brep.is_closed(),
        "geometry":crate::brep_interchange::definition_record(&brep)?}),
        0,
    ))
}
