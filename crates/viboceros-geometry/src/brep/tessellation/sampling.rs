use super::*;

#[derive(Clone, Copy)]
pub(super) struct Sampling<'a> {
    pub edge: usize,
    grids: Option<&'a [[usize; 2]]>,
    pub axis_limit: Option<usize>,
    trim_resolution: Option<usize>,
}
impl<'a> Sampling<'a> {
    pub fn uniform(samples: usize) -> Self {
        Self {
            edge: samples,
            grids: None,
            axis_limit: None,
            trim_resolution: None,
        }
    }
    pub fn per_face(grids: &'a [[usize; 2]]) -> Self {
        Self {
            edge: grids.iter().flatten().copied().max().unwrap_or(1),
            grids: Some(grids),
            axis_limit: None,
            trim_resolution: None,
        }
    }
    pub fn with_axis_limit(mut self, limit: usize) -> Self {
        self.axis_limit = Some(limit);
        self
    }
    pub fn grid(self, face: usize) -> [usize; 2] {
        self.grids.map_or([self.edge; 2], |g| g[face])
    }
    pub fn with_trim_resolution(mut self, resolution: usize) -> Self {
        self.trim_resolution = Some(resolution);
        self
    }
    pub fn edge_resolution(self, face: usize, jagged_seams: bool) -> usize {
        if jagged_seams {
            self.trim_resolution.unwrap_or_else(|| {
                let [u, v] = self.grid(face);
                u.max(v)
            })
        } else {
            self.edge
        }
    }
    pub fn vertex_limit(self) -> usize {
        if self.grids.is_some() || self.axis_limit.is_some() {
            1_048_576
        } else {
            u32::MAX as usize
        }
    }
    pub fn uses_sparse_grid(self, face: usize, surface: &NurbsSurface) -> bool {
        self.axis_limit.is_some_and(|limit| {
            let [u, v] = self.grid(face);
            surface.spans_u().count().saturating_mul(u) >= limit
                || surface.spans_v().count().saturating_mul(v) >= limit
        })
    }
    pub fn validate(self, faces: &[BrepFace]) -> Result<(), GeometryError> {
        if self.edge == 0
            || self
                .grids
                .is_some_and(|g| g.len() != faces.len() || g.iter().any(|r| r.contains(&0)))
        {
            return Err(GeometryError::InvalidTessellationResolution);
        }
        if self.grids.is_none() && self.axis_limit.is_none() {
            return Ok(());
        }
        let mut total = 0usize;
        for (index, face) in faces.iter().enumerate() {
            let [u, v] = self.grid(index);
            let surface = &face.surface;
            let [spans_u, spans_v] = [surface.spans_u().count(), surface.spans_v().count()];
            let count = if let Some(limit) = self
                .axis_limit
                .filter(|_| self.uses_sparse_grid(index, surface))
            {
                let axis = |samples: usize, spans: usize| {
                    samples.saturating_mul(spans).saturating_add(1).min(limit)
                };
                axis(u, spans_u).checked_mul(axis(v, spans_v))
            } else {
                u.checked_add(1)
                    .and_then(|u| v.checked_add(1).and_then(|v| u.checked_mul(v)))
                    .and_then(|n| n.checked_mul(spans_u))
                    .and_then(|n| n.checked_mul(spans_v))
            };
            total = total
                .checked_add(count.ok_or(GeometryError::MeshSubdivisionLimit)?)
                .ok_or(GeometryError::MeshSubdivisionLimit)?;
            if total > 1_048_576 {
                return Err(GeometryError::MeshSubdivisionLimit);
            }
        }
        Ok(())
    }
}

pub(super) fn trim_chart_samples(
    face: &BrepFace,
    boundary: &BrepLoop,
    resolution: usize,
    jagged_seams: bool,
    tolerance: Tolerance,
) -> Result<Vec<Point2>, GeometryError> {
    let mut samples = sample_trim_loop(boundary, resolution)?;
    if !jagged_seams {
        return Ok(samples);
    }
    // UV endpoint roundoff can leave two nearly identical polygon vertices.
    // Keep conforming seam samples intact. On independent faces, eliminate
    // only neighboring duplicates that also coincide in the sampled image.
    let surface = &face.surface;
    let close_uv = [surface.domain_u(), surface.domain_v()]
        .map(|d| (d.end() - d.start()) * (8. * Real::EPSILON));
    let coincident = |a: Point2, b: Point2| -> Result<bool, GeometryError> {
        if (a.x() - b.x()).abs() > close_uv[0] || (a.y() - b.y()).abs() > close_uv[1] {
            return Ok(false);
        }
        let first = surface.evaluate(a.x(), a.y())?;
        let second = surface.evaluate(b.x(), b.y())?;
        Ok(first.distance_to(second)? <= tolerance.absolute() * 1e-6)
    };
    let mut retained = 0;
    for i in 0..samples.len() {
        if retained == 0 || !coincident(samples[retained - 1], samples[i])? {
            samples[retained] = samples[i];
            retained += 1;
        }
    }
    if retained > 1 && coincident(samples[retained - 1], samples[0])? {
        retained -= 1;
    }
    samples.truncate(retained);
    Ok(samples)
}

pub(super) fn append_trimmed_surface_grid_parameters(
    parameters: &mut Vec<Point2>,
    loop_lengths: &[usize],
    surface: &NurbsSurface,
    resolution: [usize; 2],
    axis_limit: Option<usize>,
) -> Result<(), GeometryError> {
    let boundary_vertex_count = loop_lengths
        .iter()
        .try_fold(0_usize, |total, length| total.checked_add(*length))
        .ok_or(GeometryError::TooManyMeshVertices)?;
    if boundary_vertex_count != parameters.len() || loop_lengths.is_empty() {
        return invalid("trimmed surface grid requires complete sampled boundary loops");
    }
    if boundary_vertex_count > MAX_CONSTRAINED_TRIM_VERTICES {
        return Err(GeometryError::TooManyMeshVertices);
    }
    let Some(normalization) = TrimParameterNormalization::try_from_points(parameters)? else {
        return Ok(());
    };
    let normalized_boundary = parameters
        .iter()
        .map(|parameter| normalization.normalize(*parameter))
        .collect::<Result<Vec<_>, _>>()?;
    let mut loop_ranges = Vec::with_capacity(loop_lengths.len());
    let mut loop_start = 0;
    for length in loop_lengths {
        let loop_end = loop_start + *length;
        loop_ranges.push(loop_start..loop_end);
        loop_start = loop_end;
    }
    let epsilon = 64.0 * Real::EPSILON;
    let strictly_inside = |parameter: Point2| -> Result<bool, GeometryError> {
        let point = normalization.normalize(parameter)?;
        if loop_ranges
            .iter()
            .any(|range| point_on_trim_polygon(point, &normalized_boundary[range.clone()], epsilon))
        {
            return Ok(false);
        }
        if !point_in_trim_polygon(point, &normalized_boundary[loop_ranges[0].clone()], epsilon) {
            return Ok(false);
        }
        Ok(!loop_ranges[1..].iter().any(|range| {
            point_in_trim_polygon(point, &normalized_boundary[range.clone()], epsilon)
        }))
    };

    let sample_direction = |spans: Vec<(Real, Real)>,
                            samples_per_span: usize,
                            knots: &[Real],
                            degree: usize|
     -> Result<Vec<Real>, GeometryError> {
        let capacity = spans
            .len()
            .checked_mul(samples_per_span)
            .and_then(|count| count.checked_add(1))
            .ok_or(GeometryError::TooManyMeshVertices)?;
        if let Some(limit) = axis_limit.filter(|limit| capacity > *limit) {
            let interval = [spans[0].0, spans.last().unwrap().1];
            let mut stations = vec![interval[0], interval[1]];
            for group in knots.chunk_by(|a, b| a == b) {
                if group[0] <= interval[0] || group[0] >= interval[1] {
                    continue;
                }
                if group.len() > degree {
                    return Err(GeometryError::MeshSubdivisionLimit);
                }
                if group.len() == degree {
                    stations.push(group[0]);
                }
            }
            // Creases are mandatory grid stations. Only the extra stations
            // on smooth intervals use the requested density limit.
            let divisions = limit.saturating_sub(stations.len()) + 1;
            for i in 1..divisions {
                stations.push(normalized_span_parameter(
                    interval,
                    i as Real / divisions as Real,
                )?);
            }
            stations.sort_by(Real::total_cmp);
            stations.dedup();
            return Ok(stations);
        }
        if capacity > MAX_CONSTRAINED_TRIM_VERTICES {
            return Err(GeometryError::TooManyMeshVertices);
        }
        let mut result = Vec::with_capacity(capacity);
        for (span_index, span) in spans.into_iter().enumerate() {
            let first_sample = usize::from(span_index != 0);
            for sample in first_sample..=samples_per_span {
                result.push(normalized_span_parameter(
                    [span.0, span.1],
                    sample as Real / samples_per_span as Real,
                )?);
            }
        }
        Ok(result)
    };
    let parameters_u = sample_direction(
        surface.spans_u().collect(),
        resolution[0],
        surface.knots_u(),
        surface.degree_u(),
    )?;
    let parameters_v = sample_direction(
        surface.spans_v().collect(),
        resolution[1],
        surface.knots_v(),
        surface.degree_v(),
    )?;
    if parameters_u
        .len()
        .checked_mul(parameters_v.len())
        .is_none_or(|count| count > MAX_CONSTRAINED_TRIM_VERTICES)
    {
        return Err(GeometryError::TooManyMeshVertices);
    }
    for v in parameters_v {
        for &u in &parameters_u {
            let parameter = Point2::try_new(u, v)?;
            if strictly_inside(parameter)? {
                if parameters.len() == MAX_CONSTRAINED_TRIM_VERTICES {
                    return Err(GeometryError::TooManyMeshVertices);
                }
                parameters.push(parameter);
            }
        }
    }
    Ok(())
}
