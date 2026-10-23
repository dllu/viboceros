//! Scale-safe directions for stored polygons and triangulated facets.
use super::{GeometryError, MeshFace, TriangleMesh, UnitVector3};
use crate::{Real, Vector3};

#[cfg(test)]
mod tests;

impl TriangleMesh {
    /// Reconstruct stored vertex normals through public OpenNURBS face and
    /// vertex calculations, regardless of an existing or absent channel.
    /// Unused vertices are removed in source order, remapping their channels
    /// and n-gons. Face winding and coincident referenced raw seams survive.
    /// Collapsed faces contribute zero; cancelling sums use the world Z axis.
    pub fn rebuilt_normals(&self) -> Result<Self, GeometryError> {
        let (mesh, _) = self.culled_unused_vertices();
        let data = viboceros_opennurbs::recompute_mesh_normals(&mesh.open_nurbs_data())?;
        let normals = data.vertex_normals.ok_or(GeometryError::Degenerate {
            context: "missing rebuilt mesh normals",
        })?;
        mesh.try_with_vertex_normals(Some(
            normals
                .into_iter()
                .map(Vector3::try_from)
                .collect::<Result<_, _>>()?,
        ))
    }

    /// Computes unit vertex normals from raw vertex sharing without changing
    /// the stored normal channel. Each incident polygon contributes its unit
    /// normal once, including non-planar quads as one polygon.
    ///
    /// Coincident vertices remain independent. Collapsed polygons contribute
    /// nothing; unused vertices and cancelling sums use the world Z direction.
    /// Work and storage are linear in the vertex and face counts.
    pub fn computed_vertex_normals(&self) -> Result<Vec<Vector3>, GeometryError> {
        self.vertex_normals_from_faces(true)
    }

    /// Initial float normals following the owned OffsetMesh normal path.
    pub(super) fn raw_vertex_normals(&self) -> Result<Vec<Vector3>, GeometryError> {
        self.vertex_normals_from_faces(false)
    }

    fn vertex_normals_from_faces(
        &self,
        skip_collapsed: bool,
    ) -> Result<Vec<Vector3>, GeometryError> {
        let mut sums = vec![[0.0_f32; 3]; self.vertices.len()];
        for &face in self.faces.iter().rev() {
            let normal = match self.normal_for_face(face) {
                Ok(normal) => normal,
                Err(GeometryError::Degenerate { .. }) if skip_collapsed => continue,
                Err(error) => return Err(error),
            };
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
