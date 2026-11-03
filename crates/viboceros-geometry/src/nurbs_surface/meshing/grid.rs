//! Regular tensor grids and seam stitching, shared by uniform and adaptive meshing.
use super::*;
impl NurbsSurface {
    pub(crate) fn tessellate_grid(
        &self,
        samples_per_span: usize,
        preserve_quads: bool,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        self.tessellate_grid_with_parameters(samples_per_span, preserve_quads, tolerance, false)
            .map(|(mesh, _)| mesh)
    }

    pub(crate) fn tessellate_grid_with_parameters(
        &self,
        samples_per_span: usize,
        preserve_quads: bool,
        tolerance: Tolerance,
        retain_parameters: bool,
    ) -> Result<(TriangleMesh, Vec<crate::Point2>), GeometryError> {
        self.tessellate_grid_with_resolution(
            [samples_per_span; 2],
            preserve_quads,
            tolerance,
            retain_parameters,
        )
    }
    pub(crate) fn tessellate_grid_with_resolution(
        &self,
        resolution: [usize; 2],
        preserve_quads: bool,
        tolerance: Tolerance,
        retain_parameters: bool,
    ) -> Result<(TriangleMesh, Vec<crate::Point2>), GeometryError> {
        let [samples_u, samples_v] = resolution;
        if resolution.contains(&0) {
            return Err(GeometryError::InvalidTessellationResolution);
        }
        let spans_u = self.spans_u().collect::<Vec<_>>();
        let spans_v = self.spans_v().collect::<Vec<_>>();
        let vertices_per_patch = samples_u
            .checked_add(1)
            .and_then(|side| side.checked_mul(samples_v.checked_add(1)?))
            .ok_or(GeometryError::TooManyMeshVertices)?;
        let patch_count = spans_u
            .len()
            .checked_mul(spans_v.len())
            .ok_or(GeometryError::TooManyMeshVertices)?;
        let capacity = vertices_per_patch
            .checked_mul(patch_count)
            .ok_or(GeometryError::TooManyMeshVertices)?;
        if capacity > u32::MAX as usize {
            return Err(GeometryError::TooManyMeshVertices);
        }

        let mut vertices = Vec::with_capacity(capacity);
        let mut parameters = Vec::new();
        if retain_parameters {
            parameters.reserve(capacity);
        }
        let mut faces = Vec::new();
        let domain_u_end = *self.domain_u().end();
        let domain_v_end = *self.domain_v().end();
        let side = samples_u + 1;
        for &(v_start, v_end) in &spans_v {
            for &(u_start, u_end) in &spans_u {
                let offset = u32::try_from(vertices.len())
                    .map_err(|_| GeometryError::TooManyMeshVertices)?;
                for v_sample in 0..=samples_v {
                    let v = span_parameter(v_start, v_end, v_sample, samples_v, domain_v_end);
                    for u_sample in 0..=samples_u {
                        let u = span_parameter(u_start, u_end, u_sample, samples_u, domain_u_end);
                        vertices.push(self.evaluate(u, v)?);
                        if retain_parameters {
                            parameters.push(crate::Point2::try_new(u, v)?);
                        }
                    }
                }
                for row in 0..samples_v {
                    for column in 0..samples_u {
                        let local_lower_left = row
                            .checked_mul(side)
                            .and_then(|index| index.checked_add(column))
                            .and_then(|index| u32::try_from(index).ok())
                            .ok_or(GeometryError::TooManyMeshVertices)?;
                        let lower_left = offset
                            .checked_add(local_lower_left)
                            .ok_or(GeometryError::TooManyMeshVertices)?;
                        let lower_right = lower_left + 1;
                        let row_stride =
                            u32::try_from(side).map_err(|_| GeometryError::TooManyMeshVertices)?;
                        let upper_left = lower_left
                            .checked_add(row_stride)
                            .ok_or(GeometryError::TooManyMeshVertices)?;
                        let upper_right = upper_left + 1;
                        push_tessellation_cell(
                            &vertices,
                            &mut faces,
                            [lower_left, lower_right, upper_right, upper_left],
                            preserve_quads,
                            tolerance,
                        )?;
                    }
                }
            }
        }
        stitch_continuous_patch_boundaries(
            &mut vertices,
            spans_u.len(),
            spans_v.len(),
            resolution,
            tolerance,
        )?;
        stitch_closed_surface_seams(
            &mut vertices,
            spans_u.len(),
            spans_v.len(),
            resolution,
            self.is_closed_u()?,
            self.is_closed_v()?,
            tolerance,
        )?;
        Ok((
            TriangleMesh::try_new_faces(vertices, faces, tolerance)?,
            parameters,
        ))
    }
}
fn span_parameter(
    start: Real,
    end: Real,
    sample: usize,
    sample_count: usize,
    domain_end: Real,
) -> Real {
    let fraction = sample as Real / sample_count as Real;
    let parameter = start.mul_add(1.0 - fraction, end * fraction);
    if sample == sample_count && end < domain_end {
        parameter.next_down().max(start)
    } else {
        parameter
    }
}

fn stitch_continuous_patch_boundaries(
    vertices: &mut [Point3],
    span_count_u: usize,
    span_count_v: usize,
    resolution: [usize; 2],
    tolerance: Tolerance,
) -> Result<(), GeometryError> {
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
    let patch_offset =
        |u_span: usize, v_span: usize| (v_span * span_count_u + u_span) * vertices_per_patch;

    for v_span in 0..span_count_v {
        for u_span in 0..span_count_u.saturating_sub(1) {
            let left = patch_offset(u_span, v_span);
            let right = patch_offset(u_span + 1, v_span);
            for v_sample in 0..=samples_v {
                let left_index = left + v_sample * side + samples_u;
                let right_index = right + v_sample * side;
                snap_first_point_to_second_if_near(vertices, left_index, right_index, tolerance)?;
            }
        }
    }
    for v_span in 0..span_count_v.saturating_sub(1) {
        for u_span in 0..span_count_u {
            let lower = patch_offset(u_span, v_span);
            let upper = patch_offset(u_span, v_span + 1);
            for u_sample in 0..=samples_u {
                let lower_index = lower + samples_v * side + u_sample;
                let upper_index = upper + u_sample;
                snap_first_point_to_second_if_near(vertices, lower_index, upper_index, tolerance)?;
            }
        }
    }
    Ok(())
}

fn stitch_closed_surface_seams(
    vertices: &mut [Point3],
    span_count_u: usize,
    span_count_v: usize,
    resolution: [usize; 2],
    closed_u: bool,
    closed_v: bool,
    tolerance: Tolerance,
) -> Result<(), GeometryError> {
    let [samples_u, samples_v] = resolution;
    let side = samples_u + 1;
    let vertices_per_patch = side * (samples_v + 1);
    let patch_offset =
        |u_span: usize, v_span: usize| (v_span * span_count_u + u_span) * vertices_per_patch;
    if closed_u {
        for v_span in 0..span_count_v {
            let first = patch_offset(0, v_span);
            let last = patch_offset(span_count_u - 1, v_span);
            for v_sample in 0..=samples_v {
                snap_first_point_to_second_if_near(
                    vertices,
                    last + v_sample * side + samples_u,
                    first + v_sample * side,
                    tolerance,
                )?;
            }
        }
    }
    if closed_v {
        for u_span in 0..span_count_u {
            let first = patch_offset(u_span, 0);
            let last = patch_offset(u_span, span_count_v - 1);
            for u_sample in 0..=samples_u {
                snap_first_point_to_second_if_near(
                    vertices,
                    last + samples_v * side + u_sample,
                    first + u_sample,
                    tolerance,
                )?;
            }
        }
    }
    Ok(())
}

fn snap_first_point_to_second_if_near(
    vertices: &mut [Point3],
    first_index: usize,
    second_index: usize,
    tolerance: Tolerance,
) -> Result<(), GeometryError> {
    let first = vertices[first_index];
    let second = vertices[second_index];
    let scale = first
        .to_array()
        .into_iter()
        .chain(second.to_array())
        .map(Real::abs)
        .fold(0.0, Real::max);
    let allowed = tolerance.absolute().max(tolerance.relative() * scale);
    if first.distance_to(second)? <= allowed {
        vertices[first_index] = second;
    }
    Ok(())
}

fn push_tessellation_cell(
    vertices: &[Point3],
    faces: &mut Vec<MeshFace>,
    [lower_left, lower_right, upper_right, upper_left]: [u32; 4],
    preserve_quads: bool,
    tolerance: Tolerance,
) -> Result<(), GeometryError> {
    let first = [lower_left, lower_right, upper_right];
    let second = [lower_left, upper_right, upper_left];
    let first_is_valid = tessellation_triangle_is_nondegenerate(vertices, first, tolerance)?;
    let second_is_valid = tessellation_triangle_is_nondegenerate(vertices, second, tolerance)?;
    if preserve_quads
        && first_is_valid
        && second_is_valid
        && vertices[lower_right as usize] != vertices[upper_left as usize]
    {
        faces.push(MeshFace::Quad([
            lower_left,
            lower_right,
            upper_right,
            upper_left,
        ]));
    } else {
        if first_is_valid {
            faces.push(MeshFace::Triangle(first));
        }
        if second_is_valid {
            faces.push(MeshFace::Triangle(second));
        }
    }
    Ok(())
}
