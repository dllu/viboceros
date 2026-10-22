//! Hard-crease editing shares the public OpenNURBS component policy.
use super::*;

impl SubD {
    /// WeldVertices relaxes interior control vertices only; boundary corners
    /// and their incident creases stay intact. UnweldVertex makes corners.
    /// Both commands preserve the control topology and all vertex ordinals.
    pub fn try_vertex_seams(
        &self,
        vertices: &[usize],
        weld: bool,
    ) -> Result<(Self, bool), GeometryError> {
        if vertices.iter().any(|&i| i >= self.control_points().len()) {
            return Err(viboceros_opennurbs::SubDError(
                "Vertex seam index outside the SubD control vertex table".into(),
            )
            .into());
        }
        if !weld {
            return self.try_crease_components(&[], vertices, true);
        }
        if vertices.iter().all(|&i| self.vertex_tags()[i] == 1) {
            return Ok((self.clone(), false));
        }
        let mut boundary = vec![false; self.control_points().len()];
        for (&[a, b], &count) in self.edges().iter().zip(self.edge_face_counts()) {
            if count != 2 {
                boundary[a as usize] = true;
                boundary[b as usize] = true;
            }
        }
        let interior = vertices
            .iter()
            .copied()
            .filter(|&i| !boundary[i])
            .collect::<Vec<_>>();
        self.try_crease_components(&[], &interior, false)
    }

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
