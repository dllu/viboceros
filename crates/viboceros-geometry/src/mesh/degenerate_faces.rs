//! Immutable cleanup using the native face/vertex deletion executive.
use super::*;
#[derive(Clone, Debug, PartialEq)]
pub struct MeshFaceCullResult {
    /// None means that every face was removed.
    pub mesh: Option<TriangleMesh>,
    pub removed_faces: usize,
    pub repaired_faces: usize,
    pub removed_vertices: usize,
}
impl MeshFaceCullResult {
    pub fn changed(&self) -> bool {
        self.removed_faces > 0 || self.repaired_faces > 0 || self.removed_vertices > 0
    }
}
impl TriangleMesh {
    /// Cull degenerate faces, repair collapsed quads when possible, and remove
    /// unused vertices. Uses native double coordinates and fixed native cutoffs;
    /// the document tolerance does not change this classification.
    pub fn culled_degenerate_faces(&self) -> Result<MeshFaceCullResult, GeometryError> {
        let result = viboceros_opennurbs::cull_degenerate_mesh_faces(&self.open_nurbs_data())?;
        let changed =
            result.removed_faces > 0 || result.repaired_faces > 0 || result.removed_vertices > 0;
        let mesh = if !changed {
            Some(self.clone())
        } else if result.mesh.faces.is_empty() {
            None
        } else {
            let mut data = result.mesh;
            let ngons = std::mem::take(&mut data.ngons)
                .into_iter()
                .map(|g| MeshNgon::from_parts(g.vertices, g.faces))
                .collect::<Vec<_>>();
            Some(Self::from_open_nurbs_data(data)?.retain_valid_ngons_for_same_faces(&ngons))
        };
        Ok(MeshFaceCullResult {
            mesh,
            removed_faces: result.removed_faces,
            repaired_faces: result.repaired_faces,
            removed_vertices: result.removed_vertices,
        })
    }
}
#[cfg(test)]
mod tests;
