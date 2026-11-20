use super::*;
use crate::{MeshFace, SurfaceTessellation, TessellationCorner, tessellation::CornerSampler};

/// A shaded representation with source-face indices and oriented normals.
pub struct BrepDisplayMesh {
    pub mesh: TriangleMesh,
    pub face_sources: Vec<usize>,
    pub corner_normals: Vec<[Option<UnitVector3>; 3]>,
}

impl Brep {
    /// Sample the source normals at the display mesh's own native UV stations.
    /// Dense B-reps share edge samples when the result passes the
    /// mesh-boundary audit.
    /// Corners without a usable limit, and triangles crossing internal creases
    /// in a sparse display mesh, return no normal for those corners.
    pub fn display_mesh_with_normals(
        &self,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<BrepDisplayMesh, GeometryError> {
        let (mesh, face_sources, parameters) =
            self.display_tessellation(samples_per_span, tolerance, true)?;
        let corner_normals = self.sample_tessellation_corners(
            &mesh,
            &face_sources,
            &parameters,
            false,
            |corners| corners.map_or([None; 3], |corners| corners.map(|corner| corner.normal)),
        )?;
        Ok(BrepDisplayMesh {
            mesh,
            face_sources,
            corner_normals,
        })
    }

    pub fn tessellate_with_density(
        &self,
        density: crate::NurbsMeshDensity,
        tolerance: Tolerance,
    ) -> Result<SurfaceTessellation, GeometryError> {
        self.tessellate_with_analysis_settings(
            crate::AnalysisMeshSettings {
                nurbs_density: density,
                ..Default::default()
            },
            tolerance,
        )
    }
    pub fn tessellate_with_analysis_settings(
        &self,
        settings: crate::AnalysisMeshSettings,
        tolerance: Tolerance,
    ) -> Result<SurfaceTessellation, GeometryError> {
        let grids = self
            .faces
            .iter()
            .map(|face| {
                let frame = face.sampling_parameter_frame()?;
                let face = frame.face.as_ref();
                let surface = if let Some(bounds) = rectangular_face_trim_bounds(face, tolerance)? {
                    face.surface
                        .try_trimmed(bounds[0][0]..=bounds[0][1], bounds[1][0]..=bounds[1][1])?
                } else {
                    face.surface.clone()
                };
                surface.analysis_mesh_resolution(
                    settings.nurbs_density,
                    settings.simple_planes,
                    tolerance,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let sampling = Sampling::per_face(&grids);
        let output = self.tessellate_sampling(sampling, true, false, tolerance, true, false);
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
        let corners =
            self.sample_tessellation_corners(&mesh, &sources, &parameters, true, |corners| {
                corners.expect("crease-aligned tessellation returns its corners")
            })?;
        Ok(SurfaceTessellation { mesh, corners })
    }

    fn sample_tessellation_corners<T: Send>(
        &self,
        mesh: &TriangleMesh,
        sources: &[usize],
        parameters: &[Point2],
        require_crease_alignment: bool,
        convert: impl Fn(Option<[TessellationCorner; 3]>) -> T + Sync,
    ) -> Result<Vec<T>, GeometryError> {
        let frames = self
            .faces
            .iter()
            .map(BrepFace::sampling_parameter_frame)
            .collect::<Result<Vec<_>, _>>()?;
        let creases = frames
            .iter()
            .map(|f| creases::parameters(&f.face.surface))
            .collect::<Vec<_>>();
        let sample = |triangles: &[[u32; 3]], faces: &[MeshFace], sources: &[usize]| {
            let sources = faces.iter().zip(sources).flat_map(|(face, &source)| {
                std::iter::repeat_n(source, if face.is_quad() { 2 } else { 1 })
            });
            let mut sampler = CornerSampler::default();
            let mut previous = None;
            let mut corners = Vec::with_capacity(triangles.len());
            for (triangle, source) in triangles.iter().zip(sources) {
                if previous != Some(source) {
                    sampler = CornerSampler::default();
                    previous = Some(source);
                }
                if let Err(error) = creases::validate(
                    parameters,
                    std::slice::from_ref(triangle),
                    &creases[source],
                    source,
                ) {
                    if require_crease_alignment {
                        return Err(error);
                    }
                    corners.push(convert(None));
                    continue;
                }
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
                corners.push(convert(Some(samples)));
            }
            Ok(corners)
        };
        let workers = std::thread::available_parallelism()
            .map_or(1, usize::from)
            .min(8);
        if require_crease_alignment || workers == 1 || mesh.faces().len() < 80_000 {
            return sample(mesh.triangles(), mesh.faces(), sources);
        }
        // The mesh keeps triangles grouped by source face. Work in contiguous
        // ranges and collect them in mesh order to retain corner provenance.
        let chunk = mesh.faces().len().div_ceil(workers);
        std::thread::scope(|scope| {
            let mut triangles = mesh.triangles();
            let mut tasks = Vec::new();
            for (faces, sources) in mesh.faces().chunks(chunk).zip(sources.chunks(chunk)) {
                let count = faces.len() + faces.iter().filter(|face| face.is_quad()).count();
                let (indices, remaining) = triangles.split_at(count);
                triangles = remaining;
                let sample = &sample;
                tasks.push(scope.spawn(move || sample(indices, faces, sources)));
            }
            let mut corners = Vec::with_capacity(mesh.triangles().len());
            for task in tasks {
                corners.extend(task.join().expect("corner sampler panicked")?);
            }
            Ok(corners)
        })
    }
}
