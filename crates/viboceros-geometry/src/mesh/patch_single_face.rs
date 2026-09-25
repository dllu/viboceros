use super::*;

/// Rhino's supported selections for constructing one mesh triangle or quad.
/// Indices refer to exact-location topology vertices and edges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeshSingleFaceComponents {
    ThreeVertices([usize; 3]),
    VertexAndEdge { vertex: usize, edge: usize },
    TwoEdges([usize; 2]),
}

impl TriangleMesh {
    /// Constructs a face from selected boundary components and optionally
    /// supplies a joined mesh. `None` means the components cannot form a
    /// nondegenerate face with consistent boundary winding.
    pub fn patch_single_face(
        &self,
        components: MeshSingleFaceComponents,
        tolerance: Tolerance,
    ) -> Result<Option<MeshHoleFill>, GeometryError> {
        let data = self.topology_data();
        let edge_count = data.edges.len();
        let mut required_edges = Vec::new();
        let mut vertices = match components {
            MeshSingleFaceComponents::ThreeVertices(vertices) => vertices.to_vec(),
            MeshSingleFaceComponents::VertexAndEdge { vertex, edge } => {
                let Some((&(a, b), incidence)) = data.edges.iter().nth(edge) else {
                    return Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                        edge,
                        edge_count,
                    });
                };
                if incidence.count != 1 {
                    return Ok(None);
                }
                required_edges.push((a, b));
                vec![vertex, a, b]
            }
            MeshSingleFaceComponents::TwoEdges(edges) => {
                if edges[0] == edges[1] {
                    return Ok(None);
                }
                let mut vertices = Vec::new();
                for edge in edges {
                    let Some((&(a, b), incidence)) = data.edges.iter().nth(edge) else {
                        return Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                            edge,
                            edge_count,
                        });
                    };
                    if incidence.count != 1 {
                        return Ok(None);
                    }
                    required_edges.push((a, b));
                    vertices.extend([a, b]);
                }
                vertices
            }
        };
        for &vertex in &vertices {
            if vertex >= data.topological_vertex_count {
                return Err(GeometryError::MeshTopologyVertexIndexOutOfRange {
                    vertex,
                    vertex_count: data.topological_vertex_count,
                });
            }
        }
        vertices.sort_unstable();
        vertices.dedup();
        if !(3..=4).contains(&vertices.len())
            || matches!(components, MeshSingleFaceComponents::ThreeVertices(_))
                && vertices.len() != 3
        {
            return Ok(None);
        }
        let mut naked_vertex = vec![false; data.topological_vertex_count];
        for (&(a, b), incidence) in &data.edges {
            if incidence.count == 1 {
                naked_vertex[a] = true;
                naked_vertex[b] = true;
            }
        }
        if vertices.iter().any(|&vertex| !naked_vertex[vertex]) {
            return Ok(None);
        }
        let mut candidates = Vec::new();
        let permutations: &[[usize; 3]] = &[
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        let orders = if vertices.len() == 3 {
            vec![
                vec![vertices[0], vertices[1], vertices[2]],
                vec![vertices[0], vertices[2], vertices[1]],
            ]
        } else {
            permutations
                .iter()
                .map(|p| {
                    vec![
                        vertices[0],
                        vertices[p[0] + 1],
                        vertices[p[1] + 1],
                        vertices[p[2] + 1],
                    ]
                })
                .collect()
        };
        for order in orders {
            let mut naked = 0;
            let mut bridge_length = 0.;
            let mut valid = true;
            for side in 0..order.len() {
                let a = order[side];
                let b = order[(side + 1) % order.len()];
                let edge = (a.min(b), a.max(b));
                if let Some(incidence) = data.edges.get(&edge) {
                    if incidence.count != 1 || incidence.first_use.unwrap().forward == (a < b) {
                        valid = false;
                        break;
                    }
                    naked += 1;
                } else {
                    bridge_length += data.topological_points[a]
                        .distance_to(data.topological_points[b])
                        .unwrap_or(Real::INFINITY);
                }
            }
            if !valid
                || naked == 0
                || required_edges
                    .iter()
                    .any(|&edge| !cycle_has_edge(&order, edge))
            {
                continue;
            }
            let points = order
                .iter()
                .map(|&vertex| data.topological_points[vertex])
                .collect::<Vec<_>>();
            if order.len() == 4 && !simple_projected_quad(&points)? {
                continue;
            }
            let face = if order.len() == 3 {
                MeshFace::Triangle([0, 1, 2])
            } else {
                MeshFace::Quad([0, 1, 2, 3])
            };
            let Ok(mut patch) = Self::try_new_faces(points, vec![face], tolerance) else {
                continue;
            };
            if let Some(source_colors) = &self.vertex_colors {
                let colors = order
                    .iter()
                    .map(|&topology| {
                        let raw = data
                            .topological_vertices
                            .iter()
                            .position(|&candidate| candidate == topology)
                            .expect("a topology vertex has a raw mesh vertex");
                        source_colors[raw]
                    })
                    .collect();
                patch.vertex_colors = Some(colors);
            }
            candidates.push((naked, bridge_length, order, patch));
        }
        candidates.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.total_cmp(&right.1))
                .then_with(|| left.2.cmp(&right.2))
        });
        let Some((_, _, _, patch)) = candidates.into_iter().next() else {
            return Ok(None);
        };
        let filled = Self::try_append(&[self, &patch])?;
        Ok(Some(MeshHoleFill { filled, patch }))
    }
}

fn cycle_has_edge(vertices: &[usize], edge: (usize, usize)) -> bool {
    (0..vertices.len()).any(|index| {
        let a = vertices[index];
        let b = vertices[(index + 1) % vertices.len()];
        (a.min(b), a.max(b)) == edge
    })
}

fn simple_projected_quad(points: &[Point3]) -> Result<bool, GeometryError> {
    let Some(projected) = project_mesh_hole_boundary(points)? else {
        return Ok(false);
    };
    let crosses = |a: usize, b: usize, c: usize, d: usize| {
        let side = |i: usize, j: usize, k: usize| {
            mesh_hole_cross(projected[i], projected[j], projected[k])
        };
        let ab_c = side(a, b, c);
        let ab_d = side(a, b, d);
        let cd_a = side(c, d, a);
        let cd_b = side(c, d, b);
        ab_c * ab_d <= 0. && cd_a * cd_b <= 0.
    };
    Ok(!crosses(0, 1, 2, 3) && !crosses(1, 2, 3, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patches_single_triangle_and_quad_from_boundary_components() {
        let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
        let source = TriangleMesh::try_new_faces(
            vec![
                point(0., 0., 0.),
                point(1., 0., 0.),
                point(1., 1., 0.),
                point(0., 1., 0.),
                point(0., 0., 1.),
            ],
            vec![
                MeshFace::Triangle([0, 1, 4]),
                MeshFace::Triangle([1, 2, 4]),
                MeshFace::Triangle([2, 3, 4]),
                MeshFace::Triangle([3, 0, 4]),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .try_with_vertex_colors(Some((0..5).map(|i| [i, 10, 20, 0]).collect()))
        .unwrap();
        let data = source.topology_data();
        let find_edge = |a: usize, b: usize| {
            data.edges
                .keys()
                .position(|edge| *edge == (a.min(b), a.max(b)))
                .unwrap()
        };
        let two = source
            .patch_single_face(
                MeshSingleFaceComponents::TwoEdges([find_edge(0, 1), find_edge(2, 3)]),
                Tolerance::DEFAULT,
            )
            .unwrap()
            .unwrap();
        assert_eq!(two.patch().faces().len(), 1);
        assert!(matches!(two.patch().faces()[0], MeshFace::Quad(_)));
        assert_eq!(two.patch().vertex_colors().unwrap().len(), 4);
        assert_eq!(two.filled().vertex_colors().unwrap().len(), 9);
        assert!(two.filled().topology().is_closed());
        assert_eq!(two.filled().topology().orientation_conflict_edge_count(), 0);
        let adjacent = source
            .patch_single_face(
                MeshSingleFaceComponents::TwoEdges([find_edge(0, 1), find_edge(1, 2)]),
                Tolerance::DEFAULT,
            )
            .unwrap()
            .unwrap();
        assert!(matches!(adjacent.patch().faces()[0], MeshFace::Triangle(_)));
        assert!(
            source
                .patch_single_face(
                    MeshSingleFaceComponents::TwoEdges([find_edge(0, 1), find_edge(0, 4)]),
                    Tolerance::DEFAULT,
                )
                .unwrap()
                .is_none()
        );
        let triangle = source
            .patch_single_face(
                MeshSingleFaceComponents::VertexAndEdge {
                    vertex: 2,
                    edge: find_edge(0, 1),
                },
                Tolerance::DEFAULT,
            )
            .unwrap()
            .unwrap();
        assert!(matches!(triangle.patch().faces()[0], MeshFace::Triangle(_)));
        assert_eq!(triangle.filled().face_count(), 5);
        let by_vertices = source
            .patch_single_face(
                MeshSingleFaceComponents::ThreeVertices([0, 1, 2]),
                Tolerance::DEFAULT,
            )
            .unwrap()
            .unwrap();
        assert_eq!(by_vertices.patch().face_count(), 1);
    }

    #[test]
    fn rejects_crossed_quad_boundary() {
        let points = [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]]
            .map(|point| Point3::try_from(point).unwrap());
        assert!(simple_projected_quad(&points).unwrap());
        assert!(!simple_projected_quad(&[points[0], points[2], points[1], points[3]]).unwrap());
    }
}
