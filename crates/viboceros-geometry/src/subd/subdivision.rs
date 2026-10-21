//! Immutable, bounded global and local subdivision through OpenNURBS.
use super::*;
impl SubD {
    /// True when an active edge belongs to exactly one face.
    pub fn has_boundary(&self) -> bool {
        self.data.boundary_edge_count != 0
    }
    /// Refine the complete active net without changing its limit surface.
    pub fn subdivided(&self, count: u32) -> Result<Self, GeometryError> {
        Self::from_native(viboceros_opennurbs::subd_subdivide(
            self.archive(),
            count,
            None,
        )?)
    }
    /// Refine selected active control faces; adjacent unselected polygons keep
    /// shared split edges. Indices are ordinals, independent of native face IDs.
    pub fn subdivided_faces(&self, faces: &[usize]) -> Result<Self, GeometryError> {
        self.subdivided_faces_with_selection(faces)
            .map(|(subd, _)| subd)
    }
    /// Returns fresh child face ordinals in the same canonical table that the
    /// archive restores. Metadata is kept outside the immutable geometry value.
    pub fn subdivided_faces_with_selection(
        &self,
        faces: &[usize],
    ) -> Result<(Self, Vec<usize>), GeometryError> {
        let mut data = viboceros_opennurbs::subd_subdivide(self.archive(), 1, Some(faces))?;
        let selected = std::mem::take(&mut data.selected_faces)
            .into_iter()
            .map(|f| f as usize)
            .collect();
        Ok((Self::from_native(data)?, selected))
    }
}
