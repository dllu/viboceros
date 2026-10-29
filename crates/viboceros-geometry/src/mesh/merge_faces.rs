//! Component-selected face merging reuses the public OpenNURBS implementation.
use super::*;
#[derive(Clone, Debug)]
pub struct MeshFaceMergeResult {
    pub mesh: TriangleMesh,
    pub removed_faces: usize,
    pub merged_faces: Vec<usize>,
}
impl TriangleMesh {
    /// Coalesce connected logical face sets without a coplanarity restriction.
    /// New n-gons require one boundary loop; hole-bearing regions are not
    /// represented as a new single face. Explicit face picks can weld seams.
    /// Vertex and face indices use original raw storage; edge endpoint pairs
    /// identify topology edges without relying on a foreign edge ordering.
    /// The count is the decrease in logical faces, including n-gon overlays.
    pub fn merge_face_components(
        &self,
        components: &[MeshMergeComponent],
    ) -> Result<(Self, usize), GeometryError> {
        let out = self.merge_face_components_mapped(components)?;
        Ok((out.mesh, out.removed_faces))
    }
    /// Also reports the output facets composing newly merged logical faces.
    pub fn merge_face_components_mapped(
        &self,
        components: &[MeshMergeComponent],
    ) -> Result<MeshFaceMergeResult, GeometryError> {
        if components.is_empty() {
            return Ok(MeshFaceMergeResult {
                mesh: self.clone(),
                removed_faces: 0,
                merged_faces: Vec::new(),
            });
        }
        let source = self.open_nurbs_data();
        let out = viboceros_opennurbs::merge_mesh_components_with_faces(&source, components)?;
        let mesh = if out.mesh == source {
            self.clone()
        } else {
            let merged = Self::from_open_nurbs_data(out.mesh)?;
            merged.validate_face_geometry(Tolerance::MESH_VALIDATION)?;
            merged
        };
        Ok(MeshFaceMergeResult {
            mesh,
            removed_faces: out.removed_faces,
            merged_faces: out.merged_faces,
        })
    }
}

#[cfg(test)]
mod tests;
