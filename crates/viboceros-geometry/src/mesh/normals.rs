//! Scale-safe directions for stored polygons and triangulated facets.
use super::{GeometryError, MeshFace, TriangleMesh, UnitVector3};
use crate::{Real, Vector3};

#[cfg(test)]
mod tests;

impl TriangleMesh {
    /// Initial float normals following the owned OffsetMesh normal path.
    pub(super) fn raw_vertex_normals(&self) -> Result<Vec<Vector3>, GeometryError> {
        let normals = self.polygon_face_normals()?;
        let mut sums = vec![[0.0_f32; 3]; self.vertices.len()];
        for (face, normal) in self.faces.iter().zip(normals).rev() {
            for &index in face.indices() {
                let index = index as usize;
                for (sum, coordinate) in sums[index].iter_mut().zip(normal.as_vector().to_array()) {
                    *sum += coordinate as f32;
                }
            }
        }
        let world_z = Vector3::try_new(0.0, 0.0, 1.0)?;
        sums.iter()
            .map(|&sum| {
                let vector = Vector3::try_new(sum[0] as Real, sum[1] as Real, sum[2] as Real)?;
                let normalized = vector
                    .normalized_nonzero()
                    .map(UnitVector3::as_vector)
                    .unwrap_or(world_z);
                Vector3::try_from(
                    normalized
                        .to_array()
                        .map(|coordinate| coordinate as f32 as Real),
                )
            })
            .collect::<Result<Vec<Vector3>, GeometryError>>()
    }

    /// Unit normal of a stored triangle or quad, indexed in polygon-face order.
    pub fn polygon_face_normal(&self, index: usize) -> Result<UnitVector3, GeometryError> {
        let face =
            self.faces
                .get(index)
                .copied()
                .ok_or(GeometryError::MeshFaceIndexOutOfRange {
                    face: index,
                    face_count: self.faces.len(),
                })?;
        self.normal_for_face(face)
    }

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

    pub(super) fn normal_for_face(&self, face: MeshFace) -> Result<UnitVector3, GeometryError> {
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
