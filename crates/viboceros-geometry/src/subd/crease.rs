//! Hard-crease editing shares the public OpenNURBS component policy.
use super::*;

impl SubD {
    /// Source ordinals identify active control edges and vertices. All requested
    /// edits are prepared together; invalid input cannot modify this surface.
    /// Open boundary edges remain creased when removing creases.
    pub fn try_crease_components(
        &self,
        edges: &[usize],
        vertices: &[usize],
        crease: bool,
    ) -> Result<(Self, bool), GeometryError> {
        if edges.is_empty() && vertices.is_empty() {
            return Ok((self.clone(), false));
        }
        let (data, changed) =
            viboceros_opennurbs::subd_crease_components(self.archive(), edges, vertices, crease)?;
        if changed == 0 {
            Ok((self.clone(), false))
        } else {
            Self::from_native(data).map(|result| (result, true))
        }
    }
}
