use super::*;
mod cusp;

pub(super) const SPARSE_MESH_VERTEX_LIMIT: usize = 2 * 1_048_576;
pub(super) const SPARSE_POLYGON_VERTEX_LIMIT: usize = 5 * 1_048_576;

fn grid_axes(spans: [usize; 2], samples: [usize; 2]) -> [usize; 2] {
    std::array::from_fn(|axis| spans[axis].saturating_mul(samples[axis]).saturating_add(1))
}

fn sparse_grid_axes(axes: [usize; 2], limit: usize) -> [usize; 2] {
    // A thin face can spend its unused short-axis budget along its long
    // axis. The product still fits the same per-face allocation.
    let short = usize::from(axes[1] < axes[0]);
    let long = 1 - short;
    let mut retained = axes;
    retained[short] = axes[short].min(limit);
    retained[long] = axes[long].min(limit.saturating_mul(limit) / retained[short]);
    retained
}

fn sparse_uniform_stations(
    [start, end]: [Real; 2],
    divisions: usize,
) -> Result<Vec<Real>, GeometryError> {
    let magnitude = start.abs().max(end.abs());
    let quantum = magnitude.next_up() - magnitude;
    let origin = (start / quantum).round() * quantum;
    let step = ((end - start) / divisions as Real / quantum).round() * quantum;
    let station = |i: usize| step.mul_add(i as Real, origin);
    // Keep interior grid coordinates on one binary lattice. Independent
    // interpolation can move consecutive points on a diagonal to opposite
    // sides of their chord. On a curved support, the resulting UV sliver
    // can lift into a large triangle.
    if divisions > 1 && step > 0. && station(1) > start && station(divisions - 1) < end {
        return Ok((1..divisions).map(station).collect());
    }
    (1..divisions)
        .map(|i| normalized_span_parameter([start, end], i as Real / divisions as Real))
        .collect()
}

pub(super) fn parameter_bounds(points: &[Point2]) -> [[Real; 2]; 2] {
    let mut bounds = [[Real::INFINITY, Real::NEG_INFINITY]; 2];
    for point in points {
        for (axis, uv) in point.to_array().into_iter().enumerate() {
            bounds[axis][0] = bounds[axis][0].min(uv);
            bounds[axis][1] = bounds[axis][1].max(uv);
        }
    }
    bounds
}

#[derive(Clone, Copy)]
pub(super) struct Sampling<'a> {
    pub edge: usize,
    grids: Option<&'a [[usize; 2]]>,
    pub axis_limit: Option<usize>,
    trim_resolution: Option<usize>,
    vertex_limit: usize,
    pub display_edge_samples: Option<usize>,
    pub chord_density: Option<Real>,
}
impl<'a> Sampling<'a> {
    pub fn uniform(samples: usize) -> Self {
        Self {
            edge: samples,
            grids: None,
            axis_limit: None,
            trim_resolution: None,
            vertex_limit: u32::MAX as usize,
            display_edge_samples: None,
            chord_density: None,
        }
    }
    pub fn per_face(grids: &'a [[usize; 2]]) -> Self {
        Self {
            edge: grids.iter().flatten().copied().max().unwrap_or(1),
            grids: Some(grids),
            axis_limit: None,
            trim_resolution: None,
            vertex_limit: 1_048_576,
            display_edge_samples: None,
            chord_density: None,
        }
    }
    pub fn with_axis_limit(mut self, limit: usize) -> Self {
        self.axis_limit = Some(limit);
        self.vertex_limit = SPARSE_MESH_VERTEX_LIMIT;
        self
    }
    pub fn with_vertex_limit(mut self, limit: usize) -> Self {
        self.vertex_limit = limit;
        self
    }
    pub fn with_display_edge_samples(mut self, samples: usize) -> Self {
        self.display_edge_samples = Some(samples);
        self
    }
    pub fn with_chord_density(mut self, density: Real) -> Self {
        self.chord_density = (density > 0.).then_some(density);
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
        self.vertex_limit
    }
    pub fn has_per_face_grids(self) -> bool {
        self.grids.is_some()
    }
    pub fn uses_sparse_grid(self, face: usize, surface: &NurbsSurface) -> bool {
        self.axis_limit.is_some_and(|limit| {
            let axes = grid_axes(
                [surface.spans_u().count(), surface.spans_v().count()],
                self.grid(face),
            );
            sparse_grid_axes(axes, limit) != axes
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
            let count = if let Some(limit) = self.axis_limit {
                let [nu, nv] = sparse_grid_axes(grid_axes([spans_u, spans_v], [u, v]), limit);
                nu.checked_mul(nv)
            } else {
                u.checked_add(1)
                    .and_then(|u| v.checked_add(1).and_then(|v| u.checked_mul(v)))
                    .and_then(|n| n.checked_mul(spans_u))
                    .and_then(|n| n.checked_mul(spans_v))
            };
            total = total
                .checked_add(count.ok_or(GeometryError::MeshSubdivisionLimit)?)
                .ok_or(GeometryError::MeshSubdivisionLimit)?;
            if total > self.vertex_limit() {
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
    if !jagged_seams {
        return sample_trim_loop(boundary, resolution);
    }
    let surface = &face.surface;
    let extra = trim_cusp_stations(face, boundary, resolution)?;
    let mut samples = sample_trim_loop_with_stations(boundary, resolution, &extra)?;
    // UV endpoint roundoff can leave two nearly identical polygon vertices.
    // Keep conforming seam samples intact. On independent faces, eliminate
    // only neighboring duplicates that also coincide in the sampled image.
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

pub(super) fn trim_cusp_stations(
    face: &BrepFace,
    boundary: &BrepLoop,
    resolution: usize,
) -> Result<Vec<Vec<Real>>, GeometryError> {
    let surface = &face.surface;
    let widths = [surface.domain_u(), surface.domain_v()].map(|d| d.end() - d.start());
    cusp::stations(boundary, resolution, widths)
}

pub(super) fn append_trimmed_surface_grid_parameters(
    parameters: &mut Vec<Point2>,
    loop_lengths: &[usize],
    surface: &NurbsSurface,
    resolution: [usize; 2],
    axis_limit: Option<usize>,
    balance_display_grid: bool,
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
                            degree: usize,
                            axis_limit: Option<usize>|
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
            stations.extend(sparse_uniform_stations(interval, divisions)?);
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
    let mut spans_u: Vec<_> = surface.spans_u().collect();
    let mut spans_v: Vec<_> = surface.spans_v().collect();
    let mut resolution = resolution;
    if axis_limit.is_some() {
        let bounds = parameter_bounds(&parameters[..loop_lengths[0]]);
        for (axis, spans) in [&mut spans_u, &mut spans_v].into_iter().enumerate() {
            let [lo, hi] = bounds[axis];
            if lo > spans[0].0 || hi < spans.last().unwrap().1 {
                spans.retain_mut(|span| {
                    span.0 = span.0.max(lo);
                    span.1 = span.1.min(hi);
                    span.0 < span.1
                });
                // A linear direction still needs an interior station if its
                // natural endpoints are both outside the trimmed material.
                resolution[axis] = resolution[axis].max(2);
            }
        }
        if spans_u.is_empty() || spans_v.is_empty() {
            return Ok(());
        }
    }
    let limits = axis_limit.map(|limit| {
        let wanted = grid_axes([spans_u.len(), spans_v.len()], resolution);
        if balance_display_grid
            && wanted.iter().all(|&count| count >= limit)
            && let Ok(aspect) = super::metric::aspect(surface, &parameters[..loop_lengths[0]])
            && aspect != [1.; 2]
        {
            // Keep the same station budget while shortening chords along
            // the longer physical direction of the trimmed patch.
            let short = usize::from(aspect[1] < aspect[0]);
            let long = 1 - short;
            // Keep at least a third of the short-axis station budget.
            let factor = aspect[short].recip().sqrt().min(3.);
            let mut counts = [0; 2];
            counts[long] = wanted[long].min((limit as Real * factor) as usize);
            counts[short] = wanted[short].min(limit * limit / counts[long]);
            counts
        } else {
            sparse_grid_axes(wanted, limit)
        }
    });
    let parameters_u = sample_direction(
        spans_u,
        resolution[0],
        surface.knots_u(),
        surface.degree_u(),
        limits.map(|axes| axes[0]),
    )?;
    let parameters_v = sample_direction(
        spans_v,
        resolution[1],
        surface.knots_v(),
        surface.degree_v(),
        limits.map(|axes| axes[1]),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_grids_budget_cells_across_both_axes() {
        assert_eq!(sparse_grid_axes([257, 5], 16), [51, 5]);
        assert_eq!(sparse_grid_axes([5, 31], 16), [5, 31]);
        assert_eq!(sparse_grid_axes([19, 45], 16), [16, 16]);
    }

    #[test]
    fn sparse_grid_preserves_collinearity_on_translated_parameter_diagonals() {
        use crate::exact_scalar::rational;
        let u = sparse_uniform_stations([0., 1.], 45).unwrap();
        let v = sparse_uniform_stations([25., 33.], 45).unwrap();
        let [a, b, c] =
            [(42, 21), (41, 22), (40, 23)].map(|(i, j)| [rational(u[i - 1]), rational(v[j - 1])]);
        assert_eq!(
            (&b[0] - &a[0]) * (&c[1] - &a[1]),
            (&b[1] - &a[1]) * (&c[0] - &a[0]),
        );
        assert!((u[41] - 42. / 45.).abs() < 1e-13);
        assert!((v[20] - (25. + 21. * 8. / 45.)).abs() < 1e-12);
    }

    #[test]
    fn sparse_grid_places_stations_inside_a_small_trim_on_a_large_support() {
        let surface = NurbsSurface::try_bilinear(
            [(0., 0.), (1., 0.), (1., 1.), (0., 1.)]
                .map(|(x, y)| Point3::try_new(x, y, x * y).unwrap()),
        )
        .unwrap();
        let mut uv = [(0.125, 0.25), (0.375, 0.25), (0.375, 0.75), (0.125, 0.75)]
            .map(|(u, v)| Point2::try_new(u, v).unwrap())
            .to_vec();
        append_trimmed_surface_grid_parameters(&mut uv, &[4], &surface, [1, 1], Some(16), false)
            .unwrap();
        assert_eq!(&uv[4..], &[Point2::try_new(0.25, 0.5).unwrap()]);
    }
}
