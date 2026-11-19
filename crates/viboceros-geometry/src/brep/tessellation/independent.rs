//! Per-face grid and trim sampling, followed by shared-boundary auditing.

use super::*;

const DISPLAY_GRID_VERTEX_TARGET: usize = 262_144;
const DENSE_GRID_AXIS_LIMIT: usize = 32;

fn dense_grid_axis_limit(face_count: usize) -> usize {
    // Reserve part of the mesh budget for the actual trim curves. Allow more
    // interior grid detail per face when the body has fewer supports.
    (SPARSE_MESH_VERTEX_LIMIT.div_ceil(face_count) / 2)
        .isqrt()
        .clamp(8, DENSE_GRID_AXIS_LIMIT)
}

fn polygon_face_grid(
    surface: &NurbsSurface,
    density: Real,
    simple_planes: bool,
    tolerance: Tolerance,
) -> Result<[usize; 2], GeometryError> {
    // An untrimmed support can require far more subdivision than the part
    // kept by its loops. Retry that face without degrading adjacent faces.
    for resolution in [density, density * 0.5] {
        match surface.polygon_mesh_counts(resolution, simple_planes, tolerance) {
            // Changing density does not change the size of a Bezier decomposition.
            Err(GeometryError::BezierDecompositionLimit) => break,
            Err(GeometryError::MeshSubdivisionLimit) if resolution > 0. => continue,
            result => return result,
        }
    }
    surface.polygon_mesh_counts(0., simple_planes, tolerance)
}

impl Brep {
    /// Tessellates full rectangular and generally trimmed faces.
    ///
    /// Trim boundaries are sampled per exact p-curve knot span and
    /// constrained-triangulated in parameter space while preserving every
    /// outer and inner boundary sample for watertight stitching. Nonplanar
    /// faces also receive the underlying surface's knot-span grid samples so
    /// their interior approximation tracks the requested density.
    /// If independent grids fail the naked-boundary audit, a shared-edge
    /// constrained triangulation rebuilds the face boundaries.
    pub fn tessellate(
        &self,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        self.tessellate_impl(samples_per_span, false, false, tolerance, false)
            .map(|(mesh, _, _)| mesh)
    }

    /// Creates a display approximation without requiring a watertight mesh.
    ///
    /// Prefer the audited, conforming tessellation. If shared-edge stitching
    /// cannot be certified, sample the trimmed faces independently instead.
    /// Trims and face winding are retained, but seams may have small cracks.
    /// Use `tessellate` or `polygon_mesh` for exports and geometric operations.
    pub fn display_mesh(
        &self,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        self.display_mesh_with_face_sources(samples_per_span, tolerance)
            .map(|(mesh, _)| mesh)
    }

    /// Display mesh and its source B-rep face for every stored mesh face.
    /// The mapping survives the conforming seam fallback used by `display_mesh`.
    /// Dense trimmed faces retry at lower sampling resolutions when a mesh
    /// vertex budget would otherwise prevent display.
    pub fn display_mesh_with_face_sources(
        &self,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<(TriangleMesh, Vec<usize>), GeometryError> {
        self.display_tessellation(samples_per_span, tolerance, false)
            .map(|(mesh, sources, _)| (mesh, sources))
    }

    pub(super) fn display_tessellation(
        &self,
        samples_per_span: usize,
        tolerance: Tolerance,
        retain_parameters: bool,
    ) -> Result<(TriangleMesh, Vec<usize>, Vec<Point2>), GeometryError> {
        let mut resolution = samples_per_span;
        let spans = self.faces.iter().fold(0usize, |total, face| {
            total.saturating_add(
                face.surface
                    .spans_u()
                    .count()
                    .saturating_mul(face.surface.spans_v().count()),
            )
        });
        if spans.saturating_mul(4) > DISPLAY_GRID_VERTEX_TARGET && !self.has_positional_break() {
            let limit = dense_grid_axis_limit(self.faces.len());
            let grids = self
                .faces
                .iter()
                .map(|face| {
                    // Trim loops with many authored spans already have boundary
                    // detail. Retain the requested resolution on simpler faces,
                    // where a long circular span can otherwise become one chord.
                    let spans = face
                        .loops
                        .iter()
                        .flat_map(|boundary| &boundary.trims)
                        .map(|trim| trim.curve.spans().count())
                        .sum::<usize>();
                    let samples = resolution
                        .div_ceil(spans.div_ceil(24).max(1))
                        .max(resolution.min(2));
                    [samples; 2]
                })
                .collect::<Vec<_>>();
            return self
                .tessellate_sampling(
                    Sampling::per_face(&grids).with_axis_limit(limit),
                    false,
                    true,
                    tolerance,
                    retain_parameters,
                    true,
                )
                .or_else(|error| match error {
                    GeometryError::TooManyMeshVertices
                    | GeometryError::MeshSubdivisionLimit
                    | GeometryError::UnsupportedBrepTrimTessellation { .. }
                        if resolution > 2 =>
                    {
                        self.tessellate_sampling(
                            Sampling::uniform(2).with_axis_limit(limit),
                            false,
                            true,
                            tolerance,
                            retain_parameters,
                            true,
                        )
                    }
                    _ => Err(error),
                });
        }
        while resolution > 1
            && spans.saturating_mul(resolution.saturating_add(1).saturating_pow(2))
                > DISPLAY_GRID_VERTEX_TARGET
        {
            resolution /= 2;
        }
        loop {
            match self.display_mesh_at_resolution(resolution, tolerance, retain_parameters) {
                Err(GeometryError::TooManyMeshVertices | GeometryError::MeshSubdivisionLimit)
                    if resolution > 1 =>
                {
                    resolution /= 2
                }
                result => return result,
            }
        }
    }

    fn display_mesh_at_resolution(
        &self,
        samples_per_span: usize,
        tolerance: Tolerance,
        retain_parameters: bool,
    ) -> Result<(TriangleMesh, Vec<usize>, Vec<Point2>), GeometryError> {
        self.tessellate_impl(samples_per_span, false, false, tolerance, retain_parameters)
            .or_else(|error| {
                // Independent UV triangulation does not constrain internal
                // positional breaks. Never bridge one to obtain a display mesh.
                if self.has_positional_break() {
                    return Err(error);
                }
                self.tessellate_sampling(
                    Sampling::uniform(samples_per_span),
                    false,
                    true,
                    tolerance,
                    retain_parameters,
                    true,
                )
            })
    }

    fn has_positional_break(&self) -> bool {
        self.faces.iter().any(|face| {
            let surface = &face.surface;
            [
                (surface.knots_u(), surface.degree_u(), surface.domain_u()),
                (surface.knots_v(), surface.degree_v(), surface.domain_v()),
            ]
            .into_iter()
            .any(|(knots, degree, domain)| {
                knots
                    .chunk_by(|a, b| a == b)
                    .any(|g| g.len() > degree && g[0] > *domain.start() && g[0] < *domain.end())
            })
        })
    }

    /// Creates one editable triangle/quad mesh for this B-rep.
    ///
    /// Full rectangular surface cells remain quadrilaterals and trimmed
    /// regions remain constrained triangles. With smooth seams, boundary
    /// samples are snapped to shared exact edges. Naked mesh sides must belong
    /// to true naked boundaries on their source B-rep faces; closed solids must
    /// remain watertight. A conforming triangle fallback replaces quads when
    /// independent grids cannot be stitched. Jagged seams disable both snapping
    /// and shared-boundary auditing and permit naked edges between faces.
    pub fn polygon_mesh(
        &self,
        density: Real,
        simple_planes: bool,
        jagged_seams: bool,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        let grids = self
            .faces
            .iter()
            .map(|face| {
                let frame = face.sampling_parameter_frame()?;
                polygon_face_grid(&frame.face.surface, density, simple_planes, tolerance)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let sampling = Sampling::per_face(&grids);
        let sparse_sampling = || {
            sampling
                .with_axis_limit(dense_grid_axis_limit(self.faces.len()))
                .with_vertex_limit(SPARSE_POLYGON_VERTEX_LIMIT)
        };
        if jagged_seams {
            return self
                .tessellate_sampling(sampling, true, true, tolerance, false, false)
                .or_else(|error| {
                    if !self.has_positional_break()
                        && matches!(
                            error,
                            GeometryError::MeshSubdivisionLimit
                                | GeometryError::TooManyMeshVertices
                        )
                    {
                        self.tessellate_sampling(
                            sparse_sampling(),
                            true,
                            true,
                            tolerance,
                            false,
                            false,
                        )
                    } else {
                        Err(error)
                    }
                })
                .map(|(mesh, _, _)| mesh);
        }
        let samples_per_span = grids
            .iter()
            .flatten()
            .copied()
            .max()
            .expect("a validated B-rep has at least one face");
        // A B-rep currently needs one common resolution for seam stitching.
        // Check its aggregate rectangular grid before any face allocates it;
        // an anisotropic surface's maximum count can be much larger in both axes.
        let spans = self.faces.iter().try_fold(0usize, |sum, face| {
            let count = face
                .surface
                .spans_u()
                .count()
                .checked_mul(face.surface.spans_v().count())?;
            sum.checked_add(count)
        });
        let vertices = samples_per_span
            .checked_add(1)
            .and_then(|side| side.checked_mul(side))
            .and_then(|n| n.checked_mul(spans?));
        if vertices.is_none_or(|n| n > 1_048_576) {
            if !self.has_positional_break() {
                return self
                    .tessellate_conforming_sampling(sparse_sampling(), tolerance, false)
                    .map(|(mesh, _, _)| mesh);
            }
            return Err(GeometryError::MeshSubdivisionLimit);
        }
        self.tessellate_impl(samples_per_span, true, jagged_seams, tolerance, false)
            .map(|(mesh, _, _)| mesh)
    }

    pub(super) fn tessellate_impl(
        &self,
        samples_per_span: usize,
        preserve_quads: bool,
        jagged_seams: bool,
        tolerance: Tolerance,
        retain_parameters: bool,
    ) -> Result<(TriangleMesh, Vec<usize>, Vec<Point2>), GeometryError> {
        self.tessellate_sampling(
            Sampling::uniform(samples_per_span),
            preserve_quads,
            jagged_seams,
            tolerance,
            retain_parameters,
            false,
        )
    }

    pub(super) fn tessellate_sampling(
        &self,
        sampling: Sampling<'_>,
        preserve_quads: bool,
        jagged_seams: bool,
        tolerance: Tolerance,
        retain_parameters: bool,
        skip_empty_faces: bool,
    ) -> Result<(TriangleMesh, Vec<usize>, Vec<Point2>), GeometryError> {
        sampling.validate(&self.faces)?;
        if sampling.edge == 0 {
            return Err(GeometryError::InvalidTessellationResolution);
        }
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        let mut face_sources = Vec::new();
        let mut vertex_parameters = Vec::new();
        let constrain_creases = retain_parameters && !skip_empty_faces;
        for (face_index, face) in self.faces.iter().enumerate() {
            let frame = face.sampling_parameter_frame()?;
            let face = frame.face.as_ref();
            let resolution = sampling.grid(face_index);
            let samples_per_span = sampling.edge_resolution(face_index, jagged_seams);
            let sparse = sampling.uses_sparse_grid(face_index, &face.surface);
            let tessellate_face = |samples_per_span| {
                if !sparse && face_covers_full_surface_domain(face, tolerance)? {
                    let (surface_mesh, parameters) = face.surface.tessellate_grid_with_resolution(
                        resolution,
                        preserve_quads,
                        tolerance,
                        retain_parameters,
                    )?;
                    let mut face_vertices = surface_mesh.vertices().to_vec();
                    if !jagged_seams {
                        self.snap_face_boundary_vertices(
                            face,
                            &mut face_vertices,
                            resolution,
                            samples_per_span,
                            tolerance,
                        )?;
                    }
                    Ok((
                        TriangleMesh::try_new_faces(
                            face_vertices,
                            surface_mesh.faces().to_vec(),
                            Tolerance::MESH_VALIDATION,
                        )?,
                        parameters,
                    ))
                } else if !sparse
                    && let Some(bounds) = rectangular_face_trim_bounds(face, tolerance)?
                {
                    let surface = face
                        .surface
                        .try_trimmed(bounds[0][0]..=bounds[0][1], bounds[1][0]..=bounds[1][1])?;
                    surface.tessellate_grid_with_resolution(
                        resolution,
                        preserve_quads,
                        tolerance,
                        retain_parameters,
                    )
                } else if planar_surface_plane(&face.surface, tolerance)?.is_some() {
                    self.tessellate_planar_trimmed_face(
                        face_index,
                        face,
                        samples_per_span,
                        jagged_seams,
                        tolerance,
                        constrain_creases,
                    )
                } else {
                    self.tessellate_nonplanar_trimmed_face(
                        face_index,
                        face,
                        sampling.with_trim_resolution(samples_per_span),
                        jagged_seams,
                        tolerance,
                        constrain_creases,
                    )
                }
            };
            let mut piece = tessellate_face(samples_per_span);
            // A coarse chord may cross another real trim even though the
            // underlying curves are disjoint. Retry only the affected face
            // when its mesh is allowed to keep independent boundary samples.
            if jagged_seams {
                let mut refined = samples_per_span;
                while refined < 8
                    && matches!(
                        piece,
                        Err(GeometryError::UnsupportedBrepTrimTessellation { .. })
                    )
                {
                    refined = (2 * refined).min(8);
                    piece = tessellate_face(refined);
                }
            }
            let (mesh, parameters) = match piece {
                // A collapsed face can retain wire geometry. Its neighbors
                // still contribute to the display mesh.
                Err(GeometryError::EmptyMesh) if skip_empty_faces => continue,
                result => result?,
            };
            let offset =
                u32::try_from(vertices.len()).map_err(|_| GeometryError::TooManyMeshVertices)?;
            let combined_vertex_count = vertices
                .len()
                .checked_add(mesh.vertices().len())
                .ok_or(GeometryError::TooManyMeshVertices)?;
            if combined_vertex_count > u32::MAX as usize {
                return Err(GeometryError::TooManyMeshVertices);
            }
            if combined_vertex_count > sampling.vertex_limit() {
                return Err(GeometryError::MeshSubdivisionLimit);
            }
            if retain_parameters {
                vertex_parameters.extend(parameters);
            }
            vertices.extend(mesh.vertices());
            for source_face in mesh.faces() {
                let mut mapped = source_face.remapped(|vertex| {
                    vertex
                        .checked_add(offset)
                        .expect("the combined mesh vertex range was checked")
                });
                if face.reversed {
                    mapped = mapped.reversed();
                }
                faces.push(mapped);
                face_sources.push(face_index);
            }
        }
        let mesh = TriangleMesh::try_new_faces(vertices, faces, Tolerance::MESH_VALIDATION)?;
        if !jagged_seams
            && !self.mesh_boundary_conforms(&mesh, &face_sources, sampling.edge, tolerance)?
        {
            return self.tessellate_conforming_sampling(sampling, tolerance, retain_parameters);
        }
        Ok((mesh, face_sources, vertex_parameters))
    }

    fn tessellate_planar_trimmed_face(
        &self,
        face_index: usize,
        face: &BrepFace,
        samples_per_span: usize,
        jagged_seams: bool,
        tolerance: Tolerance,
        retain_parameters: bool,
    ) -> Result<(TriangleMesh, Vec<Point2>), GeometryError> {
        let mut sampled_loops = face
            .loops
            .iter()
            .map(|face_loop| {
                trim_chart_samples(face, face_loop, samples_per_span, jagged_seams, tolerance)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let (parameters, boundary_vertex_count, triangles) = if sampled_loops.len() == 1 {
            let mut parameters = sampled_loops.pop().expect("one sampled trim loop exists");
            let boundary_vertex_count = parameters.len();
            let has_creases = retain_parameters
                && creases::parameters(&face.surface)
                    .iter()
                    .any(|p| !p.is_empty());
            let triangles = if has_creases {
                creases::triangulate(
                    &mut parameters,
                    &[boundary_vertex_count],
                    &face.surface,
                    face_index,
                )?
                .ok_or(GeometryError::UnsupportedBrepTrimTessellation { face: face_index })?
            } else if let Some(triangles) = triangulate_simple_trim_polygon(&mut parameters)? {
                triangles
            } else {
                trim_region::triangulate(&mut parameters, &[boundary_vertex_count], &[])?
                    .ok_or(GeometryError::UnsupportedBrepTrimTessellation { face: face_index })?
            };
            (parameters, boundary_vertex_count, triangles)
        } else {
            let loop_lengths = sampled_loops.iter().map(Vec::len).collect::<Vec<_>>();
            let mut parameters = sampled_loops.into_iter().flatten().collect::<Vec<_>>();
            let boundary_vertex_count = parameters.len();
            let triangles = if retain_parameters {
                creases::triangulate(&mut parameters, &loop_lengths, &face.surface, face_index)?
            } else {
                trim_region::triangulate(&mut parameters, &loop_lengths, &[])?
            }
            .ok_or(GeometryError::UnsupportedBrepTrimTessellation { face: face_index })?;
            (parameters, boundary_vertex_count, triangles)
        };
        let mut face_vertices = parameters
            .iter()
            .map(|parameter| face.surface.evaluate(parameter.x(), parameter.y()))
            .collect::<Result<Vec<_>, _>>()?;
        if !jagged_seams {
            let candidates = self.trim_boundary_snap_points(face, samples_per_span, tolerance)?;
            snap_points_to_candidates(
                &mut face_vertices[..boundary_vertex_count],
                candidates,
                tolerance,
            );
        }
        Ok((trimmed_face_mesh(face_vertices, triangles)?, parameters))
    }

    fn tessellate_nonplanar_trimmed_face(
        &self,
        face_index: usize,
        face: &BrepFace,
        sampling: Sampling<'_>,
        jagged_seams: bool,
        tolerance: Tolerance,
        retain_parameters: bool,
    ) -> Result<(TriangleMesh, Vec<Point2>), GeometryError> {
        let samples_per_span = sampling.edge_resolution(face_index, jagged_seams);
        let sampled_loops = face
            .loops
            .iter()
            .map(|face_loop| {
                trim_chart_samples(face, face_loop, samples_per_span, jagged_seams, tolerance)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let loop_lengths = sampled_loops.iter().map(Vec::len).collect::<Vec<_>>();
        let mut parameters = sampled_loops.into_iter().flatten().collect::<Vec<_>>();
        let boundary_vertex_count = parameters.len();
        append_trimmed_surface_grid_parameters(
            &mut parameters,
            &loop_lengths,
            &face.surface,
            sampling.grid(face_index),
            sampling.axis_limit,
        )?;
        let triangles = if retain_parameters {
            creases::triangulate(&mut parameters, &loop_lengths, &face.surface, face_index)?
        } else {
            trim_region::triangulate(&mut parameters, &loop_lengths, &[])?
        }
        .ok_or(GeometryError::UnsupportedBrepTrimTessellation { face: face_index })?;
        let mut face_vertices = parameters
            .iter()
            .map(|parameter| face.surface.evaluate(parameter.x(), parameter.y()))
            .collect::<Result<Vec<_>, _>>()?;
        if !jagged_seams {
            let candidates = self.trim_boundary_snap_points(face, samples_per_span, tolerance)?;
            snap_points_to_candidates(
                &mut face_vertices[..boundary_vertex_count],
                candidates,
                tolerance,
            );
        }
        Ok((trimmed_face_mesh(face_vertices, triangles)?, parameters))
    }

    fn snap_face_boundary_vertices(
        &self,
        face: &BrepFace,
        vertices: &mut [Point3],
        resolution: [usize; 2],
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<(), GeometryError> {
        let candidates = self.face_boundary_snap_points(face, samples_per_span, tolerance)?;
        let span_count_u = face.surface.spans_u().count();
        let span_count_v = face.surface.spans_v().count();
        let [samples_u, samples_v] = resolution;
        let side = samples_u
            .checked_add(1)
            .ok_or(GeometryError::TooManyMeshVertices)?;
        let vertices_per_patch = side
            .checked_mul(
                samples_v
                    .checked_add(1)
                    .ok_or(GeometryError::TooManyMeshVertices)?,
            )
            .ok_or(GeometryError::TooManyMeshVertices)?;
        let expected_vertex_count = span_count_u
            .checked_mul(span_count_v)
            .and_then(|count| count.checked_mul(vertices_per_patch))
            .ok_or(GeometryError::TooManyMeshVertices)?;
        if vertices.len() != expected_vertex_count {
            return invalid("surface tessellation layout changed while stitching B-rep edges");
        }

        for v_span in 0..span_count_v {
            for u_span in 0..span_count_u {
                let patch = v_span * span_count_u + u_span;
                let patch_offset = patch * vertices_per_patch;
                for v_sample in 0..=samples_v {
                    for u_sample in 0..=samples_u {
                        let vertex = &mut vertices[patch_offset + v_sample * side + u_sample];
                        if v_span == 0 && v_sample == 0 {
                            snap_point_to_candidates(vertex, &candidates[0], tolerance);
                        }
                        if u_span + 1 == span_count_u && u_sample == samples_u {
                            snap_point_to_candidates(vertex, &candidates[1], tolerance);
                        }
                        if v_span + 1 == span_count_v && v_sample == samples_v {
                            snap_point_to_candidates(vertex, &candidates[2], tolerance);
                        }
                        if u_span == 0 && u_sample == 0 {
                            snap_point_to_candidates(vertex, &candidates[3], tolerance);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn face_boundary_snap_points(
        &self,
        face: &BrepFace,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<[Vec<BoundarySnapPoint>; 4], GeometryError> {
        let mut candidates: [Vec<BoundarySnapPoint>; 4] = std::array::from_fn(|_| Vec::new());
        for trim in &face.loops[0].trims {
            let side = match trim.iso {
                SurfaceIso::South => 0,
                SurfaceIso::East => 1,
                SurfaceIso::North => 2,
                SurfaceIso::West => 3,
                SurfaceIso::NotIso
                | SurfaceIso::InteriorUConstant
                | SurfaceIso::InteriorVConstant => {
                    return invalid("a full-domain B-rep face has a non-boundary side");
                }
            };
            candidates[side].extend(self.trim_snap_points(trim, samples_per_span, tolerance)?);
        }
        Ok(candidates)
    }

    fn trim_boundary_snap_points(
        &self,
        face: &BrepFace,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<Vec<BoundarySnapPoint>, GeometryError> {
        let mut candidates = Vec::new();
        for trim in face.loops.iter().flat_map(|face_loop| &face_loop.trims) {
            candidates.extend(self.trim_snap_points(trim, samples_per_span, tolerance)?);
        }
        Ok(candidates)
    }

    pub(super) fn trim_snap_points(
        &self,
        trim: &BrepTrim,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<Vec<BoundarySnapPoint>, GeometryError> {
        let Some(edge_index) = trim.edge else {
            let vertex = self.vertices[trim.vertices[0]];
            return Ok(vec![BoundarySnapPoint {
                point: vertex.point,
                tolerance: tolerance.absolute().max(vertex.tolerance),
            }]);
        };
        let edge = &self.edges[edge_index];
        // Model-space samples do not need the native parameter origin. A
        // microscopic span near 0.5 otherwise rounds its interior stations to
        // a different grid than the incident face's losslessly local UV frame.
        let frame = edge.curve.sampling_parameter_frame()?;
        let curve = frame.curve.as_ref();
        let spans = curve.spans().collect::<Vec<_>>();
        let domain = curve.domain();
        let last_span = spans
            .len()
            .checked_sub(1)
            .ok_or(GeometryError::InvalidBrepTopology {
                context: "a B-rep edge curve has no nonempty span",
            })?;
        let capacity = spans
            .len()
            .checked_mul(
                samples_per_span
                    .checked_add(1)
                    .ok_or(GeometryError::TooManyMeshVertices)?,
            )
            .ok_or(GeometryError::TooManyMeshVertices)?;
        if capacity > u32::MAX as usize {
            return Err(GeometryError::TooManyMeshVertices);
        }
        let mut candidates = Vec::with_capacity(capacity);
        for (span_index, (start, end)) in spans.into_iter().enumerate() {
            for sample in 0..=samples_per_span {
                let is_start = span_index == 0 && sample == 0;
                let is_end = span_index == last_span && sample == samples_per_span;
                let (point, component_tolerance) = if is_start {
                    let vertex = self.vertices[edge.vertices[0]];
                    (vertex.point, edge.tolerance.max(vertex.tolerance))
                } else if is_end {
                    let vertex = self.vertices[edge.vertices[1]];
                    (vertex.point, edge.tolerance.max(vertex.tolerance))
                } else {
                    let parameter =
                        brep_span_parameter(start, end, sample, samples_per_span, *domain.end());
                    (curve.evaluate(parameter)?, edge.tolerance)
                };
                candidates.push(BoundarySnapPoint {
                    point,
                    tolerance: tolerance.absolute().max(component_tolerance),
                });
            }
        }
        Ok(candidates)
    }
}

// A trim may meet a singular pole or snap several UV samples to one model
// point. As with surface-grid meshing, omit collapsed facets; the strict
// caller still audits the resulting boundary and rebuilds it if necessary.
// Validate retained facets numerically, independently of the modeling angle.
fn trimmed_face_mesh(
    vertices: Vec<Point3>,
    triangles: Vec<[u32; 3]>,
) -> Result<TriangleMesh, GeometryError> {
    let mut nondegenerate = Vec::with_capacity(triangles.len());
    for triangle in triangles {
        if crate::nurbs_surface::tessellation_triangle_is_nondegenerate(
            &vertices,
            triangle,
            Tolerance::MESH_VALIDATION,
        )? {
            nondegenerate.push(triangle);
        }
    }
    TriangleMesh::try_new(vertices, nondegenerate, Tolerance::MESH_VALIDATION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trimmed_mesh_omits_collapsed_poles_and_keeps_narrow_nonzero_facets() {
        let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let vertices = vec![p(0., 0.), p(1., 0.), p(0., 0.), p(1., 1e-12), p(0., 1.)];
        let mesh = trimmed_face_mesh(vertices, vec![[0, 1, 2], [0, 1, 3], [0, 3, 4]]).unwrap();
        assert_eq!(mesh.triangles(), &[[0, 1, 3], [0, 3, 4]]);
        assert!(mesh.topology().is_oriented());
        assert!((mesh.area().unwrap() - (0.5 + 0.5e-12)).abs() < 1e-15);
    }
}
