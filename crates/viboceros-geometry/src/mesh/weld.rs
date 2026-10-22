//! Whole-mesh seam welding and incremental patch welding.
use super::*;
mod merge;
use merge::WeldState;

impl TriangleMesh {
    /// Welds coincident edge endpoints whose incident face normals fall
    /// within the supplied angular tolerance.
    ///
    /// Only vertices paired along an exact-location topology edge can merge;
    /// a coincident vertex-only contact remains distinct. Established raw
    /// representatives are reused and compacted in source order.
    /// Non-manifold edges consider their first two face uses. Stored UVs,
    /// colors and normal directions are blended; opposing stored normals leave
    /// the input unchanged. Native connected-face traversal remains incomplete.
    pub fn welded_vertices(
        &self,
        angle_tolerance_radians: Real,
    ) -> Result<(Self, usize), GeometryError> {
        self.welded_vertices_after(angle_tolerance_radians, 0)
    }

    /// Weld only seams incident to faces at or after `first_new_face`.
    /// Existing unrelated unwelded edges retain their raw vertex sharing.
    pub fn welded_vertices_after(
        &self,
        angle_tolerance_radians: Real,
        first_new_face: usize,
    ) -> Result<(Self, usize), GeometryError> {
        if !(angle_tolerance_radians.is_finite()
            && (0.0..=std::f64::consts::PI).contains(&angle_tolerance_radians))
        {
            return Err(GeometryError::InvalidMeshWeldAngle);
        }
        let data = self.topology_data();
        let mut face_normals = vec![None; self.faces.len()];
        let mut normal = |face: usize| -> Result<UnitVector3, GeometryError> {
            if let Some(value) = face_normals[face] {
                return Ok(value);
            }
            let value = self.polygon_face_normal(face)?;
            face_normals[face] = Some(value);
            Ok(value)
        };
        let minimum_dot = angle_tolerance_radians.cos();
        let mut state = WeldState::new(self);
        for incidence in data.edges.values() {
            let mut uses = incidence.uses();
            let Some(first) = uses.next() else {
                continue;
            };
            let Some(second) = uses.next() else {
                continue;
            };
            if first.face < first_new_face && second.face < first_new_face {
                continue;
            }
            let dot = normal(first.face)?
                .as_vector()
                .dot(normal(second.face)?.as_vector())?
                .clamp(-1., 1.);
            if dot < minimum_dot {
                continue;
            }
            let uses = [first, second];
            let descending = uses.map(|u| {
                let face = self.faces[u.face].indices();
                face[u.side] > face[(u.side + 1) % face.len()]
            });
            if !state.merge_edge(uses, descending)? {
                return Ok((self.clone(), 0));
            }
        }

        Ok(state.finish(self))
    }
}
