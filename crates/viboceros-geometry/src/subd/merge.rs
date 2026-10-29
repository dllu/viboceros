//! Face region merging retains the archival component identities and properties.
use super::*;
pub use viboceros_opennurbs::SubDMergeComponent;

#[derive(Clone, Debug)]
pub struct SubDFaceMergeResult {
    pub subd: SubD,
    pub removed_faces: usize,
    /// Result control-face ordinals, independent of display tessellation.
    pub merged_faces: Vec<usize>,
}
impl SubD {
    pub fn merge_face_components(
        &self,
        input: &[SubDMergeComponent],
    ) -> Result<SubDFaceMergeResult, GeometryError> {
        if input.is_empty() {
            return Ok(SubDFaceMergeResult {
                subd: self.clone(),
                removed_faces: 0,
                merged_faces: Vec::new(),
            });
        }
        let (data, removed_faces) = viboceros_opennurbs::subd_merge_faces(self.archive(), input)?;
        let merged_faces = data.selected_faces.iter().map(|&i| i as usize).collect();
        Ok(SubDFaceMergeResult {
            subd: if removed_faces == 0 {
                self.clone()
            } else {
                Self::from_native(data)?
            },
            removed_faces,
            merged_faces,
        })
    }
}

#[cfg(test)]
mod tests;
