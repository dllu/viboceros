//! Endpoint graph recovery, without splitting crossing input segments.
use super::*;
mod cycles;
#[cfg(test)]
mod tests;

const MAX_WORK: usize = 4_000_000;
fn charge(work: &mut usize, amount: usize) -> Result<(), GeometryError> {
    *work = work
        .checked_add(amount)
        .filter(|n| *n <= MAX_WORK)
        .ok_or(GeometryError::MeshLineNetworkLimit)?;
    Ok(())
}
impl TriangleMesh {
    /// Maximum input line count for endpoint-network meshing.
    pub const MAX_LINE_NETWORK_LINES: usize = 100_000;
    /// Recover short closed regions of an endpoint-connected line graph.
    /// Crossings inside segments do not introduce vertices. All triangular
    /// regions survive; longer chordless loops enter an independent cycle set.
    /// No closed region is represented by `None`, without a partial mesh.
    pub fn try_from_lines(
        lines: &[LineSegment],
        max_face_valence: usize,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        if max_face_valence < 3 {
            return Err(GeometryError::Degenerate {
                context: "mesh line-network face valence",
            });
        }
        if lines.len() > Self::MAX_LINE_NETWORK_LINES {
            return Err(GeometryError::MeshLineNetworkLimit);
        }
        if lines.len() < 3 {
            return Ok(None);
        }
        let mut work = 0;
        let (vertices, edges) = endpoints(lines, tolerance.absolute(), &mut work)?;
        let regions = cycles::recover(vertices.len(), &edges, max_face_valence, &mut work)?;
        let mut faces = Vec::new();
        for region in regions {
            let before = faces.len();
            match region.len() {
                3 => faces.push(MeshFace::Triangle(region.try_into().unwrap())),
                4 => faces.push(MeshFace::Quad(region.try_into().unwrap())),
                _ => {
                    let mut points = region
                        .iter()
                        .map(|&i| vertices[i as usize])
                        .collect::<Vec<_>>();
                    points.push(points[0]);
                    let boundary = Polyline3::try_new(points, Tolerance::NUMERICAL_VALIDATION)?;
                    let patch = Self::try_patch(
                        &[],
                        &[],
                        &[],
                        MeshPatchOptions {
                            outer_boundary: Some(&boundary),
                            ..Default::default()
                        },
                        tolerance,
                    )?;
                    let by_point = region
                        .iter()
                        .map(|&i| {
                            (
                                vertices[i as usize]
                                    .to_array()
                                    .map(canonical_coordinate_bits),
                                i,
                            )
                        })
                        .collect::<BTreeMap<_, _>>();
                    let remap = patch
                        .vertices()
                        .iter()
                        .map(|p| by_point[&p.to_array().map(canonical_coordinate_bits)])
                        .collect::<Vec<_>>();
                    for triangle in patch.triangles() {
                        faces.push(MeshFace::Triangle(triangle.map(|i| remap[i as usize])));
                    }
                }
            }
            charge(&mut work, faces.len() - before)?;
            if faces.len() > Self::MAX_LINE_NETWORK_LINES {
                return Err(GeometryError::MeshLineNetworkLimit);
            }
        }
        if faces.is_empty() {
            return Ok(None);
        }
        let mut mesh = Self::try_new_faces(vertices, faces, Tolerance::MESH_VALIDATION)?
            .culled_unused_vertices()
            .0;
        let normal = mesh.polygon_face_normal(0)?.as_vector().to_array();
        let axis = (0..3)
            .max_by(|&i, &j| normal[i].abs().total_cmp(&normal[j].abs()))
            .unwrap();
        if normal[axis] > 0. {
            mesh = mesh.reversed();
        }
        Ok(Some(mesh.unified_face_orientations()?.0))
    }
}

fn endpoints(
    lines: &[LineSegment],
    radius: Real,
    work: &mut usize,
) -> Result<(Vec<Point3>, BTreeSet<[usize; 2]>), GeometryError> {
    let input = lines
        .iter()
        .flat_map(|line| [line.start(), line.end()])
        .collect::<Vec<_>>();
    let mut parents = (0..input.len()).collect::<Vec<_>>();
    let mut unique = BTreeMap::new();
    for (i, point) in input.iter().enumerate() {
        charge(work, 1)?;
        if let Some(previous) = unique.insert(point.to_array().map(canonical_coordinate_bits), i) {
            union_indices_keep_later(&mut parents, previous, i);
        }
    }
    let mut indices = unique.into_values().collect::<Vec<_>>();
    let axis = (0..3)
        .max_by(|&i, &j| {
            let range = |axis| {
                let lo = input
                    .iter()
                    .map(|p| p.to_array()[axis])
                    .fold(Real::INFINITY, Real::min);
                let hi = input
                    .iter()
                    .map(|p| p.to_array()[axis])
                    .fold(Real::NEG_INFINITY, Real::max);
                hi - lo
            };
            range(i).total_cmp(&range(j))
        })
        .unwrap();
    indices.sort_by(|&a, &b| input[a].to_array()[axis].total_cmp(&input[b].to_array()[axis]));
    for (offset, &a) in indices.iter().enumerate() {
        for &b in &indices[offset + 1..] {
            if input[b].to_array()[axis] - input[a].to_array()[axis] > radius {
                break;
            }
            charge(work, 1)?;
            if input[a].distance_to(input[b])? <= radius {
                union_indices_keep_later(&mut parents, a, b);
            }
        }
    }
    let mut by_root = BTreeMap::new();
    let mut vertices = Vec::new();
    let mut remap = Vec::with_capacity(input.len());
    for i in 0..input.len() {
        let root = index_root(&mut parents, i);
        let node = *by_root.entry(root).or_insert_with(|| {
            let node = vertices.len();
            vertices.push(input[root]);
            node
        });
        remap.push(node);
    }
    let edges = remap
        .chunks_exact(2)
        .filter(|p| p[0] != p[1])
        .map(|p| [p[0].min(p[1]), p[0].max(p[1])])
        .collect();
    Ok((vertices, edges))
}
