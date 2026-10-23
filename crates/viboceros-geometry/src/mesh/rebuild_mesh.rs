//! Mesh reconstruction with native normal rebuilding and channel policy.
use super::*;

impl TriangleMesh {
    /// Rebuild a mesh while optionally retaining UV and color channels.
    /// Unused vertices compact in source order; coincident referenced vertices
    /// stay distinct. All n-gon overlays are removed and normals are rebuilt.
    /// The source mesh and retained double-precision positions are unchanged.
    pub fn rebuilt(
        &self,
        preserve_texture_coordinates: bool,
        preserve_vertex_colors: bool,
    ) -> Result<Self, GeometryError> {
        let mut mesh = self.rebuilt_normals()?;
        mesh.ngons.clear();
        if !preserve_texture_coordinates {
            mesh.vertex_channels.uv = None;
        }
        if !preserve_vertex_colors {
            mesh.vertex_colors = None;
        }
        Ok(mesh)
    }
}
