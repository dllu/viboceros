//! Kernel composition and spatially indexed grouping of outer and cavity shells.
use super::*;
mod components;
mod split;
use components::components;
use rstar::{AABB, RTree, RTreeObject};
use viboceros_smlib::{BooleanOperation, mesh_boolean, mesh_boolean_interaction};

struct Bounds {
    envelope: AABB<[f64; 3]>,
    index: usize,
}
impl RTreeObject for Bounds {
    type Envelope = AABB<[f64; 3]>;
    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}
fn envelope(mesh: &TriangleMesh, tolerance: Tolerance) -> Result<AABB<[f64; 3]>, CommandError> {
    let bounds = mesh.bounds();
    let min = bounds.min().to_array().map(|x| x - tolerance.absolute());
    let max = bounds.max().to_array().map(|x| x + tolerance.absolute());
    if min.iter().chain(&max).any(|x| !x.is_finite()) {
        return Err(GeometryError::NonFinite {
            context: "mesh Boolean bounds",
        }
        .into());
    }
    Ok(AABB::from_corners(min, max))
}
fn any_interaction(
    meshes: &[&TriangleMesh],
    first_set: Option<usize>,
    tolerance: Tolerance,
) -> Result<bool, CommandError> {
    let entries = meshes
        .iter()
        .enumerate()
        .map(|(index, mesh)| {
            Ok(Bounds {
                envelope: envelope(mesh, tolerance)?,
                index,
            })
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    let tree = RTree::bulk_load(entries);
    for (i, mesh) in meshes
        .iter()
        .take(first_set.unwrap_or(meshes.len()))
        .enumerate()
    {
        for candidate in tree.locate_in_envelope_intersecting(envelope(mesh, tolerance)?) {
            if first_set.map_or(candidate.index > i, |boundary| candidate.index >= boundary)
                && mesh_boolean_interaction(mesh, meshes[candidate.index], tolerance)?
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
fn union(
    meshes: &[&TriangleMesh],
    tolerance: Tolerance,
) -> Result<Option<TriangleMesh>, CommandError> {
    let mut result = None;
    for &mesh in meshes {
        result = match result {
            None => Some(mesh.clone()),
            Some(current) => mesh_boolean(&current, mesh, BooleanOperation::Union, tolerance)?,
        };
    }
    Ok(result)
}

pub(super) fn stage(
    kind: Kind,
    meshes: &[&TriangleMesh],
    first: usize,
    tolerance: Tolerance,
) -> Result<Vec<TriangleMesh>, CommandError> {
    for mesh in meshes {
        mesh.validate_face_geometry(Tolerance::MESH_VALIDATION)?;
    }
    if kind == Kind::Split {
        return split::split(meshes, first, tolerance);
    }
    if kind == Kind::Difference {
        if !any_interaction(meshes, Some(first), tolerance)? {
            return Ok(vec![]);
        }
        let Some(cutters) = union(&meshes[first..], tolerance)? else {
            return Ok(vec![]);
        };
        // A severed target remains one mesh, but separate target objects retain
        // separate output slots, including peers that were not cut.
        return meshes[..first]
            .iter()
            .map(|target| {
                mesh_boolean(target, &cutters, BooleanOperation::Difference, tolerance)
                    .map_err(CommandError::from)
            })
            .filter_map(|result| result.transpose())
            .collect();
    }
    let result = if kind == Kind::Union {
        if !any_interaction(meshes, None, tolerance)? {
            return Ok(vec![]);
        }
        union(meshes, tolerance)?
    } else {
        let Some(a) = union(&meshes[..first], tolerance)? else {
            return Ok(vec![]);
        };
        let Some(b) = union(&meshes[first..], tolerance)? else {
            return Ok(vec![]);
        };
        mesh_boolean(&a, &b, BooleanOperation::Intersection, tolerance)?
    };
    match result {
        Some(mesh) => components(mesh, tolerance),
        None => Ok(vec![]),
    }
}
