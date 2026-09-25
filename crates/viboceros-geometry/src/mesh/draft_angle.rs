//! Select stored mesh faces by normal angle to a supplied viewward direction.

use super::{GeometryError, Real, TriangleMesh};
use crate::Vector3;

impl TriangleMesh {
    /// Face indices whose normal angle to `viewward` lies in the inclusive
    /// range, measured in degrees. `viewward` points from the model toward
    /// the viewer and need not already be normalized.
    pub fn faces_by_draft_angle(
        &self,
        viewward: Vector3,
        start_degrees: Real,
        end_degrees: Real,
    ) -> Result<Vec<usize>, GeometryError> {
        if !start_degrees.is_finite()
            || !end_degrees.is_finite()
            || start_degrees < 0.0
            || start_degrees > end_degrees
            || end_degrees > 180.0
        {
            return Err(GeometryError::InvalidMeshFaceAngleInterval);
        }
        let viewward = viewward.normalized_nonzero()?;
        let start_cosine = start_degrees.to_radians().cos();
        let end_cosine = end_degrees.to_radians().cos();
        let roundoff = 8.0 * Real::EPSILON;
        let mut selected = Vec::new();
        for (index, normal) in self.polygon_face_normals()?.into_iter().enumerate() {
            let cosine = normal
                .as_vector()
                .dot(viewward.as_vector())?
                .clamp(-1.0, 1.0);
            if cosine <= start_cosine + roundoff && cosine + roundoff >= end_cosine {
                selected.push(index);
            }
        }
        Ok(selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MeshFace, Point3, Tolerance};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn three_directions() -> TriangleMesh {
        TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(3.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(3.0, 1.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(0.0, 3.0, 1.0),
            ],
            vec![
                MeshFace::Triangle([0, 1, 2]),
                MeshFace::Triangle([3, 5, 4]),
                MeshFace::Triangle([6, 7, 8]),
            ],
            Tolerance::default(),
        )
        .unwrap()
    }

    #[test]
    fn includes_exact_angle_boundaries_and_oriented_faces() {
        let mesh = three_directions();
        let toward_top = Vector3::try_new(0.0, 0.0, 5.0).unwrap();
        assert_eq!(
            mesh.faces_by_draft_angle(toward_top, 0.0, 0.0).unwrap(),
            vec![0]
        );
        assert_eq!(
            mesh.faces_by_draft_angle(toward_top, 90.0, 90.0).unwrap(),
            vec![2]
        );
        assert_eq!(
            mesh.faces_by_draft_angle(toward_top, 180.0, 180.0).unwrap(),
            vec![1]
        );
        assert_eq!(
            mesh.faces_by_draft_angle(toward_top, 0.0, 180.0).unwrap(),
            vec![0, 1, 2]
        );
        let toward_bottom = Vector3::try_new(0.0, 0.0, -1.0).unwrap();
        assert_eq!(
            mesh.faces_by_draft_angle(toward_bottom, 0.0, 0.0).unwrap(),
            vec![1]
        );
    }

    #[test]
    fn rejects_invalid_ranges_and_zero_view_direction() {
        let mesh = three_directions();
        let toward_top = Vector3::try_new(0.0, 0.0, 1.0).unwrap();
        for (start, end) in [(-1.0, 90.0), (91.0, 90.0), (0.0, 181.0), (Real::NAN, 90.0)] {
            assert!(matches!(
                mesh.faces_by_draft_angle(toward_top, start, end),
                Err(GeometryError::InvalidMeshFaceAngleInterval)
            ));
        }
        assert!(
            mesh.faces_by_draft_angle(Vector3::try_new(0.0, 0.0, 0.0).unwrap(), 0.0, 90.0)
                .is_err()
        );
    }
}
