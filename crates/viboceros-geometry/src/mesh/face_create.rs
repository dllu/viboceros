//! Direct mesh faces share polygon triangulation and validated n-gon overlays.
use super::*;

impl TriangleMesh {
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
            4 => Self::try_new_faces(
                points,
                vec![MeshFace::Quad([0, 1, 2, 3])],
                Tolerance::MESH_VALIDATION,
            ),
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
}
