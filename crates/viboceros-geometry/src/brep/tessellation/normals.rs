use super::*;
use crate::{SurfaceTessellation, tessellation::triangle_corners};

impl Brep {
    /// Audited tessellation with per-corner surface normals and native UV/face
    /// provenance. Normals follow each face's orientation, including hard joins.
    /// Regular corners survive even when singular corners have no usable limit.
    pub fn tessellate_with_normals(
        &self,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<SurfaceTessellation, GeometryError> {
        let (mesh, sources, parameters) =
            self.tessellate_impl(samples_per_span, false, false, tolerance, true)?;
        let frames = self
            .faces
            .iter()
            .map(BrepFace::local_parameter_frame)
            .collect::<Result<Vec<_>, _>>()?;
        let mut corners = Vec::with_capacity(mesh.triangles().len());
        for (triangle, source) in mesh.triangles().iter().zip(sources) {
            let frame = &frames[source];
            let face = frame.face.as_ref();
            let mut samples = triangle_corners(
                &face.surface,
                triangle.map(|i| parameters[i as usize]),
                Some(source),
                face.reversed,
            );
            for sample in &mut samples {
                sample.uv_origin = frame.origin;
            }
            corners.push(samples);
        }
        Ok(SurfaceTessellation { mesh, corners })
    }
}
