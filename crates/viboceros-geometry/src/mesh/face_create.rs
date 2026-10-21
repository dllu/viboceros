//! Direct mesh faces share polygon triangulation and validated n-gon overlays.
use super::*;

impl TriangleMesh {
    /// Direct point entry reverses its first side when the opposite sides
    /// run in the same direction; collinear first corners keep their order.
    pub fn face_quad_reverses_start([a, b, c, d]: [Point3; 4]) -> Result<bool, GeometryError> {
        let edge = a.vector_to(b)?;
        Ok(edge.unit_cross_point_difference(a, c)?.is_some() && edge.dot(c.vector_to(d)?)? > 0.)
    }
    /// Exact orientation of a corner triangle against a reference face normal.
    pub fn face_corner_side(
        points: [Point3; 3],
        normal: UnitVector3,
    ) -> Result<std::cmp::Ordering, GeometryError> {
        let [a, b, c] = points;
        Ok(crate::transform::orientation::determinant_sign([
            a.vector_to(b)?.to_array(),
            a.vector_to(c)?.to_array(),
            normal.as_vector().to_array(),
        ]))
    }
    /// Proper crossings of nonadjacent sides in the first-three-corner plane.
    /// Collinear contacts retain their point order; no model epsilon is used.
    pub fn face_quad_crosses(points: [Point3; 4]) -> Result<bool, GeometryError> {
        let [a, b, c, d] = points;
        let edge = a.vector_to(b)?;
        let normal = match edge.unit_cross_point_difference(a, c)? {
            Some(normal) => normal,
            None => match edge.unit_cross_point_difference(a, d)? {
                Some(normal) => normal,
                None => return Ok(false),
            },
        };
        let side = |a, b, c| Self::face_corner_side([a, b, c], normal);
        let crosses = |a, b, c, d| -> Result<bool, GeometryError> {
            let sides = [
                side(a, b, c)?,
                side(a, b, d)?,
                side(c, d, a)?,
                side(c, d, b)?,
            ];
            Ok(sides.iter().all(|s| *s != std::cmp::Ordering::Equal)
                && sides[0] != sides[1]
                && sides[2] != sides[3])
        };
        Ok(crosses(a, b, c, d)? || crosses(b, c, d, a)?)
    }
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
                // Preserve the polygon record, including a collapsed facet.
                // Mesh projection handles such facets as exact segments/points;
                // zero polygon normals remain explicit stored data.
                if (0..4).any(|i| points[i + 1..].contains(&points[i])) {
                    return Err(GeometryError::DegenerateQuad { face: 0 });
                }
                let edge = points[0].vector_to(points[1])?;
                if edge
                    .unit_cross_point_difference(points[0], points[2])?
                    .is_none()
                    && edge
                        .unit_cross_point_difference(points[0], points[3])?
                        .is_none()
                {
                    return Err(GeometryError::DegenerateQuad { face: 0 });
                }
                let mesh = Self::try_from_face_records(points, vec![MeshFace::Quad([0, 1, 2, 3])])?;
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

    #[test]
    fn proper_corner_crossings_distinguish_both_sides_without_a_model_tolerance() {
        let make = |x| {
            [
                point(0., 0., 0.),
                point(0., 3., 0.),
                point(x, -3., 0.),
                point(-3., 3., 0.),
            ]
        };
        for (x, crosses) in [(1e-7, false), (0., false), (-1e-7, true)] {
            let quad = make(x);
            assert_eq!(TriangleMesh::face_quad_crosses(quad).unwrap(), crosses);
            let tilted = quad.map(|p| point(p.x(), p.y(), p.x() + p.y()));
            assert_eq!(TriangleMesh::face_quad_crosses(tilted).unwrap(), crosses);
        }
    }
    #[test]
    fn stored_crossing_quads_keep_undefined_normals_and_exact_projection() {
        let quad = [
            point(4., 3., 0.),
            point(0., 0., 0.),
            point(0., 3., 0.),
            point(4., 0., 0.),
        ];
        let mesh = TriangleMesh::try_face_from_points(&quad, false).unwrap();
        assert!(mesh.polygon_face_normal(0).is_err());
        let query = point(2., 1.5, 2.);
        let index = MeshProjectionIndex::new(&mesh).unwrap();
        assert!(
            index
                .closest_point(query)
                .unwrap()
                .unwrap()
                .distance_to(point(2., 1.5, 0.))
                .unwrap()
                < 1e-12
        );
        let (welded, removed) = mesh.welded_vertices(std::f64::consts::PI).unwrap();
        assert_eq!(removed, 0);
        assert_eq!(welded.vertices(), mesh.vertices());
        assert_eq!(welded.faces(), mesh.faces());
    }
    #[test]
    fn scoped_welding_does_not_request_normals_of_unrelated_collapsed_faces() {
        let source = TriangleMesh::try_from_face_records(
            vec![point(0., 0., 0.), point(1., 0., 0.), point(2., 0., 0.)],
            vec![MeshFace::Triangle([0, 1, 2])],
        )
        .unwrap();
        let square = |x| {
            TriangleMesh::try_face_from_points(
                &[
                    point(x, 0., 0.),
                    point(x + 1., 0., 0.),
                    point(x + 1., 1., 0.),
                    point(x, 1., 0.),
                ],
                false,
            )
            .unwrap()
        };
        let a = square(10.);
        let b = square(11.);
        let mesh = TriangleMesh::try_append(&[&source, &a, &b]).unwrap();
        let (welded, removed) = mesh.welded_vertices_after(std::f64::consts::PI, 1).unwrap();
        assert_eq!(removed, 2);
        assert_eq!(welded.vertices().len(), 9);
        assert_eq!(welded.faces().len(), 3);
        assert!(welded.polygon_face_normal(0).is_err());
    }
}
