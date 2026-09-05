//! Conforming triangulation for independently parameterized shared boundaries.

use super::trim_image::LiftedTrim;
use super::*;
use crate::ParameterSide;

impl Brep {
    /// Rebuild boundaries from one sample table per topological edge. Merely
    /// snapping independently sampled face grids cannot remove T-junctions.
    pub(super) fn tessellate_conforming(
        &self,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        // The UV triangulation below does not split internal positional
        // breaks. Never "repair" a naked interior jump by bridging it. Even
        // coincident full-order limits are conservatively unsupported here.
        for (index, face) in self.faces.iter().enumerate() {
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
        let mut stations = self
            .edges
            .iter()
            .map(|edge| sample_spans(edge.curve.spans(), samples_per_span))
            .collect::<Result<Vec<_>, _>>()?;
        // Preserve p-curve corners even when the shared model-space edge uses
        // a different knot layout or parameterization.
        for face in &self.faces {
            for trim in face.loops.iter().flat_map(|l| &l.trims) {
                let Some(index) = trim.edge else { continue };
                let edge = &self.edges[index];
                for t in sample_spans(trim.curve.spans(), 1)? {
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
        let edge_samples = stations
            .into_iter()
            .zip(&self.edges)
            .map(|(mut ts, edge)| {
                ts.sort_by(Real::total_cmp);
                ts.dedup();
                let domain = edge.curve.domain();
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
                        && previous.distance_to(point)? <= tolerance.absolute()
                    {
                        if t == *domain.end() {
                            *previous_t = t;
                            *previous = point;
                        }
                        continue;
                    }
                    samples.push((t, point));
                }
                if samples.len() < 2 {
                    return invalid("conforming tessellation collapsed an entire edge");
                }
                Ok(samples)
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;

        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        let mut face_sources = Vec::new();
        for (index, face) in self.faces.iter().enumerate() {
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
                    let seeds = sample_spans(trim.curve.spans(), 8)?
                        .into_iter()
                        .map(|t| Ok((t, image.point(t, ParameterSide::Right)?)))
                        .collect::<Result<Vec<_>, GeometryError>>()?;
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
                                image.closest_point(point, &seeds, search.absolute())?.1
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
                samples_per_span,
            )?;
            let face_triangles = triangulate_trim_region(&parameters, &lengths)?
                .ok_or(GeometryError::UnsupportedBrepTrimTessellation { face: index })?;
            let offset =
                u32::try_from(vertices.len()).map_err(|_| GeometryError::TooManyMeshVertices)?;
            if vertices
                .len()
                .checked_add(parameters.len())
                .is_none_or(|n| n > u32::MAX as usize)
            {
                return Err(GeometryError::TooManyMeshVertices);
            }
            for (i, uv) in parameters.into_iter().enumerate() {
                vertices.push(if let Some(&p) = boundary_points.get(i) {
                    p
                } else {
                    face.surface.evaluate(uv.x(), uv.y())?
                });
            }
            for triangle in face_triangles {
                let mut triangle = triangle.map(|i| i + offset);
                if face.reversed {
                    triangle.swap(1, 2);
                }
                if crate::nurbs_surface::tessellation_triangle_is_nondegenerate(
                    &vertices, triangle, tolerance,
                )? {
                    triangles.push(triangle);
                    face_sources.push(index);
                }
            }
        }
        let mesh = TriangleMesh::try_new(vertices, triangles, tolerance)?;
        if !self.mesh_boundary_conforms(&mesh, &face_sources, samples_per_span, tolerance)? {
            let topology = mesh.topology();
            return Err(GeometryError::UnstitchedBrepTessellation {
                boundary_edges: topology.boundary_edge_count(),
                orientation_conflicts: topology.orientation_conflict_edge_count(),
            });
        }
        Ok(mesh)
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
