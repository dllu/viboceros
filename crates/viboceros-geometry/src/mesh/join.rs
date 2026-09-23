//! Mesh joining keeps raw-vertex seams separate from positional connectivity.
use super::*;
#[cfg(test)]
mod tests;

const MAX_INPUTS: usize = 100_000;
const MAX_SCANS: usize = 16_000_000;
const MAX_PAIRS: usize = 1_000_000;

/// Explicit geometry policy, independent of document or command preferences.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshJoinOptions {
    pub join_disjoint: bool,
    pub alignment_tolerance: Real,
    /// Match finite binary32 positions while retaining binary64 output
    /// coordinates. Positions outside the binary32 range stay in binary64.
    pub single_precision_matching: bool,
}

/// One joined mesh and its input indices, in source order.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshJoinComponent {
    pub source_indices: Vec<usize>,
    pub mesh: TriangleMesh,
}

/// Joins meshes, optionally retaining disconnected pieces in a single output.
/// Naked vertices align to directly neighboring anchor positions without
/// welding their distinct raw indices. Connected-only outputs compact
/// raw vertices in face-use order and orient matching source boundaries.
pub fn join_meshes(
    meshes: &[&TriangleMesh],
    options: MeshJoinOptions,
) -> Result<Vec<MeshJoinComponent>, GeometryError> {
    let alignment_tolerance = options.alignment_tolerance;
    if !alignment_tolerance.is_finite() || alignment_tolerance < 0. {
        return Err(GeometryError::InvalidMeshJoinTolerance);
    }
    if meshes.len() > MAX_INPUTS {
        return Err(GeometryError::MeshJoinResourceLimit);
    }
    if meshes.is_empty() {
        return Ok(Vec::new());
    }
    if meshes.len() == 1 {
        return Ok(vec![MeshJoinComponent {
            source_indices: vec![0],
            mesh: meshes[0].clone(),
        }]);
    }
    let mut aligned = align(meshes, options)?;
    if options.join_disjoint {
        return Ok(vec![MeshJoinComponent {
            source_indices: (0..meshes.len()).collect(),
            mesh: TriangleMesh::try_append(&aligned.iter().collect::<Vec<_>>())?,
        }]);
    }
    for mesh in &mut aligned {
        *mesh = compact(mesh);
    }
    let connected = aligned.iter().map(point_connected).collect::<Vec<_>>();
    let mut locations = BTreeMap::<[u64; 3], Vec<usize>>::new();
    for (i, mesh) in aligned.iter().enumerate() {
        if !connected[i] {
            continue;
        }
        for p in &mesh.vertices {
            let indices = locations.entry(key(*p)).or_default();
            if indices.last() != Some(&i) {
                indices.push(i);
            }
        }
    }
    let mut neighbors = vec![BTreeSet::new(); meshes.len()];
    let mut scans = 0;
    let mut pairs = 0;
    for indices in locations.values() {
        for (offset, &a) in indices.iter().enumerate() {
            for &b in &indices[offset + 1..] {
                scans += 1;
                if scans > MAX_SCANS {
                    return Err(GeometryError::MeshJoinResourceLimit);
                }
                if neighbors[a].insert(b) {
                    pairs += 1;
                    if pairs > MAX_PAIRS {
                        return Err(GeometryError::MeshJoinResourceLimit);
                    }
                    neighbors[b].insert(a);
                }
            }
        }
    }
    let edges = aligned.iter().map(directed_edges).collect::<Vec<_>>();
    let mut reverse = vec![None; meshes.len()];
    let mut results = Vec::new();
    for seed in 0..meshes.len() {
        if reverse[seed].is_some() {
            continue;
        }
        reverse[seed] = Some(false);
        let mut queue = VecDeque::from([seed]);
        let mut indices = Vec::new();
        while let Some(a) = queue.pop_front() {
            indices.push(a);
            for &b in &neighbors[a] {
                if reverse[b].is_some() {
                    continue;
                }
                let same = edges[a]
                    .iter()
                    .find_map(|(pair, forward)| edges[b].get(pair).map(|other| forward == other))
                    .unwrap_or(true);
                reverse[b] = Some(reverse[a].unwrap() ^ same);
                queue.push_back(b);
            }
        }
        indices.sort_unstable();
        let oriented = indices
            .iter()
            .map(|&i| {
                if reverse[i].unwrap() {
                    aligned[i].reversed()
                } else {
                    aligned[i].clone()
                }
            })
            .collect::<Vec<_>>();
        results.push(MeshJoinComponent {
            source_indices: indices,
            mesh: TriangleMesh::try_append(&oriented.iter().collect::<Vec<_>>())?,
        });
    }
    Ok(results)
}

fn key(point: Point3) -> [u64; 3] {
    point.to_array().map(canonical_coordinate_bits)
}

fn compact(mesh: &TriangleMesh) -> TriangleMesh {
    let mut mapping = vec![None; mesh.vertices.len()];
    let mut vertices = Vec::new();
    let mut colors = mesh.vertex_colors.as_ref().map(|_| Vec::new());
    let faces = mesh
        .faces
        .iter()
        .map(|face| {
            face.remapped(|i| {
                *mapping[i as usize].get_or_insert_with(|| {
                    let index = vertices.len() as u32;
                    vertices.push(mesh.vertices[i as usize]);
                    if let Some(colors) = &mut colors {
                        colors.push(mesh.vertex_colors.as_ref().unwrap()[i as usize]);
                    }
                    index
                })
            })
        })
        .collect();
    let mut compacted = TriangleMesh::from_validated_parts(vertices, faces);
    compacted.vertex_colors = colors;
    compacted.ngons = mesh
        .ngons
        .iter()
        .map(|ngon| {
            MeshNgon::from_parts(
                ngon.vertices
                    .iter()
                    .map(|&vertex| {
                        mapping[vertex as usize].expect("n-gon vertices belong to faces")
                    })
                    .collect(),
                ngon.faces.clone(),
            )
        })
        .collect();
    compacted
}

fn point_connected(mesh: &TriangleMesh) -> bool {
    let mut first = BTreeMap::new();
    let mut parents = (0..mesh.faces.len()).collect::<Vec<_>>();
    let mut ranks = vec![0; mesh.faces.len()];
    for (face, polygon) in mesh.faces.iter().enumerate() {
        for &i in polygon.indices() {
            if let Some(&other) = first.get(&key(mesh.vertices[i as usize])) {
                union_faces(&mut parents, &mut ranks, face, other);
            } else {
                first.insert(key(mesh.vertices[i as usize]), face);
            }
        }
    }
    let root = index_root(&mut parents, 0);
    (1..mesh.faces.len()).all(|i| index_root(&mut parents, i) == root)
}

type EdgeKey = ([u64; 3], [u64; 3]);
fn directed_edges(mesh: &TriangleMesh) -> BTreeMap<EdgeKey, bool> {
    let mut result = BTreeMap::new();
    for face in &mesh.faces {
        let indices = face.indices();
        for i in 0..indices.len() {
            let a = key(mesh.vertices[indices[i] as usize]);
            let b = key(mesh.vertices[indices[(i + 1) % indices.len()] as usize]);
            result
                .entry(if a < b { (a, b) } else { (b, a) })
                .or_insert(a < b);
        }
    }
    result
}

fn align(
    meshes: &[&TriangleMesh],
    options: MeshJoinOptions,
) -> Result<Vec<TriangleMesh>, GeometryError> {
    let tolerance = options.alignment_tolerance;
    let mut unique = BTreeMap::new();
    let mut points = Vec::new();
    let mut movable = Vec::new();
    let mut source_indices = Vec::new();
    for mesh in meshes {
        let topology = mesh.topology_data();
        let mut boundary = vec![false; topology.topological_vertex_count];
        let mut used = vec![false; mesh.vertices.len()];
        for face in &mesh.faces {
            for &i in face.indices() {
                used[i as usize] = true;
            }
        }
        for (&(a, b), edge) in &topology.edges {
            if edge.count == 1 {
                boundary[a] = true;
                boundary[b] = true;
            }
        }
        let indices = topology
            .topological_vertices
            .iter()
            .enumerate()
            .map(|(i, &topological)| {
                used[i].then(|| {
                    let index = *unique.entry(key(mesh.vertices[i])).or_insert_with(|| {
                        let index = points.len();
                        points.push(mesh.vertices[i]);
                        movable.push(true);
                        index
                    });
                    movable[index] &= boundary[topological];
                    index
                })
            })
            .collect::<Vec<_>>();
        source_indices.push(indices);
    }
    let coordinates = points
        .iter()
        .map(|p| {
            let coordinates = p.to_array();
            let single = coordinates.map(|c| f64::from(c as f32));
            if options.single_precision_matching && single.iter().all(|c| c.is_finite()) {
                single
            } else {
                coordinates
            }
        })
        .collect::<Vec<_>>();
    let mut neighbors = vec![Vec::new(); points.len()];
    if points.len() > 1 {
        let mut min = [Real::INFINITY; 3];
        let mut max = [Real::NEG_INFINITY; 3];
        for p in &coordinates {
            for (i, c) in p.iter().copied().enumerate() {
                min[i] = min[i].min(c);
                max[i] = max[i].max(c);
            }
        }
        let axis = (0..3)
            .max_by(|&a, &b| (max[a] - min[a]).total_cmp(&(max[b] - min[b])))
            .unwrap();
        let mut sorted = (0..points.len()).collect::<Vec<_>>();
        sorted.sort_by(|&a, &b| coordinates[a][axis].total_cmp(&coordinates[b][axis]));
        let mut scans = 0;
        let mut matches = 0;
        for (position, &a) in sorted.iter().enumerate() {
            for &b in &sorted[position + 1..] {
                let delta =
                    std::array::from_fn::<_, 3, _>(|i| coordinates[b][i] - coordinates[a][i]);
                if delta[axis] > tolerance {
                    break;
                }
                scans += 1;
                if scans > MAX_SCANS {
                    return Err(GeometryError::MeshJoinResourceLimit);
                }
                if delta[0].hypot(delta[1]).hypot(delta[2]) <= tolerance {
                    matches += 1;
                    if matches > MAX_PAIRS {
                        return Err(GeometryError::MeshJoinResourceLimit);
                    }
                    neighbors[a].push(b);
                    neighbors[b].push(a);
                }
            }
        }
    }
    // A later unassigned anchor may reclaim a neighbor already assigned to an
    // earlier anchor. Never take a transitive closure: every move must match
    // the original point directly to its final anchor. Interior points stay
    // fixed, but can anchor nearby naked vertices.
    let mut targets = (0..points.len()).collect::<Vec<_>>();
    for anchor in (0..points.len())
        .filter(|&i| !movable[i])
        .chain((0..points.len()).filter(|&i| movable[i]))
    {
        if targets[anchor] != anchor {
            continue;
        }
        for &neighbor in &neighbors[anchor] {
            if movable[neighbor] {
                targets[neighbor] = anchor;
            }
        }
    }
    meshes
        .iter()
        .zip(source_indices)
        .map(|(mesh, indices)| {
            let mut vertices = mesh.vertices.clone();
            for (vertex, index) in vertices.iter_mut().zip(indices) {
                if let Some(index) = index {
                    *vertex = points[targets[index]];
                }
            }
            TriangleMesh::try_new_faces(vertices, mesh.faces.clone(), Tolerance::MESH_VALIDATION)?
                .try_with_ngons(mesh.ngons.clone())?
                .try_with_vertex_colors(mesh.vertex_colors.clone())
        })
        .collect()
}
