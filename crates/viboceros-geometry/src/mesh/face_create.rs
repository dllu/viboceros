//! Direct mesh faces share polygon triangulation and validated n-gon overlays.
use super::*;

impl TriangleMesh {
    /// The oriented boundary loop starting with the selected incident-face edge.
    /// Every boundary vertex on this loop must have one incoming and outgoing use.
    /// The returned first pair is the reverse of `boundary_face_seed`.
    pub fn boundary_face_loop(&self, index: usize) -> Result<Vec<Point3>, GeometryError> {
        let data = self.topology_data();
        let Some((&(a, b), incidence)) = data.edges.iter().nth(index) else {
            return Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                edge: index,
                edge_count: data.edges.len(),
            });
        };
        let invalid = || GeometryError::Degenerate {
            context: "mesh boundary chain is open or ambiguous",
        };
        if incidence.count != 1 {
            return Err(invalid());
        }
        let (start, next) = if incidence.first_use.unwrap().forward {
            (a, b)
        } else {
            (b, a)
        };
        let mut successors = vec![None; data.topological_points.len()];
        let mut incoming = vec![0u8; data.topological_points.len()];
        let mut outgoing = vec![0u8; data.topological_points.len()];
        for (&(a, b), incidence) in &data.edges {
            if incidence.count == 1 {
                let (a, b) = if incidence.first_use.unwrap().forward {
                    (a, b)
                } else {
                    (b, a)
                };
                successors[a] = Some(b);
                outgoing[a] = outgoing[a].saturating_add(1);
                incoming[b] = incoming[b].saturating_add(1);
            }
        }
        let mut indices = vec![start];
        let mut vertex = next;
        while vertex != start {
            if indices.len() >= MAX_MESH_PATCH_POINTS {
                return Err(GeometryError::MeshPatchLimit);
            }
            if outgoing[vertex] != 1 || incoming[vertex] != 1 || indices.len() > data.edges.len() {
                return Err(invalid());
            }
            indices.push(vertex);
            vertex = successors[vertex].unwrap();
        }
        if outgoing[start] != 1 || incoming[start] != 1 {
            return Err(invalid());
        }
        Ok(indices
            .into_iter()
            .map(|i| data.topological_points[i])
            .collect())
    }
    /// New-face winding starts opposite the sole face at a naked edge.
    pub fn boundary_face_seed(&self, index: usize) -> Result<[Point3; 2], GeometryError> {
        let data = self.topology_data();
        let Some((&(a, b), incidence)) = data.edges.iter().nth(index) else {
            return Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                edge: index,
                edge_count: data.edges.len(),
            });
        };
        if incidence.count != 1 {
            return Err(GeometryError::Degenerate {
                context: "a new face needs a boundary edge",
            });
        }
        let forward = incidence.first_use.unwrap().forward;
        let indices = if forward { [b, a] } else { [a, b] };
        Ok(indices.map(|i| data.topological_points[i]))
    }
    /// Create a triangle, quad or triangulated n-gon from ordered corners.
    /// `in_plane` constrains corners after the first three to their plane.
    /// An n-gon overlay is attached only when there are at least five corners.
    pub fn try_face_from_points(points: &[Point3], in_plane: bool) -> Result<Self, GeometryError> {
        if points.len() > MAX_MESH_PATCH_POINTS {
            return Err(GeometryError::MeshPatchLimit);
        }
        if points.len() < 3 {
            return Err(GeometryError::Degenerate {
                context: "a mesh face needs at least three corners",
            });
        }
        let mut points = points.to_vec();
        if in_plane && points.len() > 3 {
            let frame = Frame3::try_from_directions(
                points[0],
                points[0].vector_to(points[1])?,
                points[0].vector_to(points[2])?,
                Tolerance::NUMERICAL_VALIDATION,
            )?;
            for point in &mut points[3..] {
                let [u, v] = frame.projected_coordinates_of(*point)?;
                *point = frame.point_at([u, v, 0.])?;
            }
        }
        match points.len() {
            3 => Self::try_new(points, vec![[0, 1, 2]], Tolerance::MESH_VALIDATION),
            4 => {
                // A collinear corner can leave the alternate diagonal usable.
                // Validate the same split used by projection and mass properties,
                // retaining the original polygon record and winding.
                let triangles = mass_triangles::split(&points, [0, 1, 2, 3]);
                Self::try_new(
                    points.clone(),
                    triangles.to_vec(),
                    Tolerance::MESH_VALIDATION,
                )?;
                let mesh = Self::try_from_face_records(points, vec![MeshFace::Quad([0, 1, 2, 3])])?;
                mesh.polygon_face_normal(0)
                    .map_err(|_| GeometryError::DegenerateQuad { face: 0 })?;
                Ok(mesh)
            }
            count => {
                points.push(points[0]);
                let boundary = Polyline3::try_new(points, Tolerance::NUMERICAL_VALIDATION)?;
                let mesh = Self::try_from_closed_polyline(&boundary)?.ok_or(
                    GeometryError::Degenerate {
                        context: "mesh face boundary cannot be triangulated",
                    },
                )?;
                let faces = (0..mesh.faces().len() as u32).collect();
                mesh.try_with_ngons(vec![MeshNgon::from_parts(
                    (0..count as u32).collect(),
                    faces,
                )])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    #[test]
    fn boundary_seed_reverses_the_incident_face_and_rejects_interior_edges() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0., 0., 0.),
                point(2., 0., 0.),
                point(2., 2., 0.),
                point(0., 2., 0.),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::MESH_VALIDATION,
        )
        .unwrap();
        for edge in mesh.diagnostic_edges() {
            let seed = mesh.boundary_face_seed(edge.index);
            if edge.points.contains(&point(0., 0., 0.)) && edge.points.contains(&point(2., 2., 0.))
            {
                assert!(seed.is_err());
            } else {
                let seed = seed.unwrap();
                let normal = seed[0]
                    .vector_to(seed[1])
                    .unwrap()
                    .cross(seed[0].vector_to(point(1., 1., 0.)).unwrap())
                    .unwrap();
                assert!(normal.z() < 0.);
            }
        }
        assert!(mesh.boundary_face_seed(usize::MAX).is_err());
    }
    #[test]
    fn in_plane_projection_uses_the_first_three_corners_and_rejects_collinearity() {
        let input = [
            point(0., 0., 0.),
            point(4., 0., 0.),
            point(4., 3., 2.),
            point(0., 3., 0.),
        ];
        let projected = TriangleMesh::try_face_from_points(&input, true).unwrap();
        assert_eq!(&projected.vertices()[..3], &input[..3]);
        assert!(
            projected.vertices()[3]
                .distance_to(point(0., 27. / 13., 18. / 13.))
                .unwrap()
                < 1e-12
        );
        assert!(
            TriangleMesh::try_face_from_points(
                &[
                    point(0., 0., 0.),
                    point(1., 0., 0.),
                    point(2., 0., 0.),
                    point(0., 1., 0.)
                ],
                true
            )
            .is_err()
        );
    }

    #[test]
    fn boundary_loops_retain_winding_and_decline_touching_vertex_branches() {
        let square = |x: f64, y: f64| {
            TriangleMesh::try_face_from_points(
                &[
                    point(x, y, 0.),
                    point(x + 2., y, 0.),
                    point(x + 2., y + 2., 0.),
                    point(x, y + 2., 0.),
                ],
                false,
            )
            .unwrap()
        };
        let first = square(0., 0.);
        let second = square(2., 2.);
        let edge = first.diagnostic_edges()[0].index;
        let path = first.boundary_face_loop(edge).unwrap();
        let seed = first.boundary_face_seed(edge).unwrap();
        assert_eq!(&path[..2], &[seed[1], seed[0]]);
        assert_eq!(path.len(), 4);
        let touching = TriangleMesh::try_append(&[&first, &second]).unwrap();
        assert!(touching.boundary_face_loop(edge).is_err());
    }
    #[test]
    fn collinear_corners_use_a_valid_shorter_diagonal_without_reordering_the_quad() {
        let corners = [
            point(0., 0., 0.),
            point(0., 3., 0.),
            point(0., 6., 0.),
            point(-3., 0., 0.),
        ];
        let mesh = TriangleMesh::try_face_from_points(&corners, false).unwrap();
        assert_eq!(mesh.vertices(), &corners);
        assert_eq!(mesh.faces(), &[MeshFace::Quad([0, 1, 2, 3])]);
        let (triangles, _) = mesh.triangulate_quads(Tolerance::MESH_VALIDATION).unwrap();
        assert_eq!(triangles.faces().len(), 2);
        assert!((mesh.area().unwrap() - 9.).abs() < 1e-12);
        assert!(mesh.polygon_face_normal(0).unwrap().as_vector().z() > 0.);
        assert!(
            TriangleMesh::try_face_from_points(
                &[
                    point(0., 0., 0.),
                    point(1., 0., 0.),
                    point(2., 0., 0.),
                    point(3., 0., 0.)
                ],
                false
            )
            .is_err()
        );
    }
}
