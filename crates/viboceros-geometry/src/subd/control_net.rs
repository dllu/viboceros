//! Control-net topology and vertex attributes, separate from a limit tessellation.
use super::*;
pub use viboceros_opennurbs::SubDControlNetPriority;

impl SubD {
    /// Geometry priority preserves control topology and crease seams. Texture
    /// priority transfers packed texture corners and duplicates face vertices.
    /// Both modes retain the public conversion's n-gons and stored attributes.
    /// Conversion changes only a private decoded copy of the source archive.
    pub fn control_net_mesh(
        &self,
        priority: SubDControlNetPriority,
    ) -> Result<TriangleMesh, GeometryError> {
        let data = viboceros_opennurbs::subd_control_mesh_with_priority(self.archive(), priority)?;
        let points = data
            .points
            .into_iter()
            .map(Point3::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let faces = data
            .faces
            .into_iter()
            .map(|f| match f[..] {
                [a, b, c] => MeshFace::Triangle([a, b, c]),
                [a, b, c, d] => MeshFace::Quad([a, b, c, d]),
                _ => unreachable!("native control mesh contains triangles and quads"),
            })
            .collect();
        let mesh = TriangleMesh::try_from_face_records(points, faces)?.try_with_ngons(
            data.mesh_ngons
                .into_iter()
                .map(|g| crate::MeshNgon::from_parts(g.vertices, g.faces))
                .collect(),
        )?;
        let vertex_normals = data
            .mesh_vertex_normals
            .into_iter()
            .map(Vector3::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        mesh.try_with_texture_coordinates(
            (!data.mesh_texture_coordinates.is_empty()).then_some(data.mesh_texture_coordinates),
        )?
        .try_with_vertex_normals((!vertex_normals.is_empty()).then_some(vertex_normals))
    }
}
