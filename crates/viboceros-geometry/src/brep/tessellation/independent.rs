//! Per-face grid and trim sampling, followed by shared-boundary auditing.

use super::*;

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
        self.tessellate_impl(samples_per_span, false, false, tolerance)
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
        let samples_per_span = self
            .faces
            .iter()
            .map(|face| {
                face.surface
                    .polygon_mesh_samples_per_span(density, simple_planes, tolerance)
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .max()
            .expect("a validated B-rep has at least one face");
        self.tessellate_impl(samples_per_span, true, jagged_seams, tolerance)
    }

    fn tessellate_impl(
        &self,
        samples_per_span: usize,
        preserve_quads: bool,
        jagged_seams: bool,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        if samples_per_span == 0 {
            return Err(GeometryError::InvalidTessellationResolution);
        }
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        let mut face_sources = Vec::new();
        for (face_index, face) in self.faces.iter().enumerate() {
            let mesh = if face_covers_full_surface_domain(face, tolerance)? {
                let surface_mesh = if preserve_quads {
                    face.surface
                        .tessellate_grid(samples_per_span, true, tolerance)?
                } else {
                    face.surface.tessellate(samples_per_span, tolerance)?
                };
                let mut face_vertices = surface_mesh.vertices().to_vec();
                if !jagged_seams {
                    self.snap_face_boundary_vertices(
                        face,
                        &mut face_vertices,
                        samples_per_span,
                        tolerance,
                    )?;
                }
                TriangleMesh::try_new_faces(
                    face_vertices,
                    surface_mesh.faces().to_vec(),
                    tolerance,
                )?
            } else if let Some(bounds) = rectangular_face_trim_bounds(face, tolerance)? {
                let surface = face
                    .surface
                    .try_trimmed(bounds[0][0]..=bounds[0][1], bounds[1][0]..=bounds[1][1])?;
                if preserve_quads {
                    surface.tessellate_grid(samples_per_span, true, tolerance)?
                } else {
                    surface.tessellate(samples_per_span, tolerance)?
                }
            } else if planar_surface_plane(&face.surface, tolerance)?.is_some() {
                self.tessellate_planar_trimmed_face(
                    face_index,
                    face,
                    samples_per_span,
                    jagged_seams,
                    tolerance,
                )?
            } else {
                self.tessellate_nonplanar_trimmed_face(
                    face_index,
                    face,
                    samples_per_span,
                    jagged_seams,
                    tolerance,
                )?
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
        let mesh = TriangleMesh::try_new_faces(vertices, faces, tolerance)?;
        if !jagged_seams
            && !self.mesh_boundary_conforms(&mesh, &face_sources, samples_per_span, tolerance)?
        {
            return self.tessellate_conforming(samples_per_span, tolerance);
        }
        Ok(mesh)
    }

    fn tessellate_planar_trimmed_face(
        &self,
        face_index: usize,
        face: &BrepFace,
        samples_per_span: usize,
        jagged_seams: bool,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        let mut sampled_loops = face
            .loops
            .iter()
            .map(|face_loop| sample_trim_loop(face_loop, samples_per_span))
            .collect::<Result<Vec<_>, _>>()?;
        let (parameters, boundary_vertex_count, triangles) = if sampled_loops.len() == 1 {
            let mut parameters = sampled_loops.pop().expect("one sampled trim loop exists");
            let boundary_vertex_count = parameters.len();
            let triangles = if let Some(triangles) =
                triangulate_simple_trim_polygon(&mut parameters)?
            {
                triangles
            } else {
                triangulate_trim_region(&parameters, &[boundary_vertex_count])?
                    .ok_or(GeometryError::UnsupportedBrepTrimTessellation { face: face_index })?
            };
            (parameters, boundary_vertex_count, triangles)
        } else {
            let loop_lengths = sampled_loops.iter().map(Vec::len).collect::<Vec<_>>();
            let parameters = sampled_loops.into_iter().flatten().collect::<Vec<_>>();
            let boundary_vertex_count = parameters.len();
            let triangles = triangulate_trim_region(&parameters, &loop_lengths)?
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
        TriangleMesh::try_new(face_vertices, triangles, tolerance)
    }

    fn tessellate_nonplanar_trimmed_face(
        &self,
        face_index: usize,
        face: &BrepFace,
        samples_per_span: usize,
        jagged_seams: bool,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        let sampled_loops = face
            .loops
            .iter()
            .map(|face_loop| sample_trim_loop(face_loop, samples_per_span))
            .collect::<Result<Vec<_>, _>>()?;
        let loop_lengths = sampled_loops.iter().map(Vec::len).collect::<Vec<_>>();
        let mut parameters = sampled_loops.into_iter().flatten().collect::<Vec<_>>();
        let boundary_vertex_count = parameters.len();
        append_trimmed_surface_grid_parameters(
            &mut parameters,
            &loop_lengths,
            &face.surface,
            samples_per_span,
        )?;
        let triangles = triangulate_trim_region(&parameters, &loop_lengths)?
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
        TriangleMesh::try_new(face_vertices, triangles, tolerance)
    }

    fn snap_face_boundary_vertices(
        &self,
        face: &BrepFace,
        vertices: &mut [Point3],
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<(), GeometryError> {
        let candidates = self.face_boundary_snap_points(face, samples_per_span, tolerance)?;
        let span_count_u = face.surface.spans_u().count();
        let span_count_v = face.surface.spans_v().count();
        let side = samples_per_span
            .checked_add(1)
            .ok_or(GeometryError::TooManyMeshVertices)?;
        let vertices_per_patch = side
            .checked_mul(side)
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
                for v_sample in 0..=samples_per_span {
                    for u_sample in 0..=samples_per_span {
                        let vertex = &mut vertices[patch_offset + v_sample * side + u_sample];
                        if v_span == 0 && v_sample == 0 {
                            snap_point_to_candidates(vertex, &candidates[0], tolerance);
                        }
                        if u_span + 1 == span_count_u && u_sample == samples_per_span {
                            snap_point_to_candidates(vertex, &candidates[1], tolerance);
                        }
                        if v_span + 1 == span_count_v && v_sample == samples_per_span {
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
        let spans = edge.curve.spans().collect::<Vec<_>>();
        let domain = edge.curve.domain();
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
                    (edge.curve.evaluate(parameter)?, edge.tolerance)
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
