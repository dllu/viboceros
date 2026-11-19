//! Conforming triangulation for independently parameterized shared boundaries.

use std::collections::HashSet;

use super::audit::key as vertex_location;
use super::sampling::trim_cusp_stations;
use super::trim_image::LiftedTrim;
use super::*;
use crate::ParameterSide;

struct FaceTriangulation {
    parameters: Vec<Point2>,
    points: Vec<Point3>,
    triangles: Vec<[u32; 3]>,
    chord_height: Option<Real>,
}

impl Brep {
    /// Rebuild boundaries from one sample table per topological edge. Merely
    /// snapping independently sampled face grids cannot remove T-junctions.
    #[cfg(test)]
    pub(super) fn tessellate_conforming(
        &self,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        self.tessellate_conforming_with_face_sources(samples_per_span, tolerance)
            .map(|(mesh, _)| mesh)
    }

    #[cfg(test)]
    pub(super) fn tessellate_conforming_with_face_sources(
        &self,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<(TriangleMesh, Vec<usize>), GeometryError> {
        self.tessellate_conforming_impl(samples_per_span, tolerance, false)
            .map(|(mesh, sources, _)| (mesh, sources))
    }

    pub(super) fn tessellate_conforming_impl(
        &self,
        samples_per_span: usize,
        tolerance: Tolerance,
        retain_parameters: bool,
    ) -> Result<(TriangleMesh, Vec<usize>, Vec<Point2>), GeometryError> {
        self.tessellate_conforming_sampling(
            Sampling::uniform(samples_per_span),
            tolerance,
            retain_parameters,
        )
    }

    pub(super) fn tessellate_conforming_with_chords(
        &self,
        sampling: Sampling<'_>,
        tolerance: Tolerance,
        retain_parameters: bool,
        density: Real,
    ) -> Result<(TriangleMesh, Vec<usize>, Vec<Point2>), GeometryError> {
        self.tessellate_conforming_sampling(
            sampling.with_chord_density(density),
            tolerance,
            retain_parameters,
        )
        .or_else(|error| match error {
            GeometryError::MeshSubdivisionLimit
            | GeometryError::UnstitchedBrepTessellation { .. }
                if density > 0. =>
            {
                self.tessellate_conforming_sampling(sampling, tolerance, retain_parameters)
            }
            _ => Err(error),
        })
    }

    pub(super) fn tessellate_conforming_sampling(
        &self,
        sampling: Sampling<'_>,
        tolerance: Tolerance,
        retain_parameters: bool,
    ) -> Result<(TriangleMesh, Vec<usize>, Vec<Point2>), GeometryError> {
        sampling.validate(&self.faces)?;
        let samples_per_span = sampling.edge;
        let frames = self
            .faces
            .iter()
            .map(BrepFace::sampling_parameter_frame)
            .collect::<Result<Vec<_>, _>>()?;
        // The UV triangulation below does not split internal positional
        // breaks. Never "repair" a naked interior jump by bridging it. Even
        // coincident full-order limits are conservatively unsupported here.
        for (index, frame) in frames.iter().enumerate() {
            let face = frame.face.as_ref();
            for (knots, degree, domain) in [
                (
                    face.surface.knots_u(),
                    face.surface.degree_u(),
                    face.surface.domain_u(),
                ),
                (
                    face.surface.knots_v(),
                    face.surface.degree_v(),
                    face.surface.domain_v(),
                ),
            ] {
                if knots.chunk_by(|a, b| a == b).any(|group| {
                    group.len() > degree && group[0] > *domain.start() && group[0] < *domain.end()
                }) {
                    return Err(GeometryError::UnsupportedBrepTrimTessellation { face: index });
                }
            }
        }
        let search = Tolerance::try_new(
            (tolerance.absolute() * 0.01).max(Real::MIN_POSITIVE),
            tolerance.relative(),
            tolerance.angular(),
        )?;
        let mut edge_resolution = vec![samples_per_span; self.edges.len()];
        if let Some(samples) = sampling.display_edge_samples {
            // Keep simple curved boundaries smooth without multiplying the
            // authored span count of already finely segmented display edges.
            for (edge, resolution) in self.edges.iter().zip(&mut edge_resolution) {
                *resolution = if edge.curve.degree() > 1 {
                    samples.div_ceil(edge.curve.spans().count().max(1))
                } else {
                    1
                };
            }
        } else if sampling.has_per_face_grids() {
            edge_resolution.fill(1);
            for (index, frame) in frames.iter().enumerate() {
                let grid = sampling.grid(index);
                for trim in frame.face.loops.iter().flat_map(|boundary| &boundary.trims) {
                    if let Some(edge) = trim.edge
                        && self.edges[edge].curve.degree() > 1
                    {
                        edge_resolution[edge] = edge_resolution[edge].max(grid[0].max(grid[1]));
                    }
                }
            }
        }
        let mut stations = self
            .edges
            .iter()
            .zip(&edge_resolution)
            .map(|(edge, &resolution)| sample_spans(edge.curve.spans(), resolution))
            .collect::<Result<Vec<_>, _>>()?;
        let mut cusp_edges = vec![false; stations.len()];
        // Preserve p-curve corners even when the shared model-space edge uses
        // a different knot layout or parameterization. Extra samples next to
        // tangent cusps must also be shared by all incident faces.
        for (index, frame) in frames.iter().enumerate() {
            let face = frame.face.as_ref();
            let [u, v] = sampling.grid(index);
            for boundary in &face.loops {
                let extra = trim_cusp_stations(face, boundary, u.max(v))?;
                for (i, trim) in boundary.trims.iter().enumerate() {
                    let Some(index) = trim.edge else { continue };
                    let edge = &self.edges[index];
                    let additions = extra.get(i).map_or(&[][..], Vec::as_slice);
                    cusp_edges[index] |= !additions.is_empty();
                    let knots = sample_spans(trim.curve.spans(), 1)?;
                    for t in knots.into_iter().chain(additions.iter().copied()) {
                        let uv = trim.curve.evaluate(t)?;
                        let p = face.surface.evaluate(uv.x(), uv.y())?;
                        let fraction = fraction(t, trim.curve.domain())?;
                        let fraction = if trim.reversed_3d {
                            1.0 - fraction
                        } else {
                            fraction
                        };
                        let direct = edge.curve.parameter_at(fraction)?;
                        let parameter =
                            if edge.curve.evaluate(direct)?.distance_to(p)? <= search.absolute() {
                                direct
                            } else {
                                edge.curve.closest_parameter(p, search)?
                            };
                        stations[index].push(parameter);
                        if stations[index].len() > MAX_CONSTRAINED_TRIM_VERTICES {
                            return Err(GeometryError::TooManyMeshVertices);
                        }
                    }
                }
            }
        }
        let edge_samples = stations
            .into_iter()
            .zip(&self.edges)
            .zip(cusp_edges)
            .map(|((mut ts, edge), cusp)| {
                ts.sort_by(Real::total_cmp);
                ts.dedup();
                let domain = edge.curve.domain();
                let spacing = if cusp {
                    search.absolute()
                } else {
                    tolerance.absolute()
                };
                let mut samples = Vec::<(Real, Point3)>::new();
                for t in ts {
                    let point = if t == *domain.start() {
                        self.vertices[edge.vertices[0]].point
                    } else if t == *domain.end() {
                        self.vertices[edge.vertices[1]].point
                    } else {
                        edge.curve.evaluate(t)?
                    };
                    if let Some((previous_t, previous)) = samples.last_mut()
                        && previous.distance_to(point)? <= spacing
                    {
                        if t != *domain.end() {
                            continue;
                        }
                        if *previous_t != *domain.start() {
                            *previous_t = t;
                            *previous = point;
                            continue;
                        }
                        // The two topological endpoints delimit the same
                        // boundary on its incident faces even on a short edge.
                        // Interior spacing must not merge those vertices.
                    }
                    samples.push((t, point));
                }
                if samples.len() < 2 {
                    return invalid("conforming tessellation collapsed an entire edge");
                }
                Ok(samples)
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;

        let mut face_meshes = Vec::with_capacity(frames.len());
        let mut coarse_vertices = 0;
        for (index, frame) in frames.iter().enumerate() {
            let face = frame.face.as_ref();
            let mut parameters = Vec::new();
            let mut boundary_points = Vec::new();
            let mut lengths = Vec::new();
            for boundary in &face.loops {
                let start = parameters.len();
                for trim in &boundary.trims {
                    let domain = trim.curve.domain();
                    let Some(edge_index) = trim.edge else {
                        let ts = sample_spans(trim.curve.spans(), samples_per_span)?;
                        for &t in &ts[..ts.len() - 1] {
                            push_boundary(
                                &mut parameters,
                                &mut boundary_points,
                                trim.curve.evaluate(t)?,
                                self.vertices[trim.vertices[0]].point,
                            )?;
                        }
                        continue;
                    };
                    let edge = &self.edges[edge_index];
                    let image = LiftedTrim::new(trim, &face.surface)?;
                    let mut seeds = None;
                    let samples = &edge_samples[edge_index];
                    let mut previous = *domain.start();
                    for i in 0..samples.len() - 1 {
                        let (edge_t, point) = samples[if trim.reversed_3d {
                            samples.len() - 1 - i
                        } else {
                            i
                        }];
                        let t = if i == 0 {
                            *domain.start()
                        } else {
                            let fraction = fraction(edge_t, edge.curve.domain())?;
                            let fraction = if trim.reversed_3d {
                                1.0 - fraction
                            } else {
                                fraction
                            };
                            let direct = trim.curve.parameter_at(fraction)?;
                            if image
                                .point(direct, ParameterSide::Right)?
                                .distance_to(point)?
                                <= search.absolute()
                            {
                                direct
                            } else {
                                let seeds = match &mut seeds {
                                    Some(seeds) => seeds,
                                    empty @ None => empty.insert(
                                        sample_spans(trim.curve.spans(), 8)?
                                            .into_iter()
                                            .map(|t| {
                                                Ok((
                                                    t,
                                                    ParameterSide::Right,
                                                    image.point(t, ParameterSide::Right)?,
                                                ))
                                            })
                                            .collect::<Result<Vec<_>, GeometryError>>()?,
                                    ),
                                };
                                image.distance_witness(point, seeds, search.absolute())?.1
                            }
                        };
                        if i != 0 && (t <= previous || t >= *domain.end()) {
                            return invalid("shared edge samples do not follow the trim in order");
                        }
                        let uv = trim.curve.evaluate(t)?;
                        let allowed = tolerance.absolute().max(edge.tolerance).max(if i == 0 {
                            self.vertices[trim.vertices[0]].tolerance
                        } else {
                            0.0
                        });
                        if face.surface.evaluate(uv.x(), uv.y())?.distance_to(point)? > allowed {
                            return invalid("shared tessellation sample leaves its incident face");
                        }
                        push_boundary(&mut parameters, &mut boundary_points, uv, point)?;
                        previous = t;
                    }
                }
                lengths.push(parameters.len() - start);
            }
            append_trimmed_surface_grid_parameters(
                &mut parameters,
                &lengths,
                &face.surface,
                sampling.grid(index),
                sampling.axis_limit,
            )?;
            let triangles = if retain_parameters && sampling.display_edge_samples.is_none() {
                creases::triangulate(&mut parameters, &lengths, &face.surface, index)?
            } else if sampling.axis_limit.is_some() {
                metric::triangulate(&mut parameters, &lengths, &face.surface)?
            } else {
                trim_region::triangulate(&mut parameters, &lengths, &[])?
            }
            .ok_or(GeometryError::UnsupportedBrepTrimTessellation { face: index })?;
            let points = parameters
                .iter()
                .enumerate()
                .map(|(i, uv)| match boundary_points.get(i) {
                    Some(&point) => Ok(point),
                    None => face.surface.evaluate(uv.x(), uv.y()),
                })
                .collect::<Result<Vec<_>, _>>()?;
            let chord_height = sampling
                .chord_density
                .filter(|_| face.surface.degree_u() > 1 || face.surface.degree_v() > 1)
                .map(|density| face.surface.polygon_chord_height(density))
                .transpose()?
                .map(|height| height.max(tolerance.absolute()));
            coarse_vertices += points.len();
            if coarse_vertices > sampling.vertex_limit() {
                return Err(GeometryError::MeshSubdivisionLimit);
            }
            face_meshes.push(FaceTriangulation {
                parameters,
                points,
                triangles,
                chord_height,
            });
        }

        let mut vertices = Vec::with_capacity(coarse_vertices);
        let mut triangles = Vec::new();
        let mut face_sources = Vec::new();
        let mut vertex_parameters = Vec::new();
        let vertex_limit = sampling.vertex_limit().min(u32::MAX as usize);
        let mut remaining = vertex_limit - coarse_vertices;
        let face_allowance = vertex_limit / frames.len();
        let mut locations = sampling.chord_density.map(|_| {
            let mut occupied = HashSet::with_capacity(coarse_vertices);
            occupied.extend(
                face_meshes
                    .iter()
                    .flat_map(|mesh| mesh.points.iter().copied().map(vertex_location)),
            );
            occupied
        });
        for (index, (frame, face_mesh)) in frames.iter().zip(face_meshes).enumerate() {
            let face = frame.face.as_ref();
            let FaceTriangulation {
                mut parameters,
                mut points,
                triangles: mut face_triangles,
                chord_height,
            } = face_mesh;
            if let (Some(chord_height), Some(locations)) = (chord_height, &mut locations) {
                // Reserve every coarse face before sharing the remaining
                // budget among interior chords.
                let coarse = points.len();
                let allowance = remaining.div_ceil(frames.len() - index).min(face_allowance);
                let limit = points
                    .len()
                    .saturating_add(allowance)
                    .min(MAX_CONSTRAINED_TRIM_VERTICES);
                chords::refine_interior(
                    &face.surface,
                    &mut parameters,
                    &mut points,
                    &mut face_triangles,
                    chord_height,
                    limit,
                    locations,
                )?;
                remaining -= points.len() - coarse;
            }
            let offset =
                u32::try_from(vertices.len()).map_err(|_| GeometryError::TooManyMeshVertices)?;
            if retain_parameters {
                vertex_parameters.extend(parameters);
            }
            vertices.extend(points);
            for triangle in face_triangles {
                let mut triangle = triangle.map(|i| i + offset);
                if face.reversed {
                    triangle.swap(1, 2);
                }
                if crate::nurbs_surface::tessellation_triangle_is_nondegenerate(
                    &vertices,
                    triangle,
                    Tolerance::MESH_VALIDATION,
                )? {
                    triangles.push(triangle);
                    face_sources.push(index);
                }
            }
        }
        drop(locations);
        let mesh = TriangleMesh::try_new(vertices, triangles, Tolerance::MESH_VALIDATION)?;
        if !self.mesh_boundary_conforms(&mesh, &face_sources, samples_per_span, tolerance)? {
            let topology = mesh.topology();
            return Err(GeometryError::UnstitchedBrepTessellation {
                boundary_edges: topology.boundary_edge_count(),
                orientation_conflicts: topology.orientation_conflict_edge_count(),
            });
        }
        Ok((mesh, face_sources, vertex_parameters))
    }
}

fn sample_spans(
    spans: impl Iterator<Item = (Real, Real)>,
    count: usize,
) -> Result<Vec<Real>, GeometryError> {
    let mut result = Vec::new();
    for (start, end) in spans {
        for i in 0..=count {
            if result.len() == MAX_CONSTRAINED_TRIM_VERTICES {
                return Err(GeometryError::TooManyMeshVertices);
            }
            result.push(normalized_span_parameter(
                [start, end],
                i as Real / count as Real,
            )?);
        }
    }
    result.dedup();
    Ok(result)
}

fn push_boundary(
    uvs: &mut Vec<Point2>,
    points: &mut Vec<Point3>,
    uv: Point2,
    point: Point3,
) -> Result<(), GeometryError> {
    if uvs.len() == MAX_CONSTRAINED_TRIM_VERTICES {
        return Err(GeometryError::TooManyMeshVertices);
    }
    uvs.push(uv);
    points.push(point);
    Ok(())
}

fn fraction(t: Real, domain: RangeInclusive<Real>) -> Result<Real, GeometryError> {
    let (a, b) = (*domain.start(), *domain.end());
    let s = if (b - a).is_finite() {
        (t - a) / (b - a)
    } else {
        let scale = a.abs().max(b.abs());
        (t / scale - a / scale) / (b / scale - a / scale)
    };
    require_finite([s], "shared edge sample parameter fraction")?;
    Ok(s.clamp(0.0, 1.0))
}
