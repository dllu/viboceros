//! Closed boundary meshing shares the constrained hole triangulator.
use super::*;
#[cfg(test)]
mod tests;

impl TriangleMesh {
    /// Triangulate a simple closed polyline without changing its coordinates.
    /// Closure follows the polyline's fixed coincidence policy, independently
    /// of document tolerance. Open or untriangulable boundaries return `None`.
    pub fn try_from_closed_polyline(polyline: &Polyline3) -> Result<Option<Self>, GeometryError> {
        if polyline.vertices().len() > MAX_MESH_PATCH_POINTS {
            return Err(GeometryError::MeshPatchLimit);
        }
        if !polyline.is_closed() {
            return Ok(None);
        }
        let vertices = &polyline.vertices()[..polyline.vertices().len() - 1];
        let Some(projected) = project_mesh_hole_boundary(vertices)? else {
            return Ok(None);
        };
        let Some(mut triangles) = triangulate_projected_mesh_hole(&projected)? else {
            return Ok(None);
        };
        // For a convex four-corner region, share the existing exact spatial
        // diagonal comparison, including its deterministic A-C tie.
        if vertices.len() == 4
            && (0..4).all(|i| {
                mesh_hole_cross(projected[i], projected[(i + 1) % 4], projected[(i + 2) % 4]) > 0.
            })
        {
            triangles = mass_triangles::split(vertices, [0, 1, 2, 3]).to_vec();
        }
        Ok(Some(Self::try_new(
            vertices.to_vec(),
            triangles,
            Tolerance::MESH_VALIDATION,
        )?))
    }
}
