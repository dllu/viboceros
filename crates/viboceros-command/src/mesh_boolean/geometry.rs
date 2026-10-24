//! Kernel composition and spatially indexed grouping of outer and cavity shells.
use super::*;
use rstar::{AABB, RTree, RTreeObject};
use viboceros_geometry::{MeshSolid, SolidPointLocation};
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
fn any_interaction(meshes: &[&TriangleMesh], tolerance: Tolerance) -> Result<bool, CommandError> {
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
    for (i, mesh) in meshes.iter().enumerate() {
        for candidate in tree.locate_in_envelope_intersecting(envelope(mesh, tolerance)?) {
            if candidate.index > i
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
    let result = if kind == Kind::Union {
        if !any_interaction(meshes, tolerance)? {
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
        if kind == Kind::Difference && !mesh_boolean_interaction(&a, &b, tolerance)? {
            return Ok(vec![]);
        }
        mesh_boolean(
            &a,
            &b,
            if kind == Kind::Intersection {
                BooleanOperation::Intersection
            } else {
                BooleanOperation::Difference
            },
            tolerance,
        )?
    };
    if kind == Kind::Difference {
        return Ok(result.into_iter().collect());
    }
    match result {
        Some(mesh) => components(mesh, tolerance),
        None => Ok(vec![]),
    }
}

fn components(mesh: TriangleMesh, tolerance: Tolerance) -> Result<Vec<TriangleMesh>, CommandError> {
    let pieces = mesh.try_disjoint_pieces(usize::MAX)?;
    if pieces.len() < 2 {
        return Ok(pieces);
    }
    let volumes = pieces
        .iter()
        .map(TriangleMesh::signed_volume)
        .collect::<Result<Vec<_>, _>>()?;
    let solids = pieces
        .iter()
        .zip(&volumes)
        .map(|(mesh, volume)| {
            (*volume > 0.)
                .then(|| MeshSolid::from_closed_mesh(mesh))
                .flatten()
        })
        .collect::<Vec<_>>();
    let entries = solids
        .iter()
        .enumerate()
        .filter(|(_, s)| s.is_some())
        .map(|(index, _)| {
            Ok(Bounds {
                envelope: envelope(&pieces[index], tolerance)?,
                index,
            })
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    let tree = RTree::bulk_load(entries);
    let mut owners = (0..pieces.len()).collect::<Vec<_>>();
    for (index, piece) in pieces.iter().enumerate() {
        if volumes[index] >= 0. || !piece.topology().is_solid() {
            continue;
        }
        let point = piece.vertices()[0];
        let mut best = None;
        for candidate in tree.locate_in_envelope_intersecting(AABB::from_point(point.to_array())) {
            let owner = candidate.index;
            if solids[owner]
                .as_ref()
                .unwrap()
                .classify_point(point, tolerance)?
                == SolidPointLocation::Inside
                && best.is_none_or(|previous: usize| volumes[owner] < volumes[previous])
            {
                best = Some(owner);
            }
        }
        owners[index] = best.unwrap_or(index);
    }
    let mut ranks = vec![usize::MAX; pieces.len()];
    let mut groups = Vec::<Vec<&TriangleMesh>>::new();
    for (index, &owner) in owners.iter().enumerate() {
        if ranks[owner] == usize::MAX {
            ranks[owner] = groups.len();
            groups.push(vec![]);
        }
        groups[ranks[owner]].push(&pieces[index]);
    }
    if groups.len() > MAX_SPAN_OUTPUT_OBJECTS {
        return Err(too_many_span_outputs("mesh Boolean"));
    }
    groups
        .into_iter()
        .map(|pieces| {
            if pieces.len() == 1 {
                Ok(pieces[0].clone())
            } else {
                TriangleMesh::try_append(&pieces).map_err(Into::into)
            }
        })
        .collect()
}
