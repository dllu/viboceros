//! Component-selected face merging reuses the public OpenNURBS implementation.
use super::*;
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
        if components.is_empty() {
            return Ok((self.clone(), 0));
        }
        let source = self.open_nurbs_data();
        let (data, removed) = viboceros_opennurbs::merge_mesh_components(&source, components)?;
        if data == source {
            return Ok((self.clone(), removed));
        }
        // Native merging may compact vertices or change facets, so use the
        // shared complete decoder rather than assuming a face-only conversion.
        let merged = Self::from_open_nurbs_data(data)?;
        merged.validate_face_geometry(Tolerance::MESH_VALIDATION)?;
        Ok((merged, removed))
    }
}

#[cfg(test)]
mod tests;
