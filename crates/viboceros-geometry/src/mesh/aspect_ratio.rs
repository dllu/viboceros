//! Mesh face aspect ratios from triangles along the shortest quad diagonal.

use super::*;

impl TriangleMesh {
    /// Longest triangle edge divided by its opposite altitude. For a quad,
    /// returns the larger ratio of its shorter-diagonal triangles. A
    /// collinear constituent triangle has infinite aspect ratio.
    pub fn face_aspect_ratio(&self, face_index: usize) -> Result<Real, GeometryError> {
        let face = self
            .faces
            .get(face_index)
            .ok_or(GeometryError::MeshFaceIndexOutOfRange {
                face: face_index,
                face_count: self.faces.len(),
            })?;
        let triangles = match *face {
            MeshFace::Triangle(triangle) => [Some(triangle), None],
            MeshFace::Quad(quad) => mass_triangles::split(&self.vertices, quad).map(Some),
        };
        let mut ratio: Real = 0.0;
        for triangle in triangles.into_iter().flatten() {
            ratio = ratio.max(triangle_aspect_ratio(
                triangle.map(|index| self.vertices[index as usize]),
            )?);
        }
        Ok(ratio)
    }
}

fn triangle_aspect_ratio([a, b, c]: [Point3; 3]) -> Result<Real, GeometryError> {
    if a == b || b == c || c == a {
        return Ok(Real::INFINITY);
    }
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
    fn square_ratio_matches_short_diagonal_triangles() {
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

    #[test]
    fn face_aspect_ratios_match_rhinocommon_observations() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tools/rhino_oracle/fixtures/mesh_face_metrics.json"
        ))
        .unwrap();
        let observed: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tools/rhino_oracle/observations/mesh_face_metrics.json"
        ))
        .unwrap();
        let operations = fixture["operations"].as_array().unwrap();
        let results = observed["results"].as_array().unwrap();
        assert_eq!(operations.len(), results.len());
        for (operation, observation) in operations.iter().zip(results) {
            let id = operation["id"].as_str().unwrap();
            assert_eq!(id, observation["id"].as_str().unwrap());
            let vertices = operation["vertices"]
                .as_array()
                .unwrap()
                .iter()
                .map(|array| {
                    let array = array.as_array().unwrap();
                    Point3::try_new(
                        array[0].as_f64().unwrap(),
                        array[1].as_f64().unwrap(),
                        array[2].as_f64().unwrap(),
                    )
                    .unwrap()
                })
                .collect();
            let faces = operation["faces"]
                .as_array()
                .unwrap()
                .iter()
                .map(|array| {
                    let values = array
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| value.as_u64().unwrap() as u32)
                        .collect::<Vec<_>>();
                    match values.as_slice() {
                        &[a, b, c] => MeshFace::Triangle([a, b, c]),
                        &[a, b, c, d] => MeshFace::Quad([a, b, c, d]),
                        _ => panic!("{id}: unsupported face"),
                    }
                })
                .collect();
            let mesh = TriangleMesh::try_new_faces(vertices, faces, Tolerance::DEFAULT).unwrap();
            let expected = observation["value"]["aspect_ratios"].as_array().unwrap();
            assert_eq!(mesh.face_count(), expected.len(), "{id}: face count");
            for (index, value) in expected.iter().enumerate() {
                let actual = mesh.face_aspect_ratio(index).unwrap();
                let expected = value.as_f64().unwrap();
                assert!(
                    (actual - expected).abs() <= 1e-8,
                    "{id}: face {index}: {actual} vs {expected}"
                );
            }
        }
    }
}
