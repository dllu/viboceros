//! Scale-safe directions for stored polygons and triangulated facets.
use super::{GeometryError, MeshFace, TriangleMesh, UnitVector3};

#[cfg(test)]
mod tests;

impl TriangleMesh {
    /// Unit normals in stored polygon-face order (one per triangle or quad).
    /// Non-planar quads use the oriented cross product of their diagonals,
    /// not an unweighted average of their triangulation's unit normals.
    pub fn polygon_face_normals(&self) -> Result<Vec<UnitVector3>, GeometryError> {
        self.faces
            .iter()
            .map(|&face| self.normal_for_face(face))
            .collect()
    }

    /// Unit normal of a triangulated facet, indexed into [`Self::triangles`].
    /// For one normal per original polygon, use [`Self::polygon_face_normals`].
    pub fn face_normal(&self, index: usize) -> Result<UnitVector3, GeometryError> {
        let triangle = self
            .triangles
            .get(index)
            .copied()
            .ok_or(GeometryError::TriangleIndexOutOfRange { triangle: index })?;
        self.normal_for_face(MeshFace::Triangle(triangle))
    }

    fn normal_for_face(&self, face: MeshFace) -> Result<UnitVector3, GeometryError> {
        let (first, second) = match face {
            MeshFace::Triangle([a, b, c]) => ([a, b], [a, c]),
            MeshFace::Quad([a, b, c, d]) => ([a, c], [b, d]),
        };
        // Only the direction is needed. Normalize each edge/diagonal before
        // crossing, so a valid face need not have a representable area.
        let direction = |[start, end]: [u32; 2]| {
            self.vertices[start as usize]
                .vector_to(self.vertices[end as usize])?
                .normalized_nonzero()
        };
        direction(first)?
            .as_vector()
            .cross(direction(second)?.as_vector())?
            .normalized_nonzero()
    }
}
