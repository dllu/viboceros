//! Admission and editing of stored polygon records, including collapsed faces.

use super::*;

impl TriangleMesh {
    /// Admits stored triangle/quad records without imposing geometric validity.
    /// Points are finite by construction; every face must have distinct raw
    /// indices in range. Coincident or collinear positions are retained.
    /// Use [`Self::try_new_faces`] for newly generated, nondegenerate geometry.
    pub fn try_from_face_records(
        vertices: Vec<Point3>,
        faces: Vec<MeshFace>,
    ) -> Result<Self, GeometryError> {
        if faces.is_empty() {
            return Err(GeometryError::EmptyMesh);
        }
        if vertices
            .len()
            .checked_sub(1)
            .is_some_and(|last| u32::try_from(last).is_err())
        {
            return Err(GeometryError::TooManyMeshVertices);
        }
        let mut triangles = Vec::new();
        triangles
            .try_reserve(faces.len().saturating_mul(2))
            .map_err(|_| GeometryError::TooManyMeshFaces)?;
        for (index, face) in faces.iter().enumerate() {
            let indices = face.indices();
            for (corner, &vertex) in indices.iter().enumerate() {
                if vertex as usize >= vertices.len() {
                    return Err(match face {
                        MeshFace::Triangle(_) => GeometryError::InvalidTriangleIndex {
                            triangle: index,
                            vertex,
                        },
                        MeshFace::Quad(_) => GeometryError::InvalidQuadIndex {
                            face: index,
                            vertex,
                        },
                    });
                }
                if indices[..corner].contains(&vertex) {
                    return Err(match face {
                        MeshFace::Triangle(_) => {
                            GeometryError::DegenerateTriangle { triangle: index }
                        }
                        MeshFace::Quad(_) => GeometryError::DegenerateQuad { face: index },
                    });
                }
            }
            match *face {
                MeshFace::Triangle(triangle) => triangles.push(triangle),
                MeshFace::Quad([a, b, c, d]) => triangles.extend([[a, b, c], [a, c, d]]),
            }
        }
        Ok(Self {
            vertices,
            vertex_colors: None,
            faces,
            triangles,
            ngons: Vec::new(),
        })
    }

    /// Moves raw vertices one to one, retaining faces, colors and n-gons even
    /// when positions coincide. This allows a subsequent edit to heal a face.
    pub fn try_with_edited_vertices(&self, vertices: Vec<Point3>) -> Result<Self, GeometryError> {
        if vertices.len() != self.vertices.len() {
            return Err(GeometryError::Degenerate {
                context: "mesh vertex map size",
            });
        }
        Ok(Self {
            vertices,
            vertex_colors: self.vertex_colors.clone(),
            faces: self.faces.clone(),
            triangles: self.triangles.clone(),
            ngons: self.ngons.clone(),
        })
    }

    /// Checks geometric validity under the supplied tolerance. Stored records
    /// can exist even when this fails; normal-dependent operations may reject
    /// them explicitly rather than invent a direction.
    pub fn validate_face_geometry(&self, tolerance: Tolerance) -> Result<(), GeometryError> {
        validate_geometry(&self.vertices, &self.faces, tolerance)
    }
}

pub(super) fn validate_geometry(
    vertices: &[Point3],
    faces: &[MeshFace],
    tolerance: Tolerance,
) -> Result<(), GeometryError> {
    for (index, face) in faces.iter().copied().enumerate() {
        match face {
            MeshFace::Triangle(triangle) => {
                validate_triangle(vertices, triangle, index, false, tolerance)?
            }
            MeshFace::Quad([a, b, c, d]) => {
                validate_triangle(vertices, [a, b, c], index, true, tolerance)?;
                validate_triangle(vertices, [a, c, d], index, true, tolerance)?;
                if vertices[b as usize] == vertices[d as usize] {
                    return Err(GeometryError::DegenerateQuad { face: index });
                }
            }
        }
    }
    Ok(())
}
