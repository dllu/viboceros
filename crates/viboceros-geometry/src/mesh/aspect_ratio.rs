//! Triangle aspect ratio, extended to all triples of a quad's vertices.

use super::*;

impl TriangleMesh {
    /// Longest triangle edge divided by its opposite altitude. For a quad,
    /// returns the maximum over all four vertex triples. A collinear triple
    /// has infinite aspect ratio.
    pub fn face_aspect_ratio(&self, face_index: usize) -> Result<Real, GeometryError> {
        let face = self
            .faces
            .get(face_index)
            .ok_or(GeometryError::MeshFaceIndexOutOfRange {
                face: face_index,
                face_count: self.faces.len(),
            })?;
        let indices = face.indices();
        let point = |index| self.vertices[indices[index] as usize];
        let mut ratio: Real = 0.0;
        let triples: &[[usize; 3]] = if indices.len() == 3 {
            &[[0, 1, 2]]
        } else {
            &[[0, 1, 2], [0, 1, 3], [0, 2, 3], [1, 2, 3]]
        };
        for &[a, b, c] in triples {
            ratio = ratio.max(triangle_aspect_ratio([point(a), point(b), point(c)])?);
        }
        Ok(ratio)
    }
}

fn triangle_aspect_ratio([a, b, c]: [Point3; 3]) -> Result<Real, GeometryError> {
    let lengths = [a.distance_to(b)?, b.distance_to(c)?, c.distance_to(a)?];
    let longest = lengths
        .iter()
        .enumerate()
        .max_by(|left, right| left.1.total_cmp(right.1))
        .expect("a triangle has three sides")
        .0;
    let (base, first, second, vertex) = match longest {
        0 => (lengths[0], lengths[2], lengths[1], c),
        1 => (lengths[1], lengths[0], lengths[2], a),
        _ => (lengths[2], lengths[1], lengths[0], b),
    };
    let (first_point, second_point) = match longest {
        0 => (a, b),
        1 => (b, c),
        _ => (c, a),
    };
    if let (Ok(base_vector), Ok(first_vector), Ok(second_vector)) = (
        first_point.vector_to(second_point),
        vertex.vector_to(first_point),
        vertex.vector_to(second_point),
    ) && let (Ok(base_squared), Ok(area)) = (
        base_vector.dot(base_vector),
        first_vector.half_cross_length(second_vector),
    ) && base_squared > 0.0
        && area > 0.0
    {
        let ratio = base_squared / (2.0 * area);
        if ratio.is_finite() {
            return Ok(ratio);
        }
    }
    let sine = vertex
        .direction_to(first_point)?
        .as_vector()
        .cross(vertex.direction_to(second_point)?.as_vector())?
        .length()?;
    if sine == 0.0 {
        return Ok(Real::INFINITY);
    }
    Ok((base / first) * (base / second) / sine)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.0).unwrap()
    }

    #[test]
    fn square_uses_the_worst_of_four_triangle_triples() {
        let mesh = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0),
                point(1.0, 0.0),
                point(1.0, 1.0),
                point(0.0, 1.0),
            ],
            vec![MeshFace::Quad([0, 1, 2, 3])],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!((mesh.face_aspect_ratio(0).unwrap() - 2.0).abs() < 1e-14);
    }

    #[test]
    fn skinny_triangle_and_huge_triangle_keep_scale_independent_ratio() {
        let skinny = TriangleMesh::try_new(
            vec![point(0.0, 0.0), point(10.0, 0.0), point(0.0, 1.0)],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!((skinny.face_aspect_ratio(0).unwrap() - 10.1).abs() < 1e-12);
        let huge = TriangleMesh::try_new(
            vec![point(0.0, 0.0), point(1e200, 0.0), point(0.0, 1e200)],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!((huge.face_aspect_ratio(0).unwrap() - 2.0).abs() < 1e-14);
    }

    #[test]
    fn nonvalid_index_and_collinear_quad_triple_are_explicit() {
        let mesh = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0),
                point(1.0, 0.0),
                point(1.0, 1.0),
                point(2.0, 0.0),
            ],
            vec![MeshFace::Quad([0, 1, 2, 3])],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(mesh.face_aspect_ratio(0).unwrap(), Real::INFINITY);
        assert_eq!(
            mesh.face_aspect_ratio(1),
            Err(GeometryError::MeshFaceIndexOutOfRange {
                face: 1,
                face_count: 1,
            })
        );
    }
}
