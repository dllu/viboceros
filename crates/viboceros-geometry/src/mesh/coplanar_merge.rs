//! Native logical mesh-face coalescing without a separate triangulation engine.
use super::*;
impl TriangleMesh {
    /// Merge adjacent coplanar logical mesh faces. Native face sets can replace
    /// two triangles by a quad; larger regions and holes use n-gon overlays.
    /// The source remains unchanged. Zero removed faces means a no-op.
    pub fn merge_coplanar_faces(
        &self,
        tolerance: Tolerance,
    ) -> Result<(Self, usize), GeometryError> {
        if self.faces.len() < 2 {
            return Ok((self.clone(), 0));
        }
        let (data, removed) = viboceros_opennurbs::merge_coplanar_mesh(
            &self.open_nurbs_data(),
            tolerance.absolute(),
            tolerance.angular(),
        )?;
        if removed == 0 {
            return Ok((self.clone(), 0));
        }
        Ok((Self::from_open_nurbs_data(data)?, removed))
    }
    pub(super) fn open_nurbs_data(&self) -> viboceros_opennurbs::MeshData {
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
        .try_with_ngons(
            data.ngons
                .into_iter()
                .map(|g| MeshNgon::from_parts(g.vertices, g.faces))
                .collect(),
        )
    }
}
#[cfg(test)]
mod tests;
