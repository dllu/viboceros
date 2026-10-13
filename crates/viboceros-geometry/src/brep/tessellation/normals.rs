use super::*;
use crate::{SurfaceTessellation, tessellation::triangle_corners};

impl Brep {
    /// Audited tessellation with per-corner surface normals and native UV/face
    /// provenance. Normals follow each face's orientation, including hard joins.
    /// Regular corners survive even when singular corners have no usable limit.
    /// Trimmed triangles are constrained to possible C0 knot creases. Missing
    /// anchors retry the shared edge table; unsupported anchors return
    /// `UnsupportedBrepNormalCrease` instead of blending across the crease.
    /// The optional mesh can differ from `tessellate`; that API is unchanged.
    pub fn tessellate_with_normals(
        &self,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<SurfaceTessellation, GeometryError> {
        let output = self.tessellate_impl(samples_per_span, false, false, tolerance, true);
        let (mesh, sources, parameters) = match output {
            Err(GeometryError::UnsupportedBrepNormalCrease { .. }) => {
                self.tessellate_conforming_impl(samples_per_span, tolerance, true)?
            }
            output => output?,
        };
        let frames = self
            .faces
            .iter()
            .map(BrepFace::local_parameter_frame)
            .collect::<Result<Vec<_>, _>>()?;
        let creases = frames
            .iter()
            .map(|f| creases::parameters(&f.face.surface))
            .collect::<Vec<_>>();
        let mut corners = Vec::with_capacity(mesh.triangles().len());
        for (triangle, source) in mesh.triangles().iter().zip(sources) {
            creases::validate(
                &parameters,
                std::slice::from_ref(triangle),
                &creases[source],
                source,
            )?;
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
