//! Public OpenNURBS planarity measurements for an individual stored quad.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshQuadPlanarity {
    /// Range of signed distances to the diagonal-normal face plane, in model units.
    pub distance: Real,
    /// Largest angle between opposite corner normals, in radians.
    pub angle: Real,
}
impl TriangleMesh {
    /// Measure one stored quad, independently of document tolerance and n-gons.
    /// Only its four points cross the native boundary; source storage is unchanged.
    pub fn quad_planarity(&self, face: usize) -> Result<MeshQuadPlanarity, GeometryError> {
        let record = self
            .faces
            .get(face)
            .ok_or(GeometryError::MeshFaceIndexOutOfRange {
                face,
                face_count: self.faces.len(),
            })?;
        let MeshFace::Quad(quad) = record else {
            return Err(GeometryError::MeshFaceNotQuad { face });
        };
        let points = quad.map(|i| self.vertices[i as usize].to_array());
        let (distance, angle) = viboceros_opennurbs::quad_planarity(&points)?;
        Ok(MeshQuadPlanarity { distance, angle })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quad_planarity_is_scale_invariant_and_rejects_triangles_and_bad_indices() {
        let points = [[0., 0., 0.], [2., 0., 0.], [2., 1., 0.], [0., 1., 0.4]];
        let make = |scale: f64| {
            TriangleMesh::try_from_face_records(
                points
                    .iter()
                    .map(|p| Point3::try_from(p.map(|v| v * scale)).unwrap())
                    .collect(),
                vec![MeshFace::Quad([0, 1, 2, 3])],
            )
            .unwrap()
        };
        let a = make(1.).quad_planarity(0).unwrap();
        assert!((a.distance - 0.2 / (1.05f64).sqrt()).abs() < 1e-14);
        for scale in [1e-8, 1e-3, 10., 1e8] {
            let b = make(scale).quad_planarity(0).unwrap();
            assert!((b.distance / scale - a.distance).abs() < 1e-14);
            assert!((b.angle - a.angle).abs() < 1e-14);
        }
        let triangle = TriangleMesh::try_from_face_records(
            points[..3]
                .iter()
                .copied()
                .map(|p| Point3::try_from(p).unwrap())
                .collect(),
            vec![MeshFace::Triangle([0, 1, 2])],
        )
        .unwrap();
        assert!(matches!(
            triangle.quad_planarity(0),
            Err(GeometryError::MeshFaceNotQuad { .. })
        ));
        assert!(matches!(
            triangle.quad_planarity(1),
            Err(GeometryError::MeshFaceIndexOutOfRange { .. })
        ));
    }
}
