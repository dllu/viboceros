//! Transfer physical face boundaries to a changed surface parameterization.
use super::*;
use crate::PointMorph;
mod poles;

const MAX_PROJECTED_CONTROLS: usize = 4096;

fn certified_images(
    surface: &NurbsSurface,
    uv: NurbsCurve2,
    tolerance: Tolerance,
    depth: usize,
    output: &mut Vec<(NurbsCurve2, NurbsCurve)>,
) -> Result<(), GeometryError> {
    if output.len() >= 256 {
        return Err(GeometryError::SurfaceCurveCertificateWorkLimit);
    }
    let result = if depth == 0 && (uv.degree() == 1 || uv.control_points().len() <= 32) {
        surface.try_pushup_curve_certified(&uv, tolerance)
    } else {
        let domain = uv.domain();
        let mut points = Vec::new();
        for t in [0., 1. / 3., 2. / 3., 1.] {
            let p = uv.evaluate(uv.parameter_at(t)?)?;
            points.push(surface.evaluate(p.x(), p.y())?);
        }
        let blend = |weights: [Real; 4]| -> Result<Point3, GeometryError> {
            let anchor = points[0].to_array();
            Point3::try_from(std::array::from_fn(|k| {
                points
                    .iter()
                    .zip(weights)
                    .skip(1)
                    .fold(anchor[k], |sum, (p, w)| {
                        w.mul_add(p.to_array()[k] - anchor[k], sum)
                    })
            }))
        };
        let candidate = NurbsCurve::try_new(
            3,
            vec![
                points[0],
                blend([-5. / 6., 3., -1.5, 1. / 3.])?,
                blend([1. / 3., -1.5, 3., -5. / 6.])?,
                points[3],
            ],
            [vec![*domain.start(); 4], vec![*domain.end(); 4]].concat(),
        )?;
        match surface.parameter_curve_deviation_bound(&uv, &candidate, tolerance.absolute()) {
            Ok(Some(_)) => Ok(candidate),
            Ok(None) => Err(GeometryError::SurfacePushupDidNotConverge {
                tolerance: tolerance.absolute(),
            }),
            Err(error) => Err(error),
        }
    };
    match result {
        Ok(spatial) => {
            output.push((uv, spatial));
            Ok(())
        }
        Err(
            GeometryError::SurfaceCurveCertificateWorkLimit
            | GeometryError::SurfacePushupDidNotConverge { .. },
        ) if depth < 8 && output.len() < 256 => {
            let domain = uv.domain();
            let middle = domain.start().midpoint(*domain.end());
            certified_images(
                surface,
                uv.try_trimmed(*domain.start()..=middle)?,
                tolerance,
                depth + 1,
                output,
            )?;
            certified_images(
                surface,
                uv.try_trimmed(middle..=*domain.end())?,
                tolerance,
                depth + 1,
                output,
            )
        }
        Err(error) => Err(error),
    }
}

struct Projection<'a> {
    surface: &'a NurbsSurface,
    tolerance: Tolerance,
}

struct ChartProjection<'a> {
    source: &'a NurbsSurface,
    target: &'a NurbsSurface,
    closed: [bool; 2],
    tolerance: Tolerance,
    fixed: Option<(usize, Real, NurbsCurve)>,
    curve: &'a NurbsCurve2,
}
struct ProjectionChart<'a> {
    source: &'a NurbsSurface,
    curve: &'a NurbsCurve2,
    closed: [bool; 2],
    fixed: Option<(usize, Real)>,
}
fn nearest_lift(mut projected: [Real; 2], reference: [Real; 2], closed: [bool; 2]) -> [Real; 2] {
    for axis in 0..2 {
        if closed[axis] {
            projected[axis] += (reference[axis] - projected[axis]).round();
        }
    }
    projected
}
impl PointMorph for ChartProjection<'_> {
    fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
        let station = self.curve.evaluate(self.curve.parameter_at(point.x())?)?;
        let reference = station.to_array();
        if reference
            .iter()
            .any(|&t| !(-4096. * Real::EPSILON..=1. + 4096. * Real::EPSILON).contains(&t))
        {
            return invalid("source projection station leaves its natural chart");
        }
        let xyz = self
            .source
            .evaluate(reference[0].clamp(0., 1.), reference[1].clamp(0., 1.))?;
        let (u, v) = if let Some((axis, value, curve)) = &self.fixed {
            let free = curve.closest_parameter(xyz, self.tolerance)?;
            if *axis == 0 {
                (*value, free)
            } else {
                (free, *value)
            }
        } else {
            self.target
                .closest_parameters_from_seed(xyz, reference, self.tolerance)
                .or_else(|_| self.target.closest_parameters(xyz, self.tolerance))?
        };
        let uv = nearest_lift([u, v], reference, self.closed);
        Point3::try_new(uv[0], uv[1], 0.)
    }
}
impl PointMorph for Projection<'_> {
    fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
        let (u, v) = self.surface.closest_parameters(point, self.tolerance)?;
        Point3::try_new(u, v, 0.)
    }
}

fn canonical_projection(
    mut uv: NurbsCurve2,
    parameter_limit: Real,
) -> Result<NurbsCurve2, GeometryError> {
    // Closest-point and interpolation roundoff can perturb a constant
    // coordinate. Same-sign weights bound the complete coordinate change by
    // the maximum control displacement; keep it inside the UV fitting budget.
    let a = uv.start_point()?.to_array();
    let b = uv.end_point()?.to_array();
    let sign = uv.control_points()[0].weight().is_sign_positive();
    if uv
        .control_points()
        .iter()
        .all(|p| p.weight().is_sign_positive() == sign)
    {
        for axis in 0..2 {
            if a[axis] != b[axis]
                || uv
                    .control_points()
                    .iter()
                    .any(|p| (p.point().to_array()[axis] - a[axis]).abs() > parameter_limit)
            {
                continue;
            }
            let controls = uv
                .control_points()
                .iter()
                .map(|p| {
                    let mut point = p.point().to_array();
                    point[axis] = a[axis];
                    WeightedPoint2::try_new(Point2::try_new(point[0], point[1])?, p.weight())
                })
                .collect::<Result<Vec<_>, GeometryError>>()?;
            uv = NurbsCurve2::try_new_rational(uv.degree(), controls, uv.knots().to_vec())?;
        }
    }
    // This predicate proves the complete curve image is its ordered segment.
    // Retain that exact locus and orientation with linear parameter speed so
    // tensor-knot crossings can be composed directly instead of subdivided.
    if uv.is_straight_segment() {
        let domain = uv.domain();
        NurbsCurve2::try_new(
            1,
            vec![uv.start_point()?, uv.end_point()?],
            vec![
                *domain.start(),
                *domain.start(),
                *domain.end(),
                *domain.end(),
            ],
        )
    } else {
        Ok(uv)
    }
}

fn projection_curve(
    surface: &NurbsSurface,
    spatial: &NurbsCurve,
    ends: [Point2; 2],
    tolerance: Tolerance,
    chart: Option<ProjectionChart<'_>>,
) -> Result<NurbsCurve2, GeometryError> {
    if let Ok(uv) = surface.try_pullback_exact_curve(spatial, tolerance)
        && let Some(uv) = crate::surface_pullback::constrain_curve_endpoints(uv, ends)?
        && surface
            .parameter_curve_deviation_bound(&uv, spatial, tolerance.absolute())?
            .is_some()
    {
        return canonical_projection(uv, 0.);
    }
    // In normalized UV, polynomial derivative control nets bound the amount
    // of model-space motion caused by an interpolation error in each axis.
    let mut speed = [0_f64; 2];
    let count = [
        surface.control_point_count_u(),
        surface.control_point_count_v(),
    ];
    for axis in 0..2 {
        let (degree, knots) = if axis == 0 {
            (surface.degree_u(), surface.knots_u())
        } else {
            (surface.degree_v(), surface.knots_v())
        };
        for j in 0..count[1] {
            for i in 0..count[0] {
                let index = [i, j][axis];
                if index + 1 == count[axis] {
                    continue;
                }
                let denominator = knots[index + degree + 1] - knots[index + 1];
                if denominator <= 0. {
                    continue;
                }
                let a = surface.control_points()[j * count[0] + i].point();
                let b = surface.control_points()
                    [j * count[0] + i + if axis == 0 { 1 } else { count[0] }]
                .point();
                speed[axis] = speed[axis].max(a.distance_to(b)? * degree as Real / denominator);
            }
        }
    }
    let limit = tolerance.absolute() / (4. * (speed[0] + speed[1]).max(1.));
    if !limit.is_finite() || limit <= 0. {
        return Err(GeometryError::SingularSystem);
    }
    let fitting = Tolerance::try_new(limit, tolerance.relative(), tolerance.angular())?;
    let numerical = Tolerance::try_new(
        (tolerance.absolute() * 1e-4).max(Real::MIN_POSITIVE),
        (tolerance.relative() * 1e-4).max(Real::MIN_POSITIVE),
        tolerance.angular(),
    )?;
    let fitted = if let Some(ProjectionChart {
        source,
        curve: uv,
        closed,
        fixed,
    }) = chart
    {
        let domain = uv.domain();
        let lifted = NurbsCurve::try_new(
            1,
            vec![Point3::try_new(0., 0., 0.)?, Point3::try_new(1., 0., 0.)?],
            vec![
                *domain.start(),
                *domain.start(),
                *domain.end(),
                *domain.end(),
            ],
        )?;
        if uv.spans().count() > 2048 {
            return Err(GeometryError::TooManyMorphCurveControlPoints {
                maximum: MAX_PROJECTED_CONTROLS,
            });
        }
        let mut stations = uv
            .spans()
            .flat_map(|(a, b)| {
                (0..=16).map(move |i| a.mul_add(1. - i as Real / 16., b * (i as Real / 16.)))
            })
            .collect::<Vec<_>>();
        stations.sort_by(Real::total_cmp);
        stations.dedup();
        let fixed = fixed
            .map(|(axis, value)| {
                let curve = if axis == 0 {
                    surface.isocurve_v(value)?
                } else {
                    surface.isocurve_u(value)?
                };
                Ok::<_, GeometryError>((axis, value, curve))
            })
            .transpose()?;
        crate::morph::fit_curve_with_control_limit_and_stations(
            &ChartProjection {
                source,
                target: surface,
                closed,
                tolerance: numerical,
                fixed,
                curve: uv,
            },
            &lifted,
            fitting,
            MAX_PROJECTED_CONTROLS,
            &stations,
        )?
    } else {
        crate::morph::fit_curve_with_control_limit(
            &Projection {
                surface,
                tolerance: numerical,
            },
            spatial,
            fitting,
            MAX_PROJECTED_CONTROLS,
        )?
    };
    let uv = NurbsCurve2::try_new_rational(
        fitted.degree(),
        fitted
            .control_points()
            .iter()
            .map(|p| {
                WeightedPoint2::try_new(Point2::try_new(p.point().x(), p.point().y())?, p.weight())
            })
            .collect::<Result<Vec<_>, GeometryError>>()?,
        fitted.knots().to_vec(),
    )?;
    let uv = crate::surface_pullback::constrain_curve_endpoints(uv, ends)?.ok_or(
        GeometryError::InvalidBrepTopology {
            context: "projected trim endpoints cannot be constrained",
        },
    )?;
    canonical_projection(uv, limit)
}

impl Brep {
    /// Project a single face's spatial edges onto a changed polynomial surface,
    /// retaining loop order, shared vertices, face orientation and source UV domains.
    /// Exact pullbacks are certified when available. Otherwise the bounded
    /// closest-point projection fit has sampled accuracy checks; its resulting
    /// spatial edges have continuous certificates against the new UV trims.
    /// Natural singular sides require an exactly collapsed target boundary and
    /// retain their UV intervals and shared pole vertices without spatial edges.
    /// Projection is not a certified global nearest-point or topology guarantee.
    pub fn try_retrimmed_single_surface(
        &self,
        surface: NurbsSurface,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let [face] = self.faces.as_slice() else {
            return invalid("retrimming requires one face");
        };
        if surface.is_rational() {
            return invalid("retrimming requires a polynomial target surface");
        }
        // Native natural faces retain their complete target boundary. Construct
        // exact isocurves directly, preserving the source's numeric topology,
        // instead of fitting an inverse parameter map along the same boundary.
        if face.loops.len() == 1
            && face.loops[0].trims.len() == 4
            && face.is_untrimmed(tolerance)?
        {
            let surface =
                surface.try_reparameterized(face.surface.domain_u(), face.surface.domain_v())?;
            let natural = Self::try_surface_face(surface.clone(), tolerance)?;
            let exact_sides = face.loops[0].trims.iter().all(|old| {
                old.curve.is_straight_segment()
                    && natural.faces[0].loops[0]
                        .trims
                        .iter()
                        .find(|t| t.iso == old.iso)
                        .is_some_and(|new| {
                            old.curve
                                .start_point()
                                .is_ok_and(|p| new.curve.start_point().is_ok_and(|q| p == q))
                                && old
                                    .curve
                                    .end_point()
                                    .is_ok_and(|p| new.curve.end_point().is_ok_and(|q| p == q))
                        })
            });
            let same_incidence = natural.vertices.len() == self.vertices.len()
                && natural.edges.len() == self.edges.len()
                && face.loops[0].trims.iter().all(|old| {
                    natural.faces[0].loops[0]
                        .trims
                        .iter()
                        .find(|t| t.iso == old.iso)
                        .is_some_and(|new| {
                            new.trim_type == old.trim_type
                                && new.edge.is_some() == old.edge.is_some()
                        })
                });
            if same_incidence && exact_sides {
                let mut vertices = self.vertices.clone();
                let mut edges = self.edges.clone();
                let mut loops = face.loops.clone();
                let mut used = std::collections::BTreeSet::new();
                for old in &mut loops[0].trims {
                    let Some(new) = natural.faces[0].loops[0]
                        .trims
                        .iter()
                        .find(|t| t.iso == old.iso)
                    else {
                        return invalid("natural retrim boundary class is not recognized");
                    };
                    if !used.insert(new.iso as usize) {
                        return invalid("natural retrim repeats a boundary side");
                    }
                    if let (Some(old_edge), Some(new_edge)) = (old.edge, new.edge) {
                        let mut edge = natural.edges[new_edge].clone();
                        if new.reversed_3d != old.reversed_3d {
                            edge.curve = edge.curve.reversed()?;
                        }
                        edge.vertices = self.edges[old_edge].vertices;
                        edges[old_edge] = edge;
                    }
                    for (a, b) in old.vertices.into_iter().zip(new.vertices) {
                        vertices[a] = natural.vertices[b];
                    }
                    let vertices = old.vertices;
                    let edge = old.edge;
                    let reversed = old.reversed_3d;
                    *old = new.clone();
                    old.vertices = vertices;
                    old.edge = edge;
                    old.reversed_3d = reversed;
                }
                return Self::try_new(
                    vertices,
                    edges,
                    vec![BrepFace::try_new(surface, face.reversed, loops)?],
                    tolerance,
                );
            }
        }
        if face.loops.iter().flat_map(|l| &l.trims).any(|t| {
            !matches!(
                t.trim_type,
                BrepTrimType::Boundary | BrepTrimType::Seam | BrepTrimType::Singular
            )
        }) {
            return invalid("retrimming requires boundary, seam or singular trims");
        }
        let normalized = surface.try_reparameterized(0. ..=1., 0. ..=1.)?;
        let closed = [normalized.is_closed_u()?, normalized.is_closed_v()?];
        let original_normalized = face.surface.try_reparameterized(0. ..=1., 0. ..=1.)?;
        let poles = poles::prepare(face, &normalized)?;
        let numerical = Tolerance::try_new(
            (tolerance.absolute() * 1e-4).max(Real::MIN_POSITIVE),
            (tolerance.relative() * 1e-4).max(Real::MIN_POSITIVE),
            tolerance.angular(),
        )?;
        let parameters = self
            .vertices
            .iter()
            .enumerate()
            .map(|(index, v)| {
                if let Some(pole) = poles.get(&index) {
                    return Ok(pole.parameter);
                }
                let (u, v) = normalized.closest_parameters(v.point, numerical)?;
                Point2::try_new(u, v)
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let mut vertices = self.vertices.clone();
        for (index, (vertex, uv)) in vertices.iter_mut().zip(&parameters).enumerate() {
            vertex.point = if let Some(pole) = poles.get(&index) {
                pole.point
            } else {
                normalized.evaluate(uv.x(), uv.y())?
            };
            vertex.tolerance = tolerance.absolute();
        }
        let mut edges = self.edges.clone();
        let mut loops = face.loops.clone();
        let domains = [face.surface.domain_u(), face.surface.domain_v()];
        let target = normalized.try_reparameterized(domains[0].clone(), domains[1].clone())?;
        let natural = Self::try_surface_face(target.clone(), tolerance)?;
        let original_uses = self.faces[0].loops.iter().flat_map(|l| &l.trims).fold(
            vec![0usize; self.edges.len()],
            |mut counts, t| {
                if let Some(i) = t.edge {
                    counts[i] += 1;
                }
                counts
            },
        );
        let mut replacements = std::collections::BTreeMap::<(usize, usize), Vec<BrepTrim>>::new();
        for (loop_index, face_loop) in loops.iter_mut().enumerate() {
            for (trim_index, trim) in face_loop.trims.iter_mut().enumerate() {
                if trim.trim_type == BrepTrimType::Singular {
                    // The complete target side was proved exactly constant.
                    // Keep this trim's own UV interval and the shared pole index.
                    trim.iso = trim_iso::classify(&trim.curve, &target);
                    trim.tolerance = [0.; 2];
                    continue;
                }
                let index = trim.edge.ok_or(GeometryError::InvalidBrepTopology {
                    context: "retrimming needs spatial edges",
                })?;
                if let Some(candidate) = natural.faces[0].loops[0]
                    .trims
                    .iter()
                    .find(|t| t.iso == trim.iso && t.edge.is_some())
                    && trim.curve.is_straight_segment()
                    && trim.curve.start_point()? == candidate.curve.start_point()?
                    && trim.curve.end_point()? == candidate.curve.end_point()?
                {
                    let mut edge = natural.edges[candidate.edge.unwrap()].clone();
                    if candidate.reversed_3d != trim.reversed_3d {
                        edge.curve = edge.curve.reversed()?;
                    }
                    edge.vertices = edges[index].vertices;
                    edges[index] = edge;
                    for (old, new) in trim.vertices.into_iter().zip(candidate.vertices) {
                        vertices[old] = natural.vertices[new];
                    }
                    trim.curve = candidate.curve.clone();
                    trim.tolerance = [0.; 2];
                    continue;
                }
                let mut source = self.edges[index].curve.clone();
                if trim.reversed_3d {
                    source = source.reversed()?;
                }
                source = source.try_reparameterized(trim.curve.domain())?;
                let normalized_trim = NurbsCurve2::try_new_rational(
                    trim.curve.degree(),
                    trim.curve
                        .control_points()
                        .iter()
                        .map(|p| {
                            let xyz = p.point().to_array();
                            WeightedPoint2::try_new(
                                Point2::try_new(
                                    crate::remap_scalar(
                                        xyz[0],
                                        [*domains[0].start(), *domains[0].end()],
                                        [0., 1.],
                                    )?,
                                    crate::remap_scalar(
                                        xyz[1],
                                        [*domains[1].start(), *domains[1].end()],
                                        [0., 1.],
                                    )?,
                                )?,
                                p.weight(),
                            )
                        })
                        .collect::<Result<Vec<_>, GeometryError>>()?,
                    trim.curve.knots().to_vec(),
                )?;
                let references = [
                    normalized_trim.start_point()?.to_array(),
                    normalized_trim.end_point()?.to_array(),
                ];
                let mut ends = [parameters[trim.vertices[0]], parameters[trim.vertices[1]]];
                for i in 0..2 {
                    ends[i] = if let Some(pole) = poles.get(&trim.vertices[i]) {
                        pole.endpoint(references[i])?
                    } else {
                        let uv = nearest_lift(ends[i].to_array(), references[i], closed);
                        Point2::try_new(uv[0], uv[1])?
                    };
                }
                // Native transfer keeps a complete constant-U contour on a
                // closed V chart as a target isocurve. Its U coordinate comes
                // from the projected shared endpoint. Pointwise surface
                // closest points can vary in U and describe a different locus.
                let fixed_isocurve = (closed[1]
                    && trim.iso == SurfaceIso::InteriorUConstant
                    && normalized_trim.is_straight_segment()
                    && references[0][0] == references[1][0]
                    && matches!([references[0][1], references[1][1]], [0., 1.] | [1., 0.]))
                .then_some((0, ends[0].x()));
                let uv = projection_curve(
                    &normalized,
                    &source,
                    ends,
                    tolerance,
                    (closed.into_iter().any(|c| c) || !poles.is_empty()).then_some(
                        ProjectionChart {
                            source: &original_normalized,
                            curve: &normalized_trim,
                            closed,
                            fixed: if fixed_isocurve.is_some() {
                                fixed_isocurve
                            } else if trim.trim_type == BrepTrimType::Seam {
                                match trim.iso {
                                    SurfaceIso::West => Some((0, 0.)),
                                    SurfaceIso::East => Some((0, 1.)),
                                    SurfaceIso::South => Some((1, 0.)),
                                    SurfaceIso::North => Some((1, 1.)),
                                    _ => None,
                                }
                            } else {
                                None
                            },
                        },
                    ),
                )?;
                let controls = uv
                    .control_points()
                    .iter()
                    .map(|p| {
                        let coordinates = [p.point().x(), p.point().y()];
                        WeightedPoint2::try_new(
                            Point2::try_new(
                                crate::remap_scalar(
                                    coordinates[0],
                                    [0., 1.],
                                    [*domains[0].start(), *domains[0].end()],
                                )?,
                                crate::remap_scalar(
                                    coordinates[1],
                                    [0., 1.],
                                    [*domains[1].start(), *domains[1].end()],
                                )?,
                            )?,
                            p.weight(),
                        )
                    })
                    .collect::<Result<Vec<_>, GeometryError>>()?;
                trim.curve =
                    NurbsCurve2::try_new_rational(uv.degree(), controls, uv.knots().to_vec())?;
                trim.iso = trim_iso::classify(&trim.curve, &target);
                trim.tolerance = [0.; 2];
                let mut images = Vec::new();
                certified_images(&target, trim.curve.clone(), tolerance, 0, &mut images)?;
                if images.len() > 1 {
                    if original_uses[index] != 1 {
                        return invalid("a shared retrim edge cannot be split independently");
                    }
                    let mut pieces = Vec::new();
                    let mut start = trim.vertices[0];
                    let total = images.len();
                    for (piece_index, (uv, mut spatial)) in images.into_iter().enumerate() {
                        let end = if piece_index + 1 == total {
                            trim.vertices[1]
                        } else {
                            let p = uv.end_point()?;
                            let id = vertices.len();
                            vertices.push(BrepVertex::try_new(
                                target.evaluate(p.x(), p.y())?,
                                tolerance.absolute(),
                            )?);
                            id
                        };
                        if trim.reversed_3d {
                            spatial = spatial.reversed()?;
                        }
                        let endpoints = if trim.reversed_3d {
                            [end, start]
                        } else {
                            [start, end]
                        };
                        let edge = if piece_index == 0 { index } else { edges.len() };
                        let value = BrepEdge::try_new(endpoints, spatial, tolerance.absolute())?;
                        if edge == edges.len() {
                            edges.push(value);
                        } else {
                            edges[edge] = value;
                        }
                        let mut value = trim.clone();
                        value.vertices = [start, end];
                        value.edge = Some(edge);
                        value.curve = uv;
                        value.iso = trim_iso::classify(&value.curve, &target);
                        pieces.push(value);
                        start = end;
                    }
                    replacements.insert((loop_index, trim_index), pieces);
                    continue;
                }
                let mut spatial = images.pop().unwrap().1;
                if trim.reversed_3d {
                    spatial = spatial.reversed()?;
                }
                edges[index].curve = spatial;
                edges[index].tolerance = tolerance.absolute();
            }
        }
        for (loop_index, face_loop) in loops.iter_mut().enumerate() {
            let mut trims = Vec::new();
            for (trim_index, trim) in std::mem::take(&mut face_loop.trims).into_iter().enumerate() {
                if let Some(pieces) = replacements.remove(&(loop_index, trim_index)) {
                    trims.extend(pieces);
                } else {
                    trims.push(trim);
                }
            }
            face_loop.trims = trims;
        }
        Self::try_new(
            vertices,
            edges,
            vec![BrepFace::try_new(target, face.reversed, loops)?],
            tolerance,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn periodic_lifts_keep_distinct_seam_uses_and_half_chart_cuts() {
        assert_eq!(
            nearest_lift([0., 0.25], [1., 0.25], [true, false]),
            [1., 0.25]
        );
        assert_eq!(
            nearest_lift([1., 0.75], [0., 0.75], [true, false]),
            [0., 0.75]
        );
        assert_eq!(
            nearest_lift([0.98, 0.5], [0.99, 0.5], [true, false]),
            [0.98, 0.5]
        );
        assert_eq!(
            nearest_lift([0.02, 0.5], [1.01, 0.5], [true, false]),
            [1.02, 0.5]
        );
        assert_eq!(
            nearest_lift([0.02, 0.5], [1.01, 0.5], [false, false]),
            [0.02, 0.5]
        );
    }

    #[test]
    fn cylinder_band_transfer_preserves_a_shared_seam_and_full_circle_boundaries() {
        let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
        let frame = Frame3::try_from_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            tolerance,
        )
        .unwrap();
        let surface = NurbsSurface::try_cylinder(frame, 2., 0., 4.).unwrap();
        let original = Brep::try_rectangular_surface_face(
            surface.clone(),
            surface.domain_u(),
            1. ..=3.,
            tolerance,
        )
        .unwrap();
        let before = original.clone();
        let target = crate::try_rebuild_nurbs_surface(&surface, [12, 8], [3, 3]).unwrap();
        let result = original
            .try_retrimmed_single_surface(target, tolerance)
            .unwrap();
        assert_eq!(original, before);
        assert_eq!(result.vertices.len(), 2);
        assert_eq!(result.edges.len(), 3);
        let seams = result.faces[0].loops[0]
            .trims
            .iter()
            .filter(|t| t.trim_type == BrepTrimType::Seam)
            .collect::<Vec<_>>();
        assert_eq!(seams.len(), 2);
        assert_eq!(seams[0].edge, seams[1].edge);
        assert_ne!(
            seams[0].curve.start_point().unwrap().x(),
            seams[1].curve.end_point().unwrap().x()
        );
        assert!((result.area(tolerance).unwrap() - 8. * std::f64::consts::PI).abs() < 0.01);
    }

    #[test]
    fn spherical_cap_transfer_keeps_exact_poles_and_both_chart_branches() {
        let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
        let frame = Frame3::try_from_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            tolerance,
        )
        .unwrap();
        let sphere = NurbsSurface::try_sphere(frame, 2.).unwrap();
        for swapped in [false, true] {
            let surface = if swapped {
                sphere.try_swapped_uv().unwrap()
            } else {
                sphere.clone()
            };
            let u = surface.domain_u();
            let v = surface.domain_v();
            let original = Brep::try_rectangular_surface_face(
                surface.clone(),
                if swapped {
                    surface.parameter_at_u(0.7).unwrap()..=*u.end()
                } else {
                    u
                },
                if swapped {
                    v
                } else {
                    surface.parameter_at_v(0.7).unwrap()..=*v.end()
                },
                tolerance,
            )
            .unwrap()
            .reversed();
            let before = original.clone();
            let target = crate::try_rebuild_nurbs_surface(&surface, [12, 8], [3, 3]).unwrap();
            let result = original
                .try_retrimmed_single_surface(target, tolerance)
                .unwrap();
            assert_eq!(original, before);
            assert!(result.faces[0].reversed);
            assert_eq!(result.vertices.len(), result.edges.len());
            assert_eq!(result.faces[0].loops[0].trims.len(), result.edges.len() + 2);
            let seams = result.faces[0].loops[0]
                .trims
                .iter()
                .filter(|t| t.trim_type == BrepTrimType::Seam)
                .collect::<Vec<_>>();
            assert_eq!(seams.len(), 2);
            assert_eq!(seams[0].edge, seams[1].edge);
            let singular = result.faces[0].loops[0]
                .trims
                .iter()
                .find(|t| t.trim_type == BrepTrimType::Singular)
                .unwrap();
            assert!(singular.edge.is_none());
            assert_eq!(singular.vertices[0], singular.vertices[1]);
            assert_eq!(
                result.vertices[singular.vertices[0]].point,
                Point3::try_new(0., 0., 2.).unwrap()
            );
            assert_eq!(
                singular.curve,
                original.faces[0].loops[0]
                    .trims
                    .iter()
                    .find(|t| t.trim_type == BrepTrimType::Singular)
                    .unwrap()
                    .curve
            );
        }
    }

    #[test]
    fn singular_trim_transfer_rejects_a_target_without_the_exact_collapse() {
        let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
        let frame = Frame3::try_from_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            tolerance,
        )
        .unwrap();
        let sphere = NurbsSurface::try_sphere(frame, 2.).unwrap();
        let original = Brep::try_rectangular_surface_face(
            sphere.clone(),
            sphere.domain_u(),
            sphere.parameter_at_v(0.7).unwrap()..=*sphere.domain_v().end(),
            tolerance,
        )
        .unwrap();
        let before = original.clone();
        let target = crate::try_rebuild_nurbs_surface(&sphere, [12, 8], [3, 3]).unwrap();
        let mut controls = target.control_points().to_vec();
        let index = (target.control_point_count_v() - 1) * target.control_point_count_u() + 1;
        controls[index] =
            WeightedPoint3::try_new(Point3::try_new(0.01, 0., 2.).unwrap(), 1.).unwrap();
        let target = NurbsSurface::try_new_rational(
            target.degree_u(),
            target.degree_v(),
            target.control_point_count_u(),
            target.control_point_count_v(),
            controls,
            target.knots_u().to_vec(),
            target.knots_v().to_vec(),
        )
        .unwrap();
        assert!(
            original
                .try_retrimmed_single_surface(target, tolerance)
                .is_err()
        );
        assert_eq!(original, before);
    }
    #[test]
    fn projection_moves_boundaries_to_parallel_surface_and_preserves_holes_and_orientation() {
        let tolerance = Tolerance::DEFAULT;
        let center = Point3::try_new(0., 0., 0.).unwrap();
        let normal = UnitVector3::try_new(0., 0., 1., tolerance).unwrap();
        let outer = crate::Circle3::try_new(center, 5., normal, tolerance)
            .unwrap()
            .to_nurbs()
            .unwrap();
        let inner = crate::Circle3::try_new(center, 2., normal, tolerance)
            .unwrap()
            .to_nurbs()
            .unwrap();
        let original = Brep::try_planar_face_with_holes(&outer, &[inner], tolerance)
            .unwrap()
            .reversed();
        let before = original.clone();
        let target = original.faces[0]
            .surface
            .transformed(AffineTransform3::from_translation(
                Vector3::try_new(0., 0., 1.).unwrap(),
            ))
            .unwrap()
            .try_reparameterized(2. ..=8., -4. ..=10.)
            .unwrap();
        let result = original
            .try_retrimmed_single_surface(target, tolerance)
            .unwrap();
        assert_eq!(original, before);
        assert!(result.faces[0].reversed);
        assert_eq!(result.faces[0].loops.len(), 2);
        assert_eq!(
            result.faces[0].surface.domain_u(),
            original.faces[0].surface.domain_u()
        );
        assert_eq!(
            result.faces[0].surface.domain_v(),
            original.faces[0].surface.domain_v()
        );
        assert!((result.area(tolerance).unwrap() - 21. * std::f64::consts::PI).abs() < 1e-7);
        for vertex in &result.vertices {
            assert!((vertex.point.z() - 1.).abs() < 1e-12);
        }
        for trim in result.faces[0].loops.iter().flat_map(|l| &l.trims) {
            let mut edge = result.edges[trim.edge.unwrap()].curve.clone();
            if trim.reversed_3d {
                edge = edge.reversed().unwrap();
            }
            assert!(
                result.faces[0]
                    .surface
                    .parameter_curve_deviation_bound(&trim.curve, &edge, tolerance.absolute())
                    .unwrap()
                    .is_some()
            );
        }
    }
    #[test]
    fn natural_retrim_preserves_reordered_edges_reversed_uses_and_exact_isocurves() {
        let tolerance = Tolerance::DEFAULT;
        let surface = NurbsSurface::try_bilinear([
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(4., 0., 0.).unwrap(),
            Point3::try_new(4., 6., 2.).unwrap(),
            Point3::try_new(0., 6., 0.).unwrap(),
        ])
        .unwrap();
        let mut original = Brep::try_surface_face(surface, tolerance)
            .unwrap()
            .reordered_edges(&[2, 0, 3, 1], tolerance)
            .unwrap()
            .reversed();
        let edge = original.faces[0].loops[0].trims[1].edge.unwrap();
        original.edges[edge].curve = original.edges[edge].curve.reversed().unwrap();
        original.edges[edge].vertices.swap(0, 1);
        original.faces[0].loops[0].trims[1].reversed_3d = true;
        original =
            Brep::try_new(original.vertices, original.edges, original.faces, tolerance).unwrap();
        let before = original.clone();
        let target =
            crate::try_rebuild_nurbs_surface(&original.faces[0].surface, [10, 10], [3, 3]).unwrap();
        let result = original
            .try_retrimmed_single_surface(target, tolerance)
            .unwrap();
        assert_eq!(original, before);
        assert!(result.faces[0].reversed);
        for (a, b) in original.faces[0].loops[0]
            .trims
            .iter()
            .zip(&result.faces[0].loops[0].trims)
        {
            assert_eq!(a.vertices, b.vertices);
            assert_eq!(a.edge, b.edge);
            assert_eq!(a.reversed_3d, b.reversed_3d);
            assert_eq!(a.iso, b.iso);
            let edge = &result.edges[b.edge.unwrap()];
            assert_eq!(edge.vertices, original.edges[a.edge.unwrap()].vertices);
            let spatial = if b.reversed_3d {
                edge.curve.reversed().unwrap()
            } else {
                edge.curve.clone()
            };
            assert!(
                result.faces[0]
                    .surface
                    .parameter_curve_deviation_bound(&b.curve, &spatial, 1e-9)
                    .unwrap()
                    .is_some()
            );
        }
    }
}
