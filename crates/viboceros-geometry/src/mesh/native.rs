//! Shared mesh conversion at the safe OpenNURBS boundary.
use super::*;
impl TriangleMesh {
    pub(crate) fn open_nurbs_data(&self) -> viboceros_opennurbs::MeshData {
        viboceros_opennurbs::MeshData {
            vertices: self.vertices.iter().map(|p| p.to_array()).collect(),
            faces: self
                .faces
                .iter()
                .map(|f| {
                    let p = f.indices();
                    [p[0], p[1], p[2], *p.get(3).unwrap_or(&p[2])]
                })
                .collect(),
            colors: self.vertex_colors.clone(),
            texture_coordinates: self.texture_coordinates().map(<[_]>::to_vec),
            vertex_normals: self
                .vertex_normals()
                .map(|n| n.iter().map(|v| v.to_array()).collect()),
            ngons: self
                .ngons
                .iter()
                .map(|g| viboceros_opennurbs::Ngon {
                    vertices: g.vertices.clone(),
                    faces: g.faces.clone(),
                })
                .collect(),
        }
    }
    pub(super) fn from_open_nurbs_data(
        data: viboceros_opennurbs::MeshData,
    ) -> Result<Self, GeometryError> {
        Self::try_from_face_records(
            data.vertices
                .into_iter()
                .map(Point3::try_from)
                .collect::<Result<_, _>>()?,
            data.faces
                .into_iter()
                .map(|f| {
                    if f[2] == f[3] {
                        MeshFace::Triangle([f[0], f[1], f[2]])
                    } else {
                        MeshFace::Quad(f)
                    }
                })
                .collect(),
        )?
        .try_with_vertex_colors(data.colors)?
        .try_with_texture_coordinates(data.texture_coordinates)?
        .try_with_vertex_normals(
            data.vertex_normals
                .map(|n| {
                    n.into_iter()
                        .map(Vector3::try_from)
                        .collect::<Result<_, _>>()
                })
                .transpose()?,
        )?
        .try_with_ngons(
            data.ngons
                .into_iter()
                .map(|g| MeshNgon::from_parts(g.vertices, g.faces))
                .collect(),
        )
    }
}
