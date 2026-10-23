//! Preserve original n-gon adjacency and overlay newly tessellated large polygons.
use super::*;
use crate::mesh::union_find::{index_root, union_faces};
pub(super) fn rebuild(
    source: &TriangleMesh,
    result: TriangleMesh,
    plan: &Plan,
    face_sources: &[usize],
    polygon_groups: &[Option<usize>],
) -> Result<TriangleMesh, GeometryError> {
    let mut owners = (0..source.face_count())
        .map(|i| source.ngons().len() + i)
        .collect::<Vec<_>>();
    for (index, ngon) in source.ngons().iter().enumerate() {
        for &f in ngon.faces() {
            owners[f as usize] = index;
        }
    }
    let mut counts = vec![0; source.face_count()];
    let mut changed = vec![false; source.face_count()];
    for (index, &original) in face_sources.iter().enumerate() {
        counts[original] += 1;
        changed[original] |= result.faces[index] != source.faces[original];
    }
    for (i, &count) in counts.iter().enumerate() {
        changed[i] |= count != 1;
    }
    let mut changed_parents = vec![false; source.ngons().len() + source.face_count()];
    for (f, &changed) in changed.iter().enumerate() {
        changed_parents[owners[f]] |= changed;
    }
    let edge_key = |a: Point3, b: Point3| {
        let a = a.to_array().map(canonical_coordinate_bits);
        let b = b.to_array().map(canonical_coordinate_bits);
        if a < b { [a, b] } else { [b, a] }
    };
    let barriers = plan
        .chords
        .iter()
        .map(|&[a, b]| edge_key(plan.points[a as usize], plan.points[b as usize]))
        .collect::<BTreeSet<_>>();
    let topology = result.topology_data();
    let mut parents = (0..result.face_count()).collect::<Vec<_>>();
    let mut ranks = vec![0; result.face_count()];
    for (&(a, b), incidence) in &topology.edges {
        if barriers.contains(&edge_key(
            topology.topological_points[a],
            topology.topological_points[b],
        )) {
            continue;
        }
        let mut uses = incidence.uses();
        let Some(first) = uses.next() else { continue };
        for other in uses {
            let a = face_sources[first.face];
            let b = face_sources[other.face];
            let original_adjacency = a != b && owners[a] == owners[b];
            let tessellated_ngon = polygon_groups[first.face].is_some()
                && polygon_groups[first.face] == polygon_groups[other.face];
            if original_adjacency || tessellated_ngon {
                union_faces(&mut parents, &mut ranks, first.face, other.face);
            }
        }
    }
    let mut groups = BTreeMap::<usize, Vec<u32>>::new();
    for f in 0..result.face_count() {
        groups
            .entry(index_root(&mut parents, f))
            .or_default()
            .push(f as u32);
    }
    let mut ngons = Vec::new();
    for faces in groups.into_values() {
        let parent = owners[face_sources[faces[0] as usize]];
        if faces.len() > 1 {
            ngons.push(
                result
                    .ngon_from_faces(faces)
                    .ok_or_else(|| invalid("cannot rebuild inserted mesh n-gon"))?,
            );
        } else if parent < source.ngons().len() && !changed_parents[parent] {
            ngons.push(MeshNgon::from_parts(
                source.ngons()[parent].vertices().to_vec(),
                faces,
            ));
        }
    }
    result.try_with_ngons(ngons)
}
