//! The partitions of a connected four-triangle neighborhood without interior vertices.
use super::{Adjacency, NO_FACE, Probes, SurfaceMesh, Triangle, install, neighbors, sample};

// The fourteen partitions of a cyclic six-vertex polygon. The cavity checks
// each triangle in UV space, which rules out invalid partitions of a concave
// boundary.
const PARTITIONS: [[[usize; 3]; 4]; 14] = [
    [[3, 4, 5], [2, 3, 5], [1, 2, 5], [0, 1, 5]],
    [[2, 3, 4], [2, 4, 5], [1, 2, 5], [0, 1, 5]],
    [[1, 2, 3], [3, 4, 5], [1, 3, 5], [0, 1, 5]],
    [[2, 3, 4], [1, 2, 4], [1, 4, 5], [0, 1, 5]],
    [[1, 2, 3], [1, 3, 4], [1, 4, 5], [0, 1, 5]],
    [[0, 1, 2], [3, 4, 5], [2, 3, 5], [0, 2, 5]],
    [[0, 1, 2], [2, 3, 4], [2, 4, 5], [0, 2, 5]],
    [[1, 2, 3], [0, 1, 3], [3, 4, 5], [0, 3, 5]],
    [[0, 1, 2], [0, 2, 3], [3, 4, 5], [0, 3, 5]],
    [[2, 3, 4], [1, 2, 4], [0, 1, 4], [0, 4, 5]],
    [[1, 2, 3], [1, 3, 4], [0, 1, 4], [0, 4, 5]],
    [[0, 1, 2], [2, 3, 4], [0, 2, 4], [0, 4, 5]],
    [[1, 2, 3], [0, 1, 3], [0, 3, 4], [0, 4, 5]],
    [[0, 1, 2], [0, 2, 3], [0, 3, 4], [0, 4, 5]],
];

pub(super) fn repair(
    mesh: &SurfaceMesh<'_>,
    faces: &mut [Triangle],
    inverted: &[usize],
    adjacent: &mut Adjacency,
    probes: &mut Probes,
) {
    let mut groups = Vec::new();
    let mut next_groups = Vec::new();
    for &i in inverted {
        if !sample(mesh.surface, mesh.points, mesh.parameters, faces[i])
            .is_some_and(|s| s.alignment < 0.)
        {
            continue;
        }
        groups.clear();
        groups.push([i, NO_FACE, NO_FACE, NO_FACE]);
        // Add each shared-edge neighbor so the search includes strips and
        // three-way junctions. Deduplicate before growing the next layer.
        for size in 1..4 {
            next_groups.clear();
            for &group in &groups {
                for &member in &group[..size] {
                    for neighbor in neighbors(member, faces, adjacent).into_iter().flatten() {
                        if !group.contains(&neighbor) {
                            let mut grown = group;
                            grown[size] = neighbor;
                            grown[..=size].sort_unstable();
                            next_groups.push(grown);
                        }
                    }
                }
            }
            next_groups.sort_unstable();
            next_groups.dedup();
            std::mem::swap(&mut groups, &mut next_groups);
        }
        let best = groups
            .iter()
            .filter_map(|&indices| {
                let (error, replacement) =
                    mesh.best_partition::<4, 6>(indices, faces, adjacent, probes, &PARTITIONS)?;
                Some((error, indices, replacement))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, indices, replacement)) = best {
            install(faces, adjacent, indices, replacement);
        }
    }
}
