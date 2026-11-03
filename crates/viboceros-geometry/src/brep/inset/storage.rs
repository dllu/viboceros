//! Conservative output-storage admission before supporting surfaces are copied.
use super::*;
impl Brep {
    pub(super) fn inset_storage_estimate(
        &self,
        selected: &BTreeSet<usize>,
        crease: bool,
    ) -> Result<usize, GeometryError> {
        // Border faces retain the complete supporting control net. Charge
        // those copies before construction, including highly refined planes.
        let mut bytes = self.vertices.len().saturating_mul(32);
        for edge in &self.edges {
            bytes = bytes
                .saturating_add(256)
                .saturating_add(edge.curve.control_points().len().saturating_mul(32))
                .saturating_add(edge.curve.knots().len().saturating_mul(8));
        }
        for (i, face) in self.faces.iter().enumerate() {
            let surface = face
                .surface
                .control_points()
                .len()
                .saturating_mul(32)
                .saturating_add(face.surface.knots_u().len().saturating_mul(8))
                .saturating_add(face.surface.knots_v().len().saturating_mul(8))
                .saturating_add(256);
            bytes = bytes.saturating_add(surface);
            let mut count = 0usize;
            for boundary in &face.loops {
                bytes = bytes.saturating_add(128);
                count = count.saturating_add(boundary.trims.len());
                for trim in &boundary.trims {
                    bytes = bytes
                        .saturating_add(256)
                        .saturating_add(trim.curve.control_points().len().saturating_mul(24))
                        .saturating_add(trim.curve.knots().len().saturating_mul(8));
                }
            }
            if selected.contains(&i) {
                let borders = if crease { count } else { face.loops.len() };
                bytes = bytes
                    .saturating_add(surface.saturating_mul(borders))
                    .saturating_add(count.saturating_mul(2048));
            }
            if bytes > 64 * 1024 * 1024 {
                return Err(invalid("planar inset exceeds its output-storage budget"));
            }
        }
        Ok(bytes)
    }
}
