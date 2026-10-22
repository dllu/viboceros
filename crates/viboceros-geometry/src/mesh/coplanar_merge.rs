//! Native logical mesh-face coalescing without a separate triangulation engine.
use super::*;
impl TriangleMesh {
    /// Merge only selected logical faces, or the connected coplanar region of
    /// one seed. Indices reference the original underlying face table.
    pub fn merge_coplanar_faces_at(
        &self,
        faces: &[usize],
        region: bool,
        tolerance: Tolerance,
    ) -> Result<(Self, usize), GeometryError> {
        let (data, n) = viboceros_opennurbs::merge_selected_coplanar_mesh(
            &self.open_nurbs_data(),
            faces,
            region,
            tolerance.absolute(),
            tolerance.angular(),
        )?;
        if n == 0 {
            return Ok((self.clone(), 0));
        }
        Ok((Self::from_open_nurbs_data(data)?, n))
    }

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
}

#[cfg(test)]
mod tests;
