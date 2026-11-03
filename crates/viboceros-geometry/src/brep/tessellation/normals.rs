use super::*;
use crate::{SurfaceTessellation, tessellation::CornerSampler};

impl Brep {
    pub fn tessellate_with_density(
        &self,
        density: crate::NurbsMeshDensity,
        tolerance: Tolerance,
    ) -> Result<SurfaceTessellation, GeometryError> {
        let grids = self
            .faces
            .iter()
            .map(|face| {
                let frame = face.local_parameter_frame()?;
                let face = frame.face.as_ref();
                let surface = if let Some(bounds) = rectangular_face_trim_bounds(face, tolerance)? {
                    face.surface
                        .try_trimmed(bounds[0][0]..=bounds[0][1], bounds[1][0]..=bounds[1][1])?
                } else {
                    face.surface.clone()
                };
                surface.analysis_mesh_resolution(density)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let sampling = Sampling::per_face(&grids);
        let output = self.tessellate_sampling(sampling, true, false, tolerance, true);
        let output = match output {
            Err(GeometryError::UnsupportedBrepNormalCrease { .. }) => {
                self.tessellate_conforming_sampling(sampling, tolerance, true)?
            }
            output => output?,
        };
        self.tessellation_normals(output)
    }
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
        self.tessellation_normals((mesh, sources, parameters))
    }

    fn tessellation_normals(
        &self,
        (mesh, sources, parameters): (TriangleMesh, Vec<usize>, Vec<Point2>),
    ) -> Result<SurfaceTessellation, GeometryError> {
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
        let sources = mesh.faces().iter().zip(sources).flat_map(|(face, source)| {
            std::iter::repeat_n(source, if face.is_quad() { 2 } else { 1 })
        });
        let mut sampler = CornerSampler::default();
        for (triangle, source) in mesh.triangles().iter().zip(sources) {
            creases::validate(
                &parameters,
                std::slice::from_ref(triangle),
                &creases[source],
                source,
            )?;
            let frame = &frames[source];
            let face = frame.face.as_ref();
            let mut samples = sampler.corners(
                &face.surface,
                *triangle,
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
