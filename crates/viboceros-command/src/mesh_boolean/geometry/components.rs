//! Separate disconnected outer bodies while retaining enclosed cavity shells.
use super::*;
use viboceros_geometry::{MeshSolid, SolidPointLocation};

pub(super) fn components(
    mesh: TriangleMesh,
    tolerance: Tolerance,
) -> Result<Vec<TriangleMesh>, CommandError> {
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
