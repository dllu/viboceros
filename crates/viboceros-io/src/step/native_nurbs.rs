//! Native conversion of STEP NURBS shell geometry and face-local p-curves.
use super::{StepError, Table, native_planar, reported_trimmed_shell};
use monstertruck::core::cgmath64::{InnerSpace, Transform as _};
use monstertruck::meshing::prelude::{BoundedCurve, ParametricCurve, ParametricSurface};
use monstertruck::step::load::step_geometry::{
    Conic2D, Conic3D, Curve2D, Curve3D, ElementarySurface, StepParameterCurve,
    StepRevolutionSurface, Surface, SurfaceCurve3D, SurfaceCurveAssociatedGeometry, SweepSurface,
};
use num_rational::BigRational;
use num_traits::ToPrimitive;
use viboceros_geometry::{
    Brep, BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, BrepVertex,
    NurbsCurve, NurbsCurve2, NurbsSurface, Point2, Point3, SurfaceIso, Tolerance, WeightedPoint2,
    WeightedPoint3,
};

pub(super) fn convert_shell(
    table: &Table,
    id: u64,
    tolerance: Tolerance,
) -> Result<Brep, StepError> {
    match native_planar::convert_shell(table, id, tolerance) {
        Ok(planar) => return Ok(planar),
        Err(StepError::UnsupportedPlanarShell { .. }) => {}
        Err(error) => return Err(error),
    }
    let unsupported = |reason| StepError::UnsupportedNativeShell { shell: id, reason };
    let (shell, report) = reported_trimmed_shell(table, id)?;
    if report.total_lost() != 0 {
        return Err(unsupported("source shell has topology losses"));
    }
    let vertices = shell
        .vertices
        .iter()
        .map(|p| BrepVertex::try_new(Point3::try_new(p.x, p.y, p.z)?, tolerance.absolute()))
        .collect::<Result<Vec<_>, _>>()?;
    let edges = shell
        .edges
        .iter()
        .map(|edge| {
            Ok(BrepEdge::try_new(
                [edge.vertices.0, edge.vertices.1],
                edge_curve(&edge.curve, id)?,
                tolerance.absolute(),
            )?)
        })
        .collect::<Result<Vec<_>, StepError>>()?;
    let mut incidence = vec![0; edges.len()];
    let mut first_face = vec![None; edges.len()];
    let mut repeated_on_face = vec![false; edges.len()];
    for (face_index, face) in shell.faces.iter().enumerate() {
        for use_ in face.boundaries.iter().flatten() {
            incidence[use_.index] += 1;
            match first_face[use_.index] {
                Some(first) if first == face_index => repeated_on_face[use_.index] = true,
                None => first_face[use_.index] = Some(face_index),
                _ => {}
            }
        }
    }
    if incidence.iter().any(|&count| count > 2) {
        return Err(unsupported("non-manifold edge"));
    }
    let mut faces = Vec::with_capacity(shell.faces.len());
    for face in &shell.faces {
        if face.boundaries.is_empty() {
            return Err(unsupported("face has no boundary"));
        }
        let mut boundaries = Vec::with_capacity(face.boundaries.len());
        let periodic_axes = match &face.surface {
            Surface::ElementarySurface(
                ElementarySurface::CylindricalSurface(_) | ElementarySurface::ConicalSurface(_),
            ) => [true, false],
            Surface::ElementarySurface(ElementarySurface::ToroidalSurface(_)) => [true, true],
            Surface::ElementarySurface(ElementarySurface::Sphere(_)) => [true, false],
            Surface::SweepSurface(SweepSurface::RevolutionSurface(revolution)) => {
                if revolution.orientation() {
                    [false, true]
                } else {
                    [true, false]
                }
            }
            _ => [false, false],
        };
        for boundary in &face.boundaries {
            let mut trims = Vec::with_capacity(boundary.len());
            let mut previous_end = None;
            for use_ in boundary {
                let source = use_
                    .trim_curve
                    .as_ref()
                    .ok_or_else(|| unsupported("missing UV trim"))?;
                let mut curve = trim_curve(source.curve().as_ref(), id)?;
                if periodic_axes != [false; 2] {
                    if let Some(end) = previous_end {
                        curve = align_periodic_trim(curve, end, periodic_axes, tolerance)?;
                    }
                    previous_end = Some(curve.end_point()?);
                }
                let endpoints = shell.edges[use_.index].vertices;
                let vertices = if use_.orientation {
                    [endpoints.0, endpoints.1]
                } else {
                    [endpoints.1, endpoints.0]
                };
                trims.push(BrepTrim::try_new(
                    vertices,
                    Some(use_.index),
                    !use_.orientation,
                    curve,
                    if incidence[use_.index] == 2 && repeated_on_face[use_.index] {
                        BrepTrimType::Seam
                    } else if incidence[use_.index] == 2 {
                        BrepTrimType::Mated
                    } else {
                        BrepTrimType::Boundary
                    },
                    SurfaceIso::NotIso,
                    [0.0; 2],
                )?);
            }
            boundaries.push(trims);
        }
        let surface = surface(&face.surface, &boundaries, id, tolerance)?;
        let native = if boundaries.len() == 1 {
            BrepFace::try_new(
                surface,
                !face.orientation,
                vec![BrepLoop::try_new(
                    BrepLoopType::Outer,
                    boundaries.remove(0),
                )?],
            )?
        } else {
            BrepFace::try_from_certified_boundaries(surface, !face.orientation, boundaries)?
        };
        faces.push(native);
    }
    Ok(Brep::try_new(vertices, edges, faces, tolerance)?)
}

/// Face-local p-curves on a periodic surface may use different angular turns.
/// Translate by whole turns so adjacent trims meet without changing their shape.
fn align_periodic_trim(
    curve: NurbsCurve2,
    previous_end: Point2,
    periodic_axes: [bool; 2],
    tolerance: Tolerance,
) -> Result<NurbsCurve2, StepError> {
    let start = curve.start_point()?;
    let mut offset = [0.; 2];
    for (axis, delta) in [previous_end.x() - start.x(), previous_end.y() - start.y()]
        .into_iter()
        .enumerate()
    {
        if periodic_axes[axis] {
            offset[axis] = (delta / std::f64::consts::TAU).round() * std::f64::consts::TAU;
        }
        let threshold = if periodic_axes[axis] {
            tolerance.angular()
        } else {
            tolerance.absolute()
        };
        if !offset[axis].is_finite() || (delta - offset[axis]).abs() > threshold {
            return Ok(curve);
        }
    }
    if offset == [0.; 2] {
        return Ok(curve);
    }
    let controls = curve
        .control_points()
        .iter()
        .map(|control| {
            WeightedPoint2::try_new(
                Point2::try_new(
                    control.point().x() + offset[0],
                    control.point().y() + offset[1],
                )?,
                control.weight(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(NurbsCurve2::try_new_rational(
        curve.degree(),
        controls,
        curve.knots().to_vec(),
    )?)
}

fn point3(p: monstertruck::modeling::Point3) -> Result<Point3, StepError> {
    Ok(Point3::try_new(p.x, p.y, p.z)?)
}

fn weighted3(x: f64, y: f64, z: f64, w: f64, id: u64) -> Result<WeightedPoint3, StepError> {
    if !w.is_finite() || w == 0.0 {
        return Err(StepError::UnsupportedNativeShell {
            shell: id,
            reason: "rational control weight is zero or non-finite",
        });
    }
    Ok(WeightedPoint3::try_new(
        Point3::try_new(x / w, y / w, z / w)?,
        w,
    )?)
}

fn weighted2(x: f64, y: f64, w: f64, id: u64) -> Result<WeightedPoint2, StepError> {
    if !w.is_finite() || w == 0.0 {
        return Err(StepError::UnsupportedNativeShell {
            shell: id,
            reason: "rational UV control weight is zero or non-finite",
        });
    }
    Ok(WeightedPoint2::try_new(Point2::try_new(x / w, y / w)?, w)?)
}

fn edge_curve(curve: &Curve3D, id: u64) -> Result<NurbsCurve, StepError> {
    let unsupported = |reason| StepError::UnsupportedNativeShell { shell: id, reason };
    match curve {
        Curve3D::SurfaceCurve(surface_curve) => {
            if let Some(parameter_curve) = collapsed_surface_curve_pcurve(surface_curve) {
                edge_curve(&Curve3D::ParameterCurve(parameter_curve.clone()), id)
            } else {
                edge_curve(surface_curve.leader(), id)
            }
        }
        Curve3D::IntersectionCurve(intersection_curve) => {
            edge_curve(intersection_curve.leader(), id)
        }
        Curve3D::ParameterCurve(parameter_curve) => {
            let basis = parameter_curve.surface().as_ref();
            let uv = trim_curve(parameter_curve.curve().as_ref(), id)?;
            let globally_affine = matches!(
                basis,
                Surface::ElementarySurface(ElementarySurface::Plane(_))
            ) || matches!(
                basis,
                Surface::SweepSurface(SweepSurface::ExtrusionSurface(extrusion))
                    if matches!(extrusion.entity_curve(), Curve3D::Line(_))
            );
            let bounded_affine = affine_bilinear_basis(basis);
            if !globally_affine
                && bounded_affine.is_none()
                && uv.degree() == 1
                && uv.control_points().len() == 2
            {
                let uv0 = uv.control_points()[0].point();
                let uv1 = uv.control_points()[1].point();
                let weights = [
                    uv.control_points()[0].weight(),
                    uv.control_points()[1].weight(),
                ];
                let equal_weights = weights[0] == weights[1];
                if equal_weights && (uv0.y() == uv1.y() || uv0.x() == uv1.x()) {
                    if matches!(basis, Surface::NurbsSurface(_) | Surface::BsplineSurface(_)) {
                        let surface = spline_surface_basis(basis, id)?;
                        let (varying_start, varying_end, curve) = if uv0.y() == uv1.y() {
                            (uv0.x(), uv1.x(), surface.isocurve_u(uv0.y())?)
                        } else {
                            (uv0.y(), uv1.y(), surface.isocurve_v(uv0.x())?)
                        };
                        return oriented_isocurve_edge(
                            curve,
                            varying_start,
                            varying_end,
                            uv.domain(),
                            id,
                        );
                    }
                    if let Surface::SweepSurface(SweepSurface::ExtrusionSurface(extrusion)) = basis
                    {
                        let directrix = sweep_directrix(extrusion.entity_curve(), id)?;
                        if uv0.y() == uv1.y() {
                            let vector = extrusion.extruding_vector();
                            let controls = directrix
                                .control_points()
                                .iter()
                                .map(|control| {
                                    WeightedPoint3::try_new(
                                        Point3::try_new(
                                            control.point().x() + uv0.y() * vector.x,
                                            control.point().y() + uv0.y() * vector.y,
                                            control.point().z() + uv0.y() * vector.z,
                                        )?,
                                        control.weight(),
                                    )
                                    .map_err(StepError::from)
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            let curve = NurbsCurve::try_new_rational(
                                directrix.degree(),
                                controls,
                                directrix.knots().to_vec(),
                            )?;
                            return oriented_isocurve_edge(
                                curve,
                                uv0.x(),
                                uv1.x(),
                                uv.domain(),
                                id,
                            );
                        }
                        if !directrix.domain().contains(&uv0.x()) {
                            return Err(unsupported(
                                "3D edge p-curve leaves its extrusion surface domain",
                            ));
                        }
                        let base = directrix.evaluate(uv0.x())?;
                        let vector = extrusion.extruding_vector();
                        let [start, end] = [uv0.y(), uv1.y()].map(|v| {
                            Point3::try_new(
                                base.x() + v * vector.x,
                                base.y() + v * vector.y,
                                base.z() + v * vector.z,
                            )
                        });
                        let domain = uv.domain();
                        return Ok(NurbsCurve::try_new(
                            1,
                            vec![start?, end?],
                            vec![
                                *domain.start(),
                                *domain.start(),
                                *domain.end(),
                                *domain.end(),
                            ],
                        )?);
                    }
                    if let Surface::SweepSurface(SweepSurface::RevolutionSurface(revolution)) =
                        basis
                    {
                        if revolution.orientation() {
                            return Err(unsupported(
                                "STEP revolution parameters are not angle-first",
                            ));
                        }
                        let directrix = sweep_directrix(revolution.entity().entity_curve(), id)?;
                        if uv0.x() == uv1.x() {
                            let controls = directrix
                                .control_points()
                                .iter()
                                .map(|control| {
                                    Ok(WeightedPoint3::try_new(
                                        rotate_revolution_point(
                                            revolution,
                                            control.point(),
                                            uv0.x(),
                                        )?,
                                        control.weight(),
                                    )?)
                                })
                                .collect::<Result<Vec<_>, StepError>>()?;
                            let curve = NurbsCurve::try_new_rational(
                                directrix.degree(),
                                controls,
                                directrix.knots().to_vec(),
                            )?;
                            return oriented_isocurve_edge(
                                curve,
                                uv0.y(),
                                uv1.y(),
                                uv.domain(),
                                id,
                            );
                        }
                        if !directrix.domain().contains(&uv0.y()) {
                            return Err(unsupported(
                                "3D edge p-curve leaves its revolution surface domain",
                            ));
                        }
                        let base = directrix.evaluate(uv0.y())?;
                        return circular_isocurve_edge(
                            |angle| rotate_revolution_point(revolution, base, angle),
                            uv0.x(),
                            uv1.x(),
                            uv.domain(),
                            id,
                        );
                    }
                    if matches!(
                        basis,
                        Surface::ElementarySurface(
                            ElementarySurface::CylindricalSurface(_)
                                | ElementarySurface::ConicalSurface(_)
                                | ElementarySurface::ToroidalSurface(_)
                                | ElementarySurface::Sphere(_)
                        )
                    ) {
                        return elementary_isocurve_edge(basis, uv0, uv1, uv.domain(), id);
                    }
                }
                if equal_weights
                    && let Some(curve) = bilinear_pcurve_edge(basis, uv0, uv1, uv.domain(), id)?
                {
                    return Ok(curve);
                }
                if let Some(curve) =
                    bezier_patch_pcurve_edge(basis, uv0, uv1, weights, uv.domain(), id)?
                {
                    return Ok(curve);
                }
                if let Some(curve) =
                    multispan_pcurve_edge(basis, uv0, uv1, weights, uv.domain(), id)?
                {
                    return Ok(curve);
                }
            }
            if !globally_affine && bounded_affine.is_none() {
                return Err(unsupported("3D edge p-curve basis is not affine"));
            }
            if let Some(([u0, u1], [v0, v1])) = bounded_affine {
                let sign = uv.control_points()[0].weight().is_sign_positive();
                if uv.control_points().iter().any(|control| {
                    let point = control.point();
                    point.x() < u0
                        || point.x() > u1
                        || point.y() < v0
                        || point.y() > v1
                        || control.weight().is_sign_positive() != sign
                }) {
                    return Err(unsupported(
                        "3D edge p-curve leaves its affine surface domain",
                    ));
                }
            }
            let controls = uv
                .control_points()
                .iter()
                .map(|control| {
                    let point = control.point();
                    WeightedPoint3::try_new(
                        point3(basis.evaluate(point.x(), point.y()))?,
                        control.weight(),
                    )
                    .map_err(StepError::from)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(NurbsCurve::try_new_rational(
                uv.degree(),
                controls,
                uv.knots().to_vec(),
            )?)
        }
        Curve3D::Polyline(curve) if curve.len() >= 2 => Ok(NurbsCurve::try_new(
            1,
            curve
                .iter()
                .copied()
                .map(point3)
                .collect::<Result<Vec<_>, _>>()?,
            polyline_knots(curve.len()),
        )?),
        Curve3D::Line(_) | Curve3D::Polyline(_) => native_planar::curves::linear_edge(curve, id)
            .map_err(|error| match error {
                StepError::UnsupportedPlanarShell { reason, .. } => unsupported(reason),
                other => other,
            }),
        Curve3D::Conic(conic) => conic_edge(conic, id),
        Curve3D::BsplineCurve(curve) => Ok(NurbsCurve::try_new(
            curve.degree(),
            curve
                .control_points()
                .iter()
                .copied()
                .map(point3)
                .collect::<Result<Vec<_>, _>>()?,
            curve.knot_vector().iter().copied().collect(),
        )?),
        Curve3D::NurbsCurve(curve) => Ok(NurbsCurve::try_new_rational(
            curve.degree(),
            curve
                .control_points()
                .iter()
                .map(|p| weighted3(p.x, p.y, p.z, p.w, id))
                .collect::<Result<Vec<_>, _>>()?,
            curve.knot_vector().iter().copied().collect(),
        )?),
    }
}

/// Some STEP loaders synthesize a point-like 3D leader for a full-turn edge
/// whose vertices coincide, while preserving its actual p-curve as associated
/// geometry. Accept that p-curve only when it is unique and closes at the
/// leader's point on the same surface.
fn collapsed_surface_curve_pcurve(surface_curve: &SurfaceCurve3D) -> Option<&StepParameterCurve> {
    let Curve3D::ParameterCurve(leader) = surface_curve.leader() else {
        return None;
    };
    let Curve2D::Line(line) = leader.curve().as_ref() else {
        return None;
    };
    if line.0 != line.1 {
        return None;
    }
    let surface = leader.surface().as_ref();
    let leader_point = surface.evaluate(line.0.x, line.0.y);
    let mut candidates = surface_curve
        .associated_geometry()
        .iter()
        .filter_map(|entry| {
            let SurfaceCurveAssociatedGeometry::ParameterCurve(candidate) = entry else {
                return None;
            };
            if candidate.surface().as_ref() != surface {
                return None;
            }
            let (start, end) = candidate.curve().range_tuple();
            let endpoints = [
                candidate.curve().evaluate(start),
                candidate.curve().evaluate(end),
            ];
            endpoints
                .into_iter()
                .all(|uv| {
                    let point = surface.evaluate(uv.x, uv.y);
                    [
                        (point.x, leader_point.x),
                        (point.y, leader_point.y),
                        (point.z, leader_point.z),
                    ]
                    .into_iter()
                    .all(|(a, b)| (a - b).abs() <= 1e-10 * a.abs().max(b.abs()).max(1.))
                })
                .then_some(candidate)
        });
    let candidate = candidates.next()?;
    candidates.next().is_none().then_some(candidate)
}

fn oriented_isocurve_edge(
    mut curve: NurbsCurve,
    varying_start: f64,
    varying_end: f64,
    domain: std::ops::RangeInclusive<f64>,
    id: u64,
) -> Result<NurbsCurve, StepError> {
    if varying_start == varying_end {
        return Err(StepError::UnsupportedNativeShell {
            shell: id,
            reason: "3D edge p-curve is degenerate",
        });
    }
    curve = curve.try_trimmed(varying_start.min(varying_end)..=varying_start.max(varying_end))?;
    if varying_start > varying_end {
        curve = curve.reversed()?;
    }
    Ok(curve.try_reparameterized(domain)?)
}

fn circular_isocurve_edge(
    evaluate: impl Fn(f64) -> Result<Point3, StepError>,
    from: f64,
    to: f64,
    domain: std::ops::RangeInclusive<f64>,
    id: u64,
) -> Result<NurbsCurve, StepError> {
    if from == to {
        return Err(StepError::UnsupportedNativeShell {
            shell: id,
            reason: "3D edge p-curve is degenerate",
        });
    }
    let start = from.min(to);
    let end = from.max(to);
    let spans = arc_span_count(end - start, id)?;
    let step = (end - start) / spans as f64;
    let mut controls = Vec::with_capacity(2 * spans + 1);
    for index in 0..spans {
        let a = start + step * index as f64;
        let b = if index + 1 == spans { end } else { a + step };
        let p0 = evaluate(a)?;
        let pm = evaluate((a + b) / 2.)?;
        let p1 = evaluate(b)?;
        let weight = ((b - a) / 2.).cos();
        if index == 0 {
            controls.push(WeightedPoint3::try_new(p0, 1.)?);
        }
        controls.push(WeightedPoint3::try_new(
            circular_middle(p0, pm, p1, weight)?,
            weight,
        )?);
        controls.push(WeightedPoint3::try_new(p1, 1.)?);
    }
    let curve = NurbsCurve::try_new_rational(2, controls, arc_knots(start, end, spans))?;
    oriented_isocurve_edge(curve, from, to, domain, id)
}

fn elementary_isocurve_edge(
    basis: &Surface,
    uv0: Point2,
    uv1: Point2,
    domain: std::ops::RangeInclusive<f64>,
    id: u64,
) -> Result<NurbsCurve, StepError> {
    let varying_u = uv0.y() == uv1.y();
    let (from, to) = if varying_u {
        (uv0.x(), uv1.x())
    } else {
        (uv0.y(), uv1.y())
    };
    if from == to {
        return Err(StepError::UnsupportedNativeShell {
            shell: id,
            reason: "3D edge p-curve is degenerate",
        });
    }
    if varying_u
        && matches!(
            basis,
            Surface::ElementarySurface(ElementarySurface::Sphere(_))
        )
        && (uv0.y().abs() - std::f64::consts::FRAC_PI_2).abs() <= 1e-12
    {
        return Err(StepError::UnsupportedNativeShell {
            shell: id,
            reason: "sphere pole requires singular trim support",
        });
    }
    let evaluate = |parameter| {
        if varying_u {
            point3(basis.evaluate(parameter, uv0.y()))
        } else {
            point3(basis.evaluate(uv0.x(), parameter))
        }
    };
    if !varying_u
        && matches!(
            basis,
            Surface::ElementarySurface(
                ElementarySurface::CylindricalSurface(_) | ElementarySurface::ConicalSurface(_)
            )
        )
    {
        return Ok(NurbsCurve::try_new(
            1,
            vec![evaluate(from)?, evaluate(to)?],
            vec![
                *domain.start(),
                *domain.start(),
                *domain.end(),
                *domain.end(),
            ],
        )?);
    }
    circular_isocurve_edge(evaluate, from, to, domain, id)
}

/// Composes a straight UV segment with one degree-one tensor-product patch.
/// Each homogeneous surface basis function is a product of two linear
/// Bernstein polynomials along the segment, so its image is a rational
/// quadratic Bézier curve even when the patch is warped or has varying weights.
fn bilinear_pcurve_edge(
    basis: &Surface,
    uv0: Point2,
    uv1: Point2,
    domain: std::ops::RangeInclusive<f64>,
    id: u64,
) -> Result<Option<NurbsCurve>, StepError> {
    let is_bilinear = match basis {
        Surface::BsplineSurface(surface) => {
            surface.udegree() == 1
                && surface.vdegree() == 1
                && surface.control_points().len() == 2
                && surface.control_points().iter().all(|row| row.len() == 2)
        }
        Surface::NurbsSurface(surface) => {
            surface.udegree() == 1
                && surface.vdegree() == 1
                && surface.control_points().len() == 2
                && surface.control_points().iter().all(|row| row.len() == 2)
        }
        _ => false,
    };
    if !is_bilinear {
        return Ok(None);
    }
    let unsupported = |reason| StepError::UnsupportedNativeShell { shell: id, reason };
    let surface = spline_surface_basis(basis, id)?;
    let u_domain = surface.domain_u();
    let v_domain = surface.domain_v();
    let inside = |point: Point2| u_domain.contains(&point.x()) && v_domain.contains(&point.y());
    if !inside(uv0) || !inside(uv1) {
        return Err(unsupported(
            "3D edge p-curve leaves its bilinear surface domain",
        ));
    }
    let sign = surface
        .control_point(0, 0)
        .unwrap()
        .weight()
        .is_sign_positive();
    if surface
        .control_points()
        .iter()
        .any(|control| control.weight().is_sign_positive() != sign)
    {
        return Err(unsupported("bilinear p-curve surface weights change sign"));
    }
    let u0 = (uv0.x() - u_domain.start()) / (u_domain.end() - u_domain.start());
    let u1 = (uv1.x() - u_domain.start()) / (u_domain.end() - u_domain.start());
    let v0 = (uv0.y() - v_domain.start()) / (v_domain.end() - v_domain.start());
    let v1 = (uv1.y() - v_domain.start()) / (v_domain.end() - v_domain.start());
    let u_basis = [[1. - u0, 1. - u1], [u0, u1]];
    let v_basis = [[1. - v0, 1. - v1], [v0, v1]];
    let mut homogeneous = [[0.; 4]; 3];
    for (u, u_factors) in u_basis.iter().enumerate() {
        for (v, v_factors) in v_basis.iter().enumerate() {
            let control = surface.control_point(u, v).unwrap();
            let weight = control.weight();
            let point = control.point();
            let source = [
                point.x() * weight,
                point.y() * weight,
                point.z() * weight,
                weight,
            ];
            let factors = [
                u_factors[0] * v_factors[0],
                (u_factors[0] * v_factors[1] + u_factors[1] * v_factors[0]) / 2.,
                u_factors[1] * v_factors[1],
            ];
            for (target, factor) in homogeneous.iter_mut().zip(factors) {
                for (coordinate, value) in target.iter_mut().zip(source) {
                    *coordinate += factor * value;
                }
            }
        }
    }
    let controls = homogeneous
        .into_iter()
        .map(|point| weighted3(point[0], point[1], point[2], point[3], id))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(NurbsCurve::try_new_rational(
        2,
        controls,
        vec![
            *domain.start(),
            *domain.start(),
            *domain.start(),
            *domain.end(),
            *domain.end(),
            *domain.end(),
        ],
    )?))
}

/// A straight path across a clamped single-span tensor-product surface is a
/// rational Bézier curve. Restrict the homogeneous patch to the UV rectangle
/// traversed by the path, then multiply its two Bernstein bases along the
/// rectangle's diagonal. Unequal UV control weights apply the same Möbius
/// reparameterization to both UV coordinates and elevate their endpoint
/// weight ratio to the composed curve degree.
fn bezier_patch_pcurve_edge(
    basis: &Surface,
    uv0: Point2,
    uv1: Point2,
    uv_weights: [f64; 2],
    domain: std::ops::RangeInclusive<f64>,
    id: u64,
) -> Result<Option<NurbsCurve>, StepError> {
    let (degree_u, degree_v, u_count, v_count) = match basis {
        Surface::BsplineSurface(surface) => (
            surface.udegree(),
            surface.vdegree(),
            surface.control_points().len(),
            surface.control_points().first().map_or(0, Vec::len),
        ),
        Surface::NurbsSurface(surface) => (
            surface.udegree(),
            surface.vdegree(),
            surface.control_points().len(),
            surface.control_points().first().map_or(0, Vec::len),
        ),
        _ => return Ok(None),
    };
    if degree_u.checked_add(1) != Some(u_count) || degree_v.checked_add(1) != Some(v_count) {
        return Ok(None);
    }
    let surface = spline_surface_basis(basis, id)?;
    compose_bezier_patch(&surface, uv0, uv1, uv_weights, domain, id)
}

fn compose_bezier_patch(
    surface: &NurbsSurface,
    uv0: Point2,
    uv1: Point2,
    uv_weights: [f64; 2],
    domain: std::ops::RangeInclusive<f64>,
    id: u64,
) -> Result<Option<NurbsCurve>, StepError> {
    let unsupported = |reason| StepError::UnsupportedNativeShell { shell: id, reason };
    let degree_u = surface.degree_u();
    let degree_v = surface.degree_v();
    let degree = degree_u
        .checked_add(degree_v)
        .ok_or_else(|| unsupported("Bezier p-curve composition degree overflows"))?;
    if degree > 64 {
        return Err(unsupported("Bezier p-curve composition degree exceeds 64"));
    }
    let u_domain = surface.domain_u();
    let v_domain = surface.domain_v();
    let clamped = |knots: &[f64], degree: usize, start: f64, end: f64| {
        knots.len() == 2 * (degree + 1)
            && knots[..=degree].iter().all(|knot| *knot == start)
            && knots[degree + 1..].iter().all(|knot| *knot == end)
    };
    if !clamped(
        surface.knots_u(),
        degree_u,
        *u_domain.start(),
        *u_domain.end(),
    ) || !clamped(
        surface.knots_v(),
        degree_v,
        *v_domain.start(),
        *v_domain.end(),
    ) {
        return Ok(None);
    }
    let inside = |point: Point2| u_domain.contains(&point.x()) && v_domain.contains(&point.y());
    if !inside(uv0) || !inside(uv1) {
        return Err(unsupported(
            "3D edge p-curve leaves its Bezier surface domain",
        ));
    }
    let sign = surface
        .control_point(0, 0)
        .unwrap()
        .weight()
        .is_sign_positive();
    if surface
        .control_points()
        .iter()
        .any(|control| control.weight().is_sign_positive() != sign)
    {
        return Err(unsupported("Bezier p-curve surface weights change sign"));
    }
    let u0 = (uv0.x() - u_domain.start()) / (u_domain.end() - u_domain.start());
    let u1 = (uv1.x() - u_domain.start()) / (u_domain.end() - u_domain.start());
    let v0 = (uv0.y() - v_domain.start()) / (v_domain.end() - v_domain.start());
    let v1 = (uv1.y() - v_domain.start()) / (v_domain.end() - v_domain.start());
    let u_count = surface.control_point_count_u();
    let v_count = surface.control_point_count_v();
    let mut u_restricted = Vec::with_capacity(v_count);
    for v in 0..v_count {
        let row = (0..u_count)
            .map(|u| homogeneous_surface_control(surface.control_point(u, v).unwrap()))
            .collect::<Vec<_>>();
        u_restricted.push(restrict_bezier_homogeneous(&row, u0, u1));
    }
    let mut restricted = vec![vec![[0.; 4]; u_count]; v_count];
    for u in 0..u_count {
        let column = (0..v_count).map(|v| u_restricted[v][u]).collect::<Vec<_>>();
        for (v, point) in restrict_bezier_homogeneous(&column, v0, v1)
            .into_iter()
            .enumerate()
        {
            restricted[v][u] = point;
        }
    }
    let choose_u = (0..=degree_u)
        .map(|index| binomial(degree_u, index))
        .collect::<Vec<_>>();
    let choose_v = (0..=degree_v)
        .map(|index| binomial(degree_v, index))
        .collect::<Vec<_>>();
    let choose_curve = (0..=degree)
        .map(|index| binomial(degree, index))
        .collect::<Vec<_>>();
    let mut controls = vec![[0.; 4]; degree + 1];
    for (v, row) in restricted.iter().enumerate() {
        for (u, point) in row.iter().enumerate() {
            let index = u + v;
            let factor = choose_u[u] * choose_v[v] / choose_curve[index];
            for (target, value) in controls[index].iter_mut().zip(point) {
                *target += factor * value;
            }
        }
    }
    let weight_scale = uv_weights[0].abs().max(uv_weights[1].abs());
    if uv_weights[0].is_sign_positive() != uv_weights[1].is_sign_positive()
        || !weight_scale.is_finite()
        || weight_scale == 0.
    {
        return Err(unsupported("rational p-curve weights change sign"));
    }
    let normalized = [
        uv_weights[0].abs() / weight_scale,
        uv_weights[1].abs() / weight_scale,
    ];
    for (index, point) in controls.iter_mut().enumerate() {
        let factor = normalized[0].powi((degree - index) as i32) * normalized[1].powi(index as i32);
        if !factor.is_finite() || factor == 0. {
            return Err(unsupported("rational p-curve weights cannot be composed"));
        }
        for coordinate in point {
            *coordinate *= factor;
        }
    }
    let controls = controls
        .into_iter()
        .map(|point| weighted3(point[0], point[1], point[2], point[3], id))
        .collect::<Result<Vec<_>, _>>()?;
    let mut knots = vec![*domain.start(); degree + 1];
    knots.extend(vec![*domain.end(); degree + 1]);
    Ok(Some(NurbsCurve::try_new_rational(degree, controls, knots)?))
}

fn homogeneous_surface_control(control: WeightedPoint3) -> [f64; 4] {
    let point = control.point();
    let weight = control.weight();
    [
        point.x() * weight,
        point.y() * weight,
        point.z() * weight,
        weight,
    ]
}

fn restrict_bezier_homogeneous(controls: &[[f64; 4]], from: f64, to: f64) -> Vec<[f64; 4]> {
    let start = from.min(to);
    let end = from.max(to);
    let (_, right) = split_bezier_homogeneous(controls, start);
    let (mut segment, _) = split_bezier_homogeneous(&right, (end - start) / (1. - start));
    if from > to {
        segment.reverse();
    }
    segment
}

fn split_bezier_homogeneous(
    controls: &[[f64; 4]],
    parameter: f64,
) -> (Vec<[f64; 4]>, Vec<[f64; 4]>) {
    let degree = controls.len() - 1;
    let mut stage = controls.to_vec();
    let mut left = Vec::with_capacity(controls.len());
    let mut right = vec![[0.; 4]; controls.len()];
    for depth in 0..=degree {
        left.push(stage[0]);
        right[degree - depth] = stage[degree - depth];
        for index in 0..degree - depth {
            let next = stage[index + 1];
            for (coordinate, following) in stage[index].iter_mut().zip(next) {
                *coordinate = *coordinate * (1. - parameter) + following * parameter;
            }
        }
    }
    (left, right)
}

fn binomial(n: usize, k: usize) -> f64 {
    let k = k.min(n - k);
    (1..=k).fold(1., |value, index| {
        value * (n - k + index) as f64 / index as f64
    })
}

/// Split a straight UV path wherever it crosses a surface knot. Exact
/// rational fractions keep simultaneous U/V crossings together and map each
/// segment endpoint onto the source knot without a floating-point gap. For
/// unequal UV weights, convert each geometric crossing fraction to its source
/// p-curve parameter before assigning the joined curve knot.
fn multispan_pcurve_edge(
    basis: &Surface,
    uv0: Point2,
    uv1: Point2,
    uv_weights: [f64; 2],
    domain: std::ops::RangeInclusive<f64>,
    id: u64,
) -> Result<Option<NurbsCurve>, StepError> {
    if !matches!(basis, Surface::NurbsSurface(_) | Surface::BsplineSurface(_)) {
        return Ok(None);
    }
    let unsupported = |reason| StepError::UnsupportedNativeShell { shell: id, reason };
    let surface = spline_surface_basis(basis, id)?.try_clamped_to_active_domain()?;
    let u_domain = surface.domain_u();
    let v_domain = surface.domain_v();
    if !u_domain.contains(&uv0.x())
        || !u_domain.contains(&uv1.x())
        || !v_domain.contains(&uv0.y())
        || !v_domain.contains(&uv1.y())
    {
        return Err(unsupported(
            "3D edge p-curve leaves its spline surface domain",
        ));
    }
    let exact = |value: f64| BigRational::from_float(value).unwrap();
    let zero = exact(0.);
    let one = exact(1.);
    let u0 = exact(uv0.x());
    let u1 = exact(uv1.x());
    let v0 = exact(uv0.y());
    let v1 = exact(uv1.y());
    let w0 = exact(uv_weights[0]);
    let w1 = exact(uv_weights[1]);
    if w0 == zero
        || w1 == zero
        || uv_weights[0].is_sign_positive() != uv_weights[1].is_sign_positive()
    {
        return Err(unsupported("rational p-curve weights change sign"));
    }
    let t_start = exact(*domain.start());
    let t_end = exact(*domain.end());
    let mut fractions = vec![zero.clone(), one.clone()];
    for (knots, from, to) in [(surface.knots_u(), &u0, &u1), (surface.knots_v(), &v0, &v1)] {
        let low = from.min(to);
        let high = from.max(to);
        for knot in knots.iter().copied() {
            let knot = exact(knot);
            if knot > *low && knot < *high {
                let fraction = (&knot - from) / (to - from);
                if fraction > zero && fraction < one {
                    fractions.push(fraction);
                }
            }
        }
    }
    fractions.sort();
    fractions.dedup();
    let value_at =
        |from: &BigRational, to: &BigRational, fraction: &BigRational| -> Result<f64, StepError> {
            (from + (to - from) * fraction)
                .to_f64()
                .ok_or_else(|| unsupported("p-curve knot crossing is not representable"))
        };
    let source_position = |fraction: &BigRational| {
        let denominator = &w1 * (&one - fraction) + &w0 * fraction;
        let parameter = &w0 * fraction / &denominator;
        let weight = &w0 * &w1 / denominator;
        Ok::<_, StepError>((
            value_at(&t_start, &t_end, &parameter)?,
            weight
                .to_f64()
                .ok_or_else(|| unsupported("rational p-curve weight is not representable"))?,
        ))
    };
    let mut spans = Vec::with_capacity(fractions.len() - 1);
    for pair in fractions.windows(2) {
        let a = Point2::try_new(value_at(&u0, &u1, &pair[0])?, value_at(&v0, &v1, &pair[0])?)?;
        let b = Point2::try_new(value_at(&u0, &u1, &pair[1])?, value_at(&v0, &v1, &pair[1])?)?;
        let (start, start_weight) = source_position(&pair[0])?;
        let (end, end_weight) = source_position(&pair[1])?;
        if start >= end || a.x() == b.x() || a.y() == b.y() {
            return Err(unsupported(
                "p-curve knot crossings are too close to compose",
            ));
        }
        let patch = surface.try_trimmed(
            a.x().min(b.x())..=a.x().max(b.x()),
            a.y().min(b.y())..=a.y().max(b.y()),
        )?;
        let curve =
            compose_bezier_patch(&patch, a, b, [start_weight, end_weight], start..=end, id)?
                .ok_or_else(|| unsupported("p-curve knot rectangle is not a Bezier patch"))?;
        spans.push(curve);
    }
    let degree = spans[0].degree();
    let mut controls = spans[0].control_points().to_vec();
    let mut knots = vec![*domain.start(); degree + 1];
    for span in spans.iter().skip(1) {
        let previous = *controls.last().unwrap();
        let first = span.control_points()[0];
        let distance = previous.point().distance_to(first.point())?;
        let scale = [previous.point(), first.point()]
            .into_iter()
            .flat_map(|point| [point.x().abs(), point.y().abs(), point.z().abs()])
            .fold(1_f64, f64::max);
        if distance > 1e-9 * scale {
            return Err(unsupported("composed p-curve spans do not meet"));
        }
        let factor = previous.weight() / first.weight();
        if !factor.is_finite() || factor == 0. {
            return Err(unsupported("composed p-curve weights cannot be joined"));
        }
        let knot = *span.domain().start();
        knots.extend(vec![knot; degree]);
        for control in span.control_points().iter().skip(1) {
            controls.push(WeightedPoint3::try_new(
                control.point(),
                control.weight() * factor,
            )?);
        }
    }
    knots.extend(vec![*domain.end(); degree + 1]);
    Ok(Some(NurbsCurve::try_new_rational(degree, controls, knots)?))
}

fn rotate_revolution_point(
    revolution: &StepRevolutionSurface,
    point: Point3,
    angle: f64,
) -> Result<Point3, StepError> {
    let point = monstertruck::modeling::Point3::new(point.x(), point.y(), point.z());
    let origin = revolution.entity().origin();
    let axis = revolution.entity().axis();
    let displacement = point - origin;
    let rotated = origin
        + displacement * angle.cos()
        + axis.cross(displacement) * angle.sin()
        + axis * axis.dot(displacement) * (1. - angle.cos());
    point3(revolution.transform().transform_point(rotated))
}

/// A single bilinear patch is affine precisely when its homogeneous controls
/// have one weight and form a parallelogram. Compare the stored binary64 values
/// as exact rationals so rounding in a cross sum cannot certify a warped patch.
fn affine_bilinear_basis(basis: &Surface) -> Option<([f64; 2], [f64; 2])> {
    let (controls, ku, kv) = match basis {
        Surface::BsplineSurface(surface)
            if surface.udegree() == 1
                && surface.vdegree() == 1
                && surface.control_points().len() == 2
                && surface.control_points().iter().all(|row| row.len() == 2) =>
        {
            let rows = surface.control_points();
            let p = |u: usize, v: usize| {
                let p = rows[u][v];
                [p.x, p.y, p.z, 1.]
            };
            (
                [p(0, 0), p(1, 0), p(0, 1), p(1, 1)],
                surface.knot_vector_u(),
                surface.knot_vector_v(),
            )
        }
        Surface::NurbsSurface(surface)
            if surface.udegree() == 1
                && surface.vdegree() == 1
                && surface.control_points().len() == 2
                && surface.control_points().iter().all(|row| row.len() == 2) =>
        {
            let rows = surface.control_points();
            let p = |u: usize, v: usize| {
                let p = rows[u][v];
                [p.x, p.y, p.z, p.w]
            };
            (
                [p(0, 0), p(1, 0), p(0, 1), p(1, 1)],
                surface.knot_vector_u(),
                surface.knot_vector_v(),
            )
        }
        _ => return None,
    };
    if ku.len() != 4
        || kv.len() != 4
        || ku[0] != ku[1]
        || ku[1] >= ku[2]
        || ku[2] != ku[3]
        || kv[0] != kv[1]
        || kv[1] >= kv[2]
        || kv[2] != kv[3]
        || !ku.iter().chain(kv.iter()).all(|knot| knot.is_finite())
        || !controls.iter().flatten().all(|value| value.is_finite())
        || controls[0][3] == 0.
        || controls.iter().any(|point| point[3] != controls[0][3])
    {
        return None;
    }
    if !(0..3).all(|axis| {
        let value = |index: usize| BigRational::from_float(controls[index][axis]).unwrap();
        value(0) + value(3) == value(1) + value(2)
    }) {
        return None;
    }
    Some(([ku[1], ku[2]], [kv[1], kv[2]]))
}

fn sweep_directrix(curve: &Curve3D, id: u64) -> Result<NurbsCurve, StepError> {
    match curve {
        Curve3D::SurfaceCurve(curve) => sweep_directrix(curve.leader(), id),
        Curve3D::IntersectionCurve(curve) => sweep_directrix(curve.leader(), id),
        Curve3D::Line(_)
        | Curve3D::Polyline(_)
        | Curve3D::BsplineCurve(_)
        | Curve3D::NurbsCurve(_) => edge_curve(curve, id),
        Curve3D::ParameterCurve(parameter_curve) => {
            let (start, end) = parameter_curve.range_tuple();
            Ok(edge_curve(curve, id)?.try_reparameterized(start..=end)?)
        }
        Curve3D::Conic(conic) => {
            let (start, end) = conic.range_tuple();
            Ok(conic_edge(conic, id)?.try_reparameterized(start..=end)?)
        }
    }
}

fn trim_curve(curve: &Curve2D, id: u64) -> Result<NurbsCurve2, StepError> {
    let unsupported = |reason| StepError::UnsupportedNativeShell { shell: id, reason };
    match curve {
        Curve2D::Polyline(curve) if curve.len() >= 2 => Ok(NurbsCurve2::try_new(
            1,
            curve
                .iter()
                .map(|p| Point2::try_new(p.x, p.y))
                .collect::<Result<Vec<_>, _>>()?,
            polyline_knots(curve.len()),
        )?),
        Curve2D::Line(_) | Curve2D::Polyline(_) => native_planar::curves::linear_trim(curve, id)
            .map_err(|error| match error {
                StepError::UnsupportedPlanarShell { reason, .. } => unsupported(reason),
                other => other,
            }),
        Curve2D::Conic(conic) => conic_trim(conic, id),
        Curve2D::BsplineCurve(curve) => Ok(NurbsCurve2::try_new(
            curve.degree(),
            curve
                .control_points()
                .iter()
                .map(|p| Point2::try_new(p.x, p.y))
                .collect::<Result<Vec<_>, _>>()?,
            curve.knot_vector().iter().copied().collect(),
        )?),
        Curve2D::NurbsCurve(curve) => Ok(NurbsCurve2::try_new_rational(
            curve.degree(),
            curve
                .control_points()
                .iter()
                .map(|p| weighted2(p.x, p.y, p.z, id))
                .collect::<Result<Vec<_>, _>>()?,
            curve.knot_vector().iter().copied().collect(),
        )?),
    }
}

fn polyline_knots(points: usize) -> Vec<f64> {
    let mut knots = Vec::with_capacity(points + 2);
    knots.push(0.);
    knots.extend((0..points).map(|index| index as f64));
    knots.push((points - 1) as f64);
    knots
}

fn arc_span_count(angle: f64, id: u64) -> Result<usize, StepError> {
    if !angle.is_finite() || angle <= 0. || angle > std::f64::consts::TAU + 1e-10 {
        return Err(StepError::UnsupportedNativeShell {
            shell: id,
            reason: "conic arc must span at most one turn",
        });
    }
    Ok(((angle / std::f64::consts::FRAC_PI_2 - 1e-12).ceil() as usize).max(1))
}

fn arc_knots(start: f64, end: f64, spans: usize) -> Vec<f64> {
    let mut knots = vec![start; 3];
    for index in 1..spans {
        knots.extend([start + (end - start) * index as f64 / spans as f64; 2]);
    }
    knots.extend([end; 3]);
    knots
}

fn circular_middle(p0: Point3, pm: Point3, p1: Point3, weight: f64) -> Result<Point3, StepError> {
    let factor = 2. * (1. + weight);
    let denominator = 2. * weight;
    Ok(Point3::try_new(
        (factor * pm.x() - p0.x() - p1.x()) / denominator,
        (factor * pm.y() - p0.y() - p1.y()) / denominator,
        (factor * pm.z() - p0.z() - p1.z()) / denominator,
    )?)
}

fn conic_edge(curve: &Conic3D, id: u64) -> Result<NurbsCurve, StepError> {
    let (start, end) = curve.range_tuple();
    let spans = match curve {
        Conic3D::Ellipse(_) => arc_span_count(end - start, id)?,
        Conic3D::Hyperbola(_) | Conic3D::Parabola(_)
            if start.is_finite() && end.is_finite() && start < end =>
        {
            1
        }
        _ => {
            return Err(StepError::UnsupportedNativeShell {
                shell: id,
                reason: "conic edge parameter range is invalid",
            });
        }
    };
    let step = (end - start) / spans as f64;
    let mut controls = Vec::with_capacity(2 * spans + 1);
    for index in 0..spans {
        let t0 = start + step * index as f64;
        let t1 = if index + 1 == spans { end } else { t0 + step };
        let p0 = curve.evaluate(t0);
        let pm = curve.evaluate((t0 + t1) / 2.);
        let p1 = curve.evaluate(t1);
        let weight = match curve {
            Conic3D::Ellipse(_) => ((t1 - t0) / 2.).cos(),
            Conic3D::Hyperbola(_) => ((t1 - t0) / 2.).cosh(),
            Conic3D::Parabola(_) => 1.,
        };
        if index == 0 {
            controls.push(WeightedPoint3::try_new(point3(p0)?, 1.)?);
        }
        controls.push(WeightedPoint3::try_new(
            Point3::try_new(
                (2. * (1. + weight) * pm.x - p0.x - p1.x) / (2. * weight),
                (2. * (1. + weight) * pm.y - p0.y - p1.y) / (2. * weight),
                (2. * (1. + weight) * pm.z - p0.z - p1.z) / (2. * weight),
            )?,
            weight,
        )?);
        controls.push(WeightedPoint3::try_new(point3(p1)?, 1.)?);
    }
    Ok(NurbsCurve::try_new_rational(
        2,
        controls,
        arc_knots(0., 1., spans),
    )?)
}

fn conic_trim(curve: &Conic2D, id: u64) -> Result<NurbsCurve2, StepError> {
    let (start, end) = curve.range_tuple();
    let spans = match curve {
        Conic2D::Ellipse(_) => arc_span_count(end - start, id)?,
        Conic2D::Hyperbola(_) | Conic2D::Parabola(_)
            if start.is_finite() && end.is_finite() && start < end =>
        {
            1
        }
        _ => {
            return Err(StepError::UnsupportedNativeShell {
                shell: id,
                reason: "conic UV trim parameter range is invalid",
            });
        }
    };
    let step = (end - start) / spans as f64;
    let mut controls = Vec::with_capacity(2 * spans + 1);
    for index in 0..spans {
        let t0 = start + step * index as f64;
        let t1 = if index + 1 == spans { end } else { t0 + step };
        let p0 = curve.evaluate(t0);
        let pm = curve.evaluate((t0 + t1) / 2.);
        let p1 = curve.evaluate(t1);
        let weight = match curve {
            Conic2D::Ellipse(_) => ((t1 - t0) / 2.).cos(),
            Conic2D::Hyperbola(_) => ((t1 - t0) / 2.).cosh(),
            Conic2D::Parabola(_) => 1.,
        };
        if index == 0 {
            controls.push(WeightedPoint2::try_new(Point2::try_new(p0.x, p0.y)?, 1.)?);
        }
        controls.push(WeightedPoint2::try_new(
            Point2::try_new(
                (2. * (1. + weight) * pm.x - p0.x - p1.x) / (2. * weight),
                (2. * (1. + weight) * pm.y - p0.y - p1.y) / (2. * weight),
            )?,
            weight,
        )?);
        controls.push(WeightedPoint2::try_new(Point2::try_new(p1.x, p1.y)?, 1.)?);
    }
    Ok(NurbsCurve2::try_new_rational(
        2,
        controls,
        arc_knots(0., 1., spans),
    )?)
}

fn spline_surface_basis(source: &Surface, id: u64) -> Result<NurbsSurface, StepError> {
    match source {
        Surface::NurbsSurface(surface) => {
            let rows = surface.control_points();
            let u_count = rows.len();
            let v_count = rows.first().map_or(0, Vec::len);
            let controls = (0..v_count)
                .flat_map(|v| (0..u_count).map(move |u| &rows[u][v]))
                .map(|p| weighted3(p.x, p.y, p.z, p.w, id))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(NurbsSurface::try_new_rational(
                surface.udegree(),
                surface.vdegree(),
                u_count,
                v_count,
                controls,
                surface.knot_vector_u().iter().copied().collect(),
                surface.knot_vector_v().iter().copied().collect(),
            )?)
        }
        Surface::BsplineSurface(surface) => {
            let rows = surface.control_points();
            let u_count = rows.len();
            let v_count = rows.first().map_or(0, Vec::len);
            let controls = (0..v_count)
                .flat_map(|v| (0..u_count).map(move |u| &rows[u][v]))
                .copied()
                .map(point3)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(NurbsSurface::try_new(
                surface.udegree(),
                surface.vdegree(),
                u_count,
                v_count,
                controls,
                surface.knot_vector_u().iter().copied().collect(),
                surface.knot_vector_v().iter().copied().collect(),
            )?)
        }
        _ => Err(StepError::UnsupportedNativeShell {
            shell: id,
            reason: "p-curve basis is not a B-spline surface",
        }),
    }
}

fn surface(
    source: &Surface,
    boundaries: &[Vec<BrepTrim>],
    id: u64,
    tolerance: Tolerance,
) -> Result<NurbsSurface, StepError> {
    let unsupported = |reason| StepError::UnsupportedNativeShell { shell: id, reason };
    match source {
        Surface::NurbsSurface(_) | Surface::BsplineSurface(_) => spline_surface_basis(source, id),
        Surface::ElementarySurface(ElementarySurface::Plane(plane)) => {
            let points = boundaries
                .iter()
                .flatten()
                .flat_map(|trim| trim.curve().control_points().iter().map(|p| p.point()));
            let mut min = [f64::INFINITY; 2];
            let mut max = [f64::NEG_INFINITY; 2];
            for p in points {
                min[0] = min[0].min(p.x());
                min[1] = min[1].min(p.y());
                max[0] = max[0].max(p.x());
                max[1] = max[1].max(p.y());
            }
            if (0..2).any(|axis| min[axis] >= max[axis]) {
                return Err(unsupported("planar UV trim bounds are degenerate"));
            }
            let controls = [
                (min[0], min[1]),
                (max[0], min[1]),
                (min[0], max[1]),
                (max[0], max[1]),
            ]
            .into_iter()
            .map(|(u, v)| point3(plane.evaluate(u, v)))
            .collect::<Result<Vec<_>, _>>()?;
            Ok(NurbsSurface::try_new(
                1,
                1,
                2,
                2,
                controls,
                vec![min[0], min[0], max[0], max[0]],
                vec![min[1], min[1], max[1], max[1]],
            )?)
        }
        Surface::SweepSurface(SweepSurface::ExtrusionSurface(extrusion)) => {
            let directrix = sweep_directrix(extrusion.entity_curve(), id)?;
            let mut v0 = f64::INFINITY;
            let mut v1 = f64::NEG_INFINITY;
            for trim in boundaries.iter().flatten() {
                for control in trim.curve().control_points() {
                    v0 = v0.min(control.point().y());
                    v1 = v1.max(control.point().y());
                }
            }
            if !v0.is_finite() || !v1.is_finite() || v0 >= v1 {
                return Err(unsupported("extrusion axial trim range is degenerate"));
            }
            let vector = extrusion.extruding_vector();
            let controls = [v0, v1]
                .into_iter()
                .flat_map(|v| {
                    directrix.control_points().iter().map(move |control| {
                        WeightedPoint3::try_new(
                            Point3::try_new(
                                control.point().x() + v * vector.x,
                                control.point().y() + v * vector.y,
                                control.point().z() + v * vector.z,
                            )?,
                            control.weight(),
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(NurbsSurface::try_new_rational(
                directrix.degree(),
                1,
                directrix.control_points().len(),
                2,
                controls,
                directrix.knots().to_vec(),
                vec![v0, v0, v1, v1],
            )?)
        }
        Surface::SweepSurface(SweepSurface::RevolutionSurface(revolution)) => {
            if revolution.orientation() {
                return Err(unsupported(
                    "STEP revolution parameters are not angle-first",
                ));
            }
            let directrix = sweep_directrix(revolution.entity().entity_curve(), id)?;
            let mut u0 = f64::INFINITY;
            let mut u1 = f64::NEG_INFINITY;
            for trim in boundaries.iter().flatten() {
                if !trim.curve().is_straight_segment() {
                    return Err(unsupported("revolution requires straight UV iso-trims"));
                }
                let start = trim.curve().start_point()?;
                let end = trim.curve().end_point()?;
                if (start.x() - end.x()).abs() > tolerance.angular()
                    && (start.y() - end.y()).abs() > tolerance.absolute()
                {
                    return Err(unsupported("revolution requires UV iso-trims"));
                }
                u0 = u0.min(start.x()).min(end.x());
                u1 = u1.max(start.x()).max(end.x());
            }
            let spans = arc_span_count(u1 - u0, id)?;
            let step = (u1 - u0) / spans as f64;
            let mut controls =
                Vec::with_capacity((2 * spans + 1) * directrix.control_points().len());
            for control in directrix.control_points() {
                for index in 0..spans {
                    let start = u0 + step * index as f64;
                    let end = if index + 1 == spans { u1 } else { start + step };
                    let p0 = rotate_revolution_point(revolution, control.point(), start)?;
                    let pm =
                        rotate_revolution_point(revolution, control.point(), (start + end) / 2.)?;
                    let p1 = rotate_revolution_point(revolution, control.point(), end)?;
                    let weight = ((end - start) / 2.).cos();
                    if index == 0 {
                        controls.push(WeightedPoint3::try_new(p0, control.weight())?);
                    }
                    controls.push(WeightedPoint3::try_new(
                        circular_middle(p0, pm, p1, weight)?,
                        control.weight() * weight,
                    )?);
                    controls.push(WeightedPoint3::try_new(p1, control.weight())?);
                }
            }
            Ok(NurbsSurface::try_new_rational(
                2,
                directrix.degree(),
                2 * spans + 1,
                directrix.control_points().len(),
                controls,
                arc_knots(u0, u1, spans),
                directrix.knots().to_vec(),
            )?)
        }
        Surface::ElementarySurface(
            ElementarySurface::CylindricalSurface(revolution)
            | ElementarySurface::ConicalSurface(revolution),
        ) => {
            let mut min = [f64::INFINITY; 2];
            let mut max = [f64::NEG_INFINITY; 2];
            for trim in boundaries.iter().flatten() {
                if !trim.curve().is_straight_segment() {
                    return Err(unsupported(
                        "revolved line surface requires straight UV iso-trims",
                    ));
                }
                let start = trim.curve().start_point()?;
                let end = trim.curve().end_point()?;
                if (start.x() - end.x()).abs() > tolerance.angular()
                    && (start.y() - end.y()).abs() > tolerance.absolute()
                {
                    return Err(unsupported("revolved line surface requires UV iso-trims"));
                }
                for point in [start, end] {
                    min[0] = min[0].min(point.x());
                    min[1] = min[1].min(point.y());
                    max[0] = max[0].max(point.x());
                    max[1] = max[1].max(point.y());
                }
            }
            let [u0, v0] = min;
            let [u1, v1] = max;
            let angle = u1 - u0;
            if v0 >= v1 {
                return Err(unsupported("revolved line axial trim range is degenerate"));
            }
            let spans = arc_span_count(angle, id)?;
            let step = angle / spans as f64;
            let mut controls = Vec::with_capacity(2 * (2 * spans + 1));
            for v in [v0, v1] {
                for index in 0..spans {
                    let start = u0 + step * index as f64;
                    let end = if index + 1 == spans { u1 } else { start + step };
                    let p0 = revolution.evaluate(start, v);
                    let pm = revolution.evaluate((start + end) / 2., v);
                    let p1 = revolution.evaluate(end, v);
                    let weight = ((end - start) / 2.).cos();
                    if index == 0 {
                        controls.push(WeightedPoint3::try_new(point3(p0)?, 1.)?);
                    }
                    let middle = Point3::try_new(
                        (2. * (1. + weight) * pm.x - p0.x - p1.x) / (2. * weight),
                        (2. * (1. + weight) * pm.y - p0.y - p1.y) / (2. * weight),
                        (2. * (1. + weight) * pm.z - p0.z - p1.z) / (2. * weight),
                    )?;
                    controls.push(WeightedPoint3::try_new(middle, weight)?);
                    controls.push(WeightedPoint3::try_new(point3(p1)?, 1.)?);
                }
            }
            Ok(NurbsSurface::try_new_rational(
                2,
                1,
                2 * spans + 1,
                2,
                controls,
                arc_knots(u0, u1, spans),
                vec![v0, v0, v1, v1],
            )?)
        }
        Surface::ElementarySurface(
            ElementarySurface::ToroidalSurface(_) | ElementarySurface::Sphere(_),
        ) => {
            let mut min = [f64::INFINITY; 2];
            let mut max = [f64::NEG_INFINITY; 2];
            for trim in boundaries.iter().flatten() {
                if !trim.curve().is_straight_segment() {
                    return Err(unsupported(
                        "angular surface requires straight UV iso-trims",
                    ));
                }
                let start = trim.curve().start_point()?;
                let end = trim.curve().end_point()?;
                if (start.x() - end.x()).abs() > tolerance.angular()
                    && (start.y() - end.y()).abs() > tolerance.angular()
                {
                    return Err(unsupported("angular surface requires UV iso-trims"));
                }
                for point in [start, end] {
                    min[0] = min[0].min(point.x());
                    min[1] = min[1].min(point.y());
                    max[0] = max[0].max(point.x());
                    max[1] = max[1].max(point.y());
                }
            }
            let [u0, v0] = min;
            let [u1, v1] = max;
            if matches!(
                source,
                Surface::ElementarySurface(ElementarySurface::Sphere(_))
            ) {
                if v0 < -std::f64::consts::FRAC_PI_2 - tolerance.angular()
                    || v1 > std::f64::consts::FRAC_PI_2 + tolerance.angular()
                {
                    return Err(unsupported("sphere latitude lies outside [-pi/2, pi/2]"));
                }
                if v0 <= -std::f64::consts::FRAC_PI_2 + tolerance.angular()
                    || v1 >= std::f64::consts::FRAC_PI_2 - tolerance.angular()
                {
                    return Err(unsupported("sphere pole requires singular trim support"));
                }
            }
            let u_spans = arc_span_count(u1 - u0, id)?;
            let v_spans = arc_span_count(v1 - v0, id)?;
            let u_step = (u1 - u0) / u_spans as f64;
            let v_step = (v1 - v0) / v_spans as f64;
            // U control positions vary circularly with V; interpolating those
            // positions as rational V arcs gives the exact tensor product.
            let u_controls_at = |v: f64| -> Result<Vec<(Point3, f64)>, StepError> {
                let mut row = Vec::with_capacity(2 * u_spans + 1);
                for index in 0..u_spans {
                    let start = u0 + u_step * index as f64;
                    let end = if index + 1 == u_spans {
                        u1
                    } else {
                        start + u_step
                    };
                    let p0 = point3(source.evaluate(start, v))?;
                    let pm = point3(source.evaluate((start + end) / 2., v))?;
                    let p1 = point3(source.evaluate(end, v))?;
                    let weight = ((end - start) / 2.).cos();
                    if index == 0 {
                        row.push((p0, 1.));
                    }
                    row.push((circular_middle(p0, pm, p1, weight)?, weight));
                    row.push((p1, 1.));
                }
                Ok(row)
            };
            let mut controls = Vec::with_capacity((2 * u_spans + 1) * (2 * v_spans + 1));
            for index in 0..v_spans {
                let start = v0 + v_step * index as f64;
                let end = if index + 1 == v_spans {
                    v1
                } else {
                    start + v_step
                };
                let bottom = u_controls_at(start)?;
                let middle = u_controls_at((start + end) / 2.)?;
                let top = u_controls_at(end)?;
                if index == 0 {
                    for &(point, weight) in &bottom {
                        controls.push(WeightedPoint3::try_new(point, weight)?);
                    }
                }
                let v_weight = ((end - start) / 2.).cos();
                for ((bottom, middle), top) in bottom.iter().zip(&middle).zip(&top) {
                    controls.push(WeightedPoint3::try_new(
                        circular_middle(bottom.0, middle.0, top.0, v_weight)?,
                        bottom.1 * v_weight,
                    )?);
                }
                for (point, weight) in top {
                    controls.push(WeightedPoint3::try_new(point, weight)?);
                }
            }
            Ok(NurbsSurface::try_new_rational(
                2,
                2,
                2 * u_spans + 1,
                2 * v_spans + 1,
                controls,
                arc_knots(u0, u1, u_spans),
                arc_knots(v0, v1, v_spans),
            )?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use monstertruck::core::cgmath64::Vector3;
    use monstertruck::modeling::{
        BsplineCurve, BsplineSurface, Invertible, KnotVector, Line, NurbsCurve as TruckNurbsCurve,
        NurbsSurface as TruckNurbsSurface, Plane, Point2 as TruckPoint2, Point3 as TruckPoint3,
        PolylineCurve, Processor, RevolutionSurface, Sphere as TruckSphere, Torus, Vector4,
        builder,
    };
    use monstertruck::step::load::step_geometry::{
        Sphere as StepSphere, StepExtrusionSurface, StepParameterCurve, SurfaceCurveKind,
        SurfaceCurveRepresentation,
    };
    use monstertruck::topology::Vertex;

    #[test]
    fn planar_pcurve_edge_lifts_rational_controls_without_losing_weights() {
        let knots = || {
            (
                KnotVector::from(vec![0., 0., 2., 2.]),
                KnotVector::bezier_knot(1),
            )
        };
        for basis in [
            Surface::ElementarySurface(ElementarySurface::Plane(Plane::new(
                TruckPoint3::new(10., 20., 30.),
                TruckPoint3::new(12., 20., 30.),
                TruckPoint3::new(10., 23., 30.),
            ))),
            Surface::SweepSurface(SweepSurface::ExtrusionSurface(
                StepExtrusionSurface::by_extrusion(
                    Curve3D::Line(Line(
                        TruckPoint3::new(10., 20., 30.),
                        TruckPoint3::new(12., 20., 30.),
                    )),
                    Vector3::new(0., 3., 0.),
                ),
            )),
            Surface::BsplineSurface(BsplineSurface::new(
                knots(),
                vec![
                    vec![
                        TruckPoint3::new(10., 20., 30.),
                        TruckPoint3::new(10., 23., 30.),
                    ],
                    vec![
                        TruckPoint3::new(14., 20., 30.),
                        TruckPoint3::new(14., 23., 30.),
                    ],
                ],
            )),
            Surface::NurbsSurface(TruckNurbsSurface::new(BsplineSurface::new(
                knots(),
                vec![
                    vec![
                        Vector4::new(20., 40., 60., 2.),
                        Vector4::new(20., 46., 60., 2.),
                    ],
                    vec![
                        Vector4::new(28., 40., 60., 2.),
                        Vector4::new(28., 46., 60., 2.),
                    ],
                ],
            ))),
        ] {
            let parameter = StepParameterCurve::new(
                Box::new(Curve2D::NurbsCurve(TruckNurbsCurve::new(
                    BsplineCurve::new(
                        KnotVector::bezier_knot(2),
                        vec![
                            Vector3::new(0., 0., 1.),
                            Vector3::new(0.5, 0.5, 0.5),
                            Vector3::new(2., 0., 1.),
                        ],
                    ),
                ))),
                Box::new(basis),
            );
            let lifted = edge_curve(&Curve3D::ParameterCurve(parameter.clone()), 1).unwrap();
            assert_eq!(lifted.degree(), 2);
            assert_eq!(lifted.knots(), &[0., 0., 0., 1., 1., 1.]);
            assert_eq!(
                lifted
                    .control_points()
                    .iter()
                    .map(|control| control.weight())
                    .collect::<Vec<_>>(),
                vec![1., 0.5, 1.]
            );
            for t in [0., 0.17, 0.5, 0.83, 1.] {
                let uv = parameter.curve().evaluate(t);
                let expected = parameter.surface().evaluate(uv.x, uv.y);
                let actual = lifted.evaluate(t).unwrap();
                assert!((actual.x() - expected.x).abs() < 1e-12);
                assert!((actual.y() - expected.y).abs() < 1e-12);
                assert!((actual.z() - expected.z).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn affine_surface_certificate_rejects_warp_and_unequal_weights() {
        let knots = || (KnotVector::bezier_knot(1), KnotVector::bezier_knot(1));
        let warped = Surface::BsplineSurface(BsplineSurface::new(
            knots(),
            vec![
                vec![TruckPoint3::new(0., 0., 0.), TruckPoint3::new(0., 10., 0.)],
                vec![
                    TruckPoint3::new(10., 0., 0.),
                    TruckPoint3::new(10., 10., f64::from_bits(1)),
                ],
            ],
        ));
        assert!(affine_bilinear_basis(&warped).is_none());

        let unequal = Surface::NurbsSurface(TruckNurbsSurface::new(BsplineSurface::new(
            knots(),
            vec![
                vec![Vector4::new(0., 0., 0., 1.), Vector4::new(0., 10., 0., 1.)],
                vec![
                    Vector4::new(10., 0., 0., 1.),
                    Vector4::new(10., 10., 0., 2.),
                ],
            ],
        )));
        assert!(affine_bilinear_basis(&unequal).is_none());

        let affine = Surface::BsplineSurface(BsplineSurface::new(
            knots(),
            vec![
                vec![TruckPoint3::new(0., 0., 0.), TruckPoint3::new(0., 10., 0.)],
                vec![
                    TruckPoint3::new(10., 0., 0.),
                    TruckPoint3::new(10., 10., 0.),
                ],
            ],
        ));
        let outside = Curve3D::ParameterCurve(StepParameterCurve::new(
            Box::new(Curve2D::Polyline(PolylineCurve(vec![
                TruckPoint2::new(-0.1, 0.),
                TruckPoint2::new(1., 0.),
            ]))),
            Box::new(affine),
        ));
        assert!(matches!(
            edge_curve(&outside, 1),
            Err(StepError::UnsupportedNativeShell {
                reason: "3D edge p-curve leaves its affine surface domain",
                ..
            })
        ));
    }

    #[test]
    fn bilinear_surface_diagonal_pcurves_compose_to_exact_quadratics() {
        let knots = || {
            (
                KnotVector::from(vec![2., 2., 4., 4.]),
                KnotVector::from(vec![-3., -3., 5., 5.]),
            )
        };
        for (basis, rational) in [
            (
                Surface::BsplineSurface(BsplineSurface::new(
                    knots(),
                    vec![
                        vec![TruckPoint3::new(0., 0., 0.), TruckPoint3::new(0., 2., 0.)],
                        vec![TruckPoint3::new(2., 0., 0.), TruckPoint3::new(2., 2., 1.)],
                    ],
                )),
                false,
            ),
            (
                Surface::NurbsSurface(TruckNurbsSurface::new(BsplineSurface::new(
                    knots(),
                    vec![
                        vec![Vector4::new(0., 0., 0., 1.), Vector4::new(0., 4., 0., 2.)],
                        vec![Vector4::new(6., 0., 0., 3.), Vector4::new(8., 8., 4., 4.)],
                    ],
                ))),
                true,
            ),
            (
                Surface::BsplineSurface(BsplineSurface::new(
                    (
                        KnotVector::from(vec![0., 2., 4., 6.]),
                        KnotVector::from(vec![-5., -3., 5., 7.]),
                    ),
                    vec![
                        vec![TruckPoint3::new(0., 0., 0.), TruckPoint3::new(0., 2., 0.)],
                        vec![TruckPoint3::new(2., 0., 0.), TruckPoint3::new(2., 2., 1.)],
                    ],
                )),
                false,
            ),
        ] {
            assert!(affine_bilinear_basis(&basis).is_none());
            for (a, b) in [
                ([2., -3.], [4., 5.]),
                ([2.3, -1.], [3.7, 4.]),
                ([3.7, 4.], [2.3, -1.]),
            ] {
                let uv0 = TruckPoint2::new(a[0], a[1]);
                let uv1 = TruckPoint2::new(b[0], b[1]);
                for (uv, domain) in [
                    (Curve2D::Line(Line(uv0, uv1)), 0.0..=1.0),
                    (
                        Curve2D::BsplineCurve(BsplineCurve::new(
                            KnotVector::from(vec![5., 5., 9., 9.]),
                            vec![uv0, uv1],
                        )),
                        5.0..=9.0,
                    ),
                    (
                        Curve2D::NurbsCurve(TruckNurbsCurve::new(BsplineCurve::new(
                            KnotVector::from(vec![5., 5., 9., 9.]),
                            vec![
                                Vector3::new(2. * a[0], 2. * a[1], 2.),
                                Vector3::new(2. * b[0], 2. * b[1], 2.),
                            ],
                        ))),
                        5.0..=9.0,
                    ),
                ] {
                    let source = Curve3D::ParameterCurve(StepParameterCurve::new(
                        Box::new(uv),
                        Box::new(basis.clone()),
                    ));
                    let curve = edge_curve(&source, 1).unwrap();
                    assert_eq!(curve.degree(), 2);
                    assert_eq!(curve.control_points().len(), 3);
                    assert_eq!(curve.domain(), domain);
                    for fraction in [0., 0.17, 0.5, 0.83, 1.] {
                        let u = a[0] * (1. - fraction) + b[0] * fraction;
                        let v = a[1] * (1. - fraction) + b[1] * fraction;
                        let t = *domain.start() * (1. - fraction) + *domain.end() * fraction;
                        let expected = basis.evaluate(u, v);
                        let actual = curve.evaluate(t).unwrap();
                        assert!((actual.x() - expected.x).abs() < 1e-11);
                        assert!((actual.y() - expected.y).abs() < 1e-11);
                        assert!((actual.z() - expected.z).abs() < 1e-11);
                    }
                    if a == [2., -3.] && rational {
                        assert_eq!(
                            curve
                                .control_points()
                                .iter()
                                .map(|control| control.weight())
                                .collect::<Vec<_>>(),
                            vec![1., 2.5, 4.]
                        );
                    }
                }
            }

            let outside = Curve3D::ParameterCurve(StepParameterCurve::new(
                Box::new(Curve2D::Line(Line(
                    TruckPoint2::new(1.9, -3.),
                    TruckPoint2::new(4., 5.),
                ))),
                Box::new(basis),
            ));
            assert!(matches!(
                edge_curve(&outside, 1),
                Err(StepError::UnsupportedNativeShell {
                    reason: "3D edge p-curve leaves its bilinear surface domain",
                    ..
                })
            ));
        }

        let mixed_weights = Surface::NurbsSurface(TruckNurbsSurface::new(BsplineSurface::new(
            knots(),
            vec![
                vec![Vector4::new(0., 0., 0., 1.), Vector4::new(0., 2., 0., 1.)],
                vec![
                    Vector4::new(2., 0., 0., 1.),
                    Vector4::new(-2., -2., -1., -1.),
                ],
            ],
        )));
        let diagonal = Curve3D::ParameterCurve(StepParameterCurve::new(
            Box::new(Curve2D::Line(Line(
                TruckPoint2::new(2., -3.),
                TruckPoint2::new(4., 5.),
            ))),
            Box::new(mixed_weights),
        ));
        assert!(matches!(
            edge_curve(&diagonal, 1),
            Err(StepError::UnsupportedNativeShell {
                reason: "bilinear p-curve surface weights change sign",
                ..
            })
        ));
    }

    #[test]
    fn bezier_surface_diagonal_pcurves_compose_to_exact_rational_curves() {
        let polynomial = Surface::BsplineSurface(BsplineSurface::new(
            (
                KnotVector::from(vec![2., 2., 2., 4., 4., 4.]),
                KnotVector::from(vec![-3., -3., 5., 5.]),
            ),
            vec![
                vec![TruckPoint3::new(0., 0., 0.), TruckPoint3::new(0., 2., 0.)],
                vec![TruckPoint3::new(1., 0., 1.), TruckPoint3::new(1., 2., 2.)],
                vec![TruckPoint3::new(2., 0., 0.), TruckPoint3::new(2., 2., 1.)],
            ],
        ));
        let rational = Surface::NurbsSurface(TruckNurbsSurface::new(BsplineSurface::new(
            (
                KnotVector::from(vec![2., 2., 2., 4., 4., 4.]),
                KnotVector::from(vec![-3., -3., -3., 5., 5., 5.]),
            ),
            (0..3)
                .map(|u| {
                    (0..3)
                        .map(|v| {
                            let x = u as f64;
                            let y = v as f64;
                            let weight = 1. + x + y;
                            Vector4::new(x * weight, y * weight, x * y * weight, weight)
                        })
                        .collect::<Vec<_>>()
                })
                .collect(),
        )));
        let cubic_quadratic = Surface::BsplineSurface(BsplineSurface::new(
            (
                KnotVector::from(vec![2., 2., 2., 2., 4., 4., 4., 4.]),
                KnotVector::from(vec![-3., -3., -3., 5., 5., 5.]),
            ),
            (0..4)
                .map(|u| {
                    (0..3)
                        .map(|v| {
                            let x = u as f64;
                            let y = v as f64;
                            TruckPoint3::new(x, y, x * x + x * y)
                        })
                        .collect::<Vec<_>>()
                })
                .collect(),
        ));
        for (basis, degree) in [(polynomial, 3), (rational, 4), (cubic_quadratic, 5)] {
            for (a, b) in [
                ([2., -3.], [4., 5.]),
                ([2.3, -1.], [3.7, 4.]),
                ([3.7, 4.], [2.3, -1.]),
                ([2.3, 4.], [3.7, -1.]),
            ] {
                let uv0 = TruckPoint2::new(a[0], a[1]);
                let uv1 = TruckPoint2::new(b[0], b[1]);
                for (uv, domain) in [
                    (Curve2D::Line(Line(uv0, uv1)), 0.0..=1.0),
                    (
                        Curve2D::BsplineCurve(BsplineCurve::new(
                            KnotVector::from(vec![5., 5., 9., 9.]),
                            vec![uv0, uv1],
                        )),
                        5.0..=9.0,
                    ),
                    (
                        Curve2D::NurbsCurve(TruckNurbsCurve::new(BsplineCurve::new(
                            KnotVector::from(vec![5., 5., 9., 9.]),
                            vec![
                                Vector3::new(2. * a[0], 2. * a[1], 2.),
                                Vector3::new(2. * b[0], 2. * b[1], 2.),
                            ],
                        ))),
                        5.0..=9.0,
                    ),
                ] {
                    let source = Curve3D::ParameterCurve(StepParameterCurve::new(
                        Box::new(uv),
                        Box::new(basis.clone()),
                    ));
                    let curve = edge_curve(&source, 1).unwrap();
                    assert_eq!(curve.degree(), degree);
                    assert_eq!(curve.domain(), domain);
                    for fraction in [0., 0.17, 0.5, 0.83, 1.] {
                        let u = a[0] * (1. - fraction) + b[0] * fraction;
                        let v = a[1] * (1. - fraction) + b[1] * fraction;
                        let t = *domain.start() * (1. - fraction) + *domain.end() * fraction;
                        let expected = basis.evaluate(u, v);
                        let actual = curve.evaluate(t).unwrap();
                        assert!((actual.x() - expected.x).abs() < 1e-10);
                        assert!((actual.y() - expected.y).abs() < 1e-10);
                        assert!((actual.z() - expected.z).abs() < 1e-10);
                    }
                }
            }
        }

        let multispan = Surface::BsplineSurface(BsplineSurface::new(
            (
                KnotVector::from(vec![0., 0., 0., 0.5, 1., 1., 1.]),
                KnotVector::bezier_knot(1),
            ),
            (0..4)
                .map(|u| {
                    (0..2)
                        .map(|v| TruckPoint3::new(u as f64, v as f64, (u * v) as f64))
                        .collect::<Vec<_>>()
                })
                .collect(),
        ));
        let diagonal = Curve3D::ParameterCurve(StepParameterCurve::new(
            Box::new(Curve2D::Line(Line(
                TruckPoint2::new(0., 0.),
                TruckPoint2::new(1., 1.),
            ))),
            Box::new(multispan.clone()),
        ));
        let curve = edge_curve(&diagonal, 1).unwrap();
        assert_eq!(curve.degree(), 3);
        assert_eq!(
            curve.knots(),
            &[0., 0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1., 1.]
        );
        for t in [0., 0.17, 0.5, 0.83, 1.] {
            let actual = curve.evaluate(t).unwrap();
            let expected = multispan.evaluate(t, t);
            assert!((actual.x() - expected.x).abs() < 1e-10);
            assert!((actual.y() - expected.y).abs() < 1e-10);
            assert!((actual.z() - expected.z).abs() < 1e-10);
        }
        for (a, b) in [([0.1, 0.2], [0.3, 0.8]), ([0.6, 0.2], [0.9, 0.8])] {
            let source = Curve3D::ParameterCurve(StepParameterCurve::new(
                Box::new(Curve2D::Line(Line(
                    TruckPoint2::new(a[0], a[1]),
                    TruckPoint2::new(b[0], b[1]),
                ))),
                Box::new(multispan.clone()),
            ));
            let curve = edge_curve(&source, 1).unwrap();
            assert_eq!(curve.degree(), 3);
            for t in [0., 0.17, 0.5, 0.83, 1.] {
                let u = a[0] * (1. - t) + b[0] * t;
                let v = a[1] * (1. - t) + b[1] * t;
                let actual = curve.evaluate(t).unwrap();
                let expected = multispan.evaluate(u, v);
                assert!((actual.x() - expected.x).abs() < 1e-10);
                assert!((actual.y() - expected.y).abs() < 1e-10);
                assert!((actual.z() - expected.z).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn rational_linear_uv_pcurve_preserves_unequal_weight_parameterization() {
        let polynomial = Surface::BsplineSurface(BsplineSurface::new(
            (KnotVector::bezier_knot(2), KnotVector::bezier_knot(1)),
            vec![
                vec![TruckPoint3::new(0., 0., 0.), TruckPoint3::new(0., 2., 0.)],
                vec![TruckPoint3::new(1., 0., 1.), TruckPoint3::new(1., 2., 2.)],
                vec![TruckPoint3::new(2., 0., 0.), TruckPoint3::new(2., 2., 1.)],
            ],
        ));
        let rational = Surface::NurbsSurface(TruckNurbsSurface::new(BsplineSurface::new(
            (KnotVector::bezier_knot(2), KnotVector::bezier_knot(2)),
            (0..3)
                .map(|u| {
                    (0..3)
                        .map(|v| {
                            let x = u as f64;
                            let y = v as f64;
                            let weight = 1. + 0.2 * x + 0.3 * y;
                            Vector4::new(x * weight, y * weight, x * y * weight, weight)
                        })
                        .collect::<Vec<_>>()
                })
                .collect(),
        )));
        for (basis, degree) in [(polynomial, 3), (rational, 4)] {
            for (a, b, weights) in [
                ([0.1, 0.2], [0.9, 0.8], [1., 2.]),
                ([0.9, 0.8], [0.1, 0.2], [2., 1.]),
                ([0.1, 0.8], [0.9, 0.2], [-1., -3.]),
            ] {
                let uv = Curve2D::NurbsCurve(TruckNurbsCurve::new(BsplineCurve::new(
                    KnotVector::from(vec![5., 5., 9., 9.]),
                    vec![
                        Vector3::new(a[0] * weights[0], a[1] * weights[0], weights[0]),
                        Vector3::new(b[0] * weights[1], b[1] * weights[1], weights[1]),
                    ],
                )));
                let source = Curve3D::ParameterCurve(StepParameterCurve::new(
                    Box::new(uv),
                    Box::new(basis.clone()),
                ));
                let curve = edge_curve(&source, 1).unwrap();
                assert_eq!(curve.degree(), degree);
                assert_eq!(curve.domain(), 5.0..=9.0);
                for fraction in [0., 0.17, 0.5, 0.83, 1.] {
                    let denominator = weights[0] * (1. - fraction) + weights[1] * fraction;
                    let u = (a[0] * weights[0] * (1. - fraction) + b[0] * weights[1] * fraction)
                        / denominator;
                    let v = (a[1] * weights[0] * (1. - fraction) + b[1] * weights[1] * fraction)
                        / denominator;
                    let expected = basis.evaluate(u, v);
                    let actual = curve.evaluate(5. + 4. * fraction).unwrap();
                    assert!((actual.x() - expected.x).abs() < 1e-10);
                    assert!((actual.y() - expected.y).abs() < 1e-10);
                    assert!((actual.z() - expected.z).abs() < 1e-10);
                }
            }
        }
    }

    #[test]
    fn multispan_rational_surface_diagonal_pcurves_cross_both_knot_directions() {
        let basis = Surface::NurbsSurface(TruckNurbsSurface::new(BsplineSurface::new(
            (
                KnotVector::from(vec![0., 0., 0., 0.25, 1., 1., 1.]),
                KnotVector::from(vec![0., 0., 0., 0.75, 1., 1., 1.]),
            ),
            (0..4)
                .map(|u| {
                    (0..4)
                        .map(|v| {
                            let x = u as f64;
                            let y = v as f64;
                            let weight = 1. + 0.2 * x + 0.3 * y;
                            Vector4::new(
                                x * weight,
                                y * weight,
                                (x * y + if u == 2 && v == 1 { 1. } else { 0. }) * weight,
                                weight,
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect(),
        )));
        for (a, b) in [
            ([0.1, 0.2], [0.9, 0.8]),
            ([0.9, 0.8], [0.1, 0.2]),
            ([0., 0.5], [0.5, 1.]),
        ] {
            let uv0 = TruckPoint2::new(a[0], a[1]);
            let uv1 = TruckPoint2::new(b[0], b[1]);
            for (uv, domain) in [
                (Curve2D::Line(Line(uv0, uv1)), 0.0..=1.0),
                (
                    Curve2D::BsplineCurve(BsplineCurve::new(
                        KnotVector::from(vec![5., 5., 9., 9.]),
                        vec![uv0, uv1],
                    )),
                    5.0..=9.0,
                ),
            ] {
                let source = Curve3D::ParameterCurve(StepParameterCurve::new(
                    Box::new(uv),
                    Box::new(basis.clone()),
                ));
                let curve = edge_curve(&source, 1).unwrap();
                assert_eq!(curve.degree(), 4);
                assert_eq!(curve.domain(), domain);
                let mut fractions = vec![0., 0.17, 0.5, 0.83, 1.];
                for (knot, start, end) in [(0.25, a[0], b[0]), (0.75, a[1], b[1])] {
                    let crossing = (knot - start) / (end - start);
                    if (0.0..1.0).contains(&crossing) {
                        fractions.extend([crossing - 1e-6, crossing, crossing + 1e-6]);
                    }
                }
                for fraction in fractions {
                    let u = a[0] * (1. - fraction) + b[0] * fraction;
                    let v = a[1] * (1. - fraction) + b[1] * fraction;
                    let t = *domain.start() * (1. - fraction) + *domain.end() * fraction;
                    let expected = basis.evaluate(u, v);
                    let actual = curve.evaluate(t).unwrap();
                    assert!((actual.x() - expected.x).abs() < 1e-9);
                    assert!((actual.y() - expected.y).abs() < 1e-9);
                    assert!((actual.z() - expected.z).abs() < 1e-9);
                }
            }
        }

        let nearly_coincident = Surface::BsplineSurface(BsplineSurface::new(
            (
                KnotVector::from(vec![0., 0., 0.35, 1., 1.]),
                KnotVector::from(vec![0., 0., 0.65, 1., 1.]),
            ),
            (0..3)
                .map(|u| {
                    (0..3)
                        .map(|v| TruckPoint3::new(u as f64, v as f64, 0.))
                        .collect::<Vec<_>>()
                })
                .collect(),
        ));
        let diagonal = Curve3D::ParameterCurve(StepParameterCurve::new(
            Box::new(Curve2D::Line(Line(
                TruckPoint2::new(0., 0.3),
                TruckPoint2::new(0.7, 1.),
            ))),
            Box::new(nearly_coincident),
        ));
        assert!(matches!(
            edge_curve(&diagonal, 1),
            Err(StepError::UnsupportedNativeShell {
                reason: "p-curve knot crossings are too close to compose",
                ..
            })
        ));
    }

    #[test]
    fn rational_linear_uv_pcurve_crosses_surface_knots_at_source_parameters() {
        let basis = Surface::NurbsSurface(TruckNurbsSurface::new(BsplineSurface::new(
            (
                KnotVector::from(vec![0., 0., 0., 0.25, 1., 1., 1.]),
                KnotVector::from(vec![0., 0., 0., 0.75, 1., 1., 1.]),
            ),
            (0..4)
                .map(|u| {
                    (0..4)
                        .map(|v| {
                            let x = u as f64;
                            let y = v as f64;
                            let weight = 1. + 0.2 * x + 0.3 * y;
                            Vector4::new(x * weight, y * weight, x * y * weight, weight)
                        })
                        .collect::<Vec<_>>()
                })
                .collect(),
        )));
        for (a, b, weights) in [
            ([0.1, 0.2], [0.9, 0.8], [1., 2.]),
            ([0.9, 0.8], [0.1, 0.2], [2., 1.]),
            ([0., 0.5], [0.5, 1.], [1., 2.]),
        ] {
            let source = Curve3D::ParameterCurve(StepParameterCurve::new(
                Box::new(Curve2D::NurbsCurve(TruckNurbsCurve::new(
                    BsplineCurve::new(
                        KnotVector::from(vec![5., 5., 9., 9.]),
                        vec![
                            Vector3::new(a[0] * weights[0], a[1] * weights[0], weights[0]),
                            Vector3::new(b[0] * weights[1], b[1] * weights[1], weights[1]),
                        ],
                    ),
                ))),
                Box::new(basis.clone()),
            ));
            let curve = edge_curve(&source, 1).unwrap();
            assert_eq!(curve.degree(), 4);
            assert_eq!(curve.domain(), 5.0..=9.0);
            let mut parameters = vec![0., 0.17, 0.5, 0.83, 1.];
            for (knot, from, to) in [(0.25, a[0], b[0]), (0.75, a[1], b[1])] {
                let position = (knot - from) / (to - from);
                if (0.0..1.0).contains(&position) {
                    let t = weights[0] * position
                        / (weights[1] * (1. - position) + weights[0] * position);
                    parameters.extend([t - 1e-6, t, t + 1e-6]);
                }
            }
            for fraction in parameters {
                let denominator = weights[0] * (1. - fraction) + weights[1] * fraction;
                let u = (a[0] * weights[0] * (1. - fraction) + b[0] * weights[1] * fraction)
                    / denominator;
                let v = (a[1] * weights[0] * (1. - fraction) + b[1] * weights[1] * fraction)
                    / denominator;
                let expected = basis.evaluate(u, v);
                let actual = curve.evaluate(5. + 4. * fraction).unwrap();
                assert!((actual.x() - expected.x).abs() < 1e-9);
                assert!((actual.y() - expected.y).abs() < 1e-9);
                assert!((actual.z() - expected.z).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn unclamped_single_span_surface_diagonal_pcurve_clamps_without_changing_locus() {
        let basis = Surface::BsplineSurface(BsplineSurface::new(
            (
                KnotVector::from(vec![0., 1., 2., 3., 4., 5.]),
                KnotVector::from(vec![-2., -1., 0., 1., 2., 3.]),
            ),
            (0..3)
                .map(|u| {
                    (0..3)
                        .map(|v| {
                            let x = u as f64;
                            let y = v as f64;
                            TruckPoint3::new(x, y, x * x + x * y)
                        })
                        .collect::<Vec<_>>()
                })
                .collect(),
        ));
        for (a, b) in [([2., 0.], [3., 1.]), ([2.1, 0.2], [2.8, 0.9])] {
            let source = Curve3D::ParameterCurve(StepParameterCurve::new(
                Box::new(Curve2D::Line(Line(
                    TruckPoint2::new(a[0], a[1]),
                    TruckPoint2::new(b[0], b[1]),
                ))),
                Box::new(basis.clone()),
            ));
            let curve = edge_curve(&source, 1).unwrap();
            assert_eq!(curve.degree(), 4);
            for t in [0., 0.17, 0.5, 0.83, 1.] {
                let u = a[0] * (1. - t) + b[0] * t;
                let v = a[1] * (1. - t) + b[1] * t;
                let expected = basis.evaluate(u, v);
                let actual = curve.evaluate(t).unwrap();
                assert!((actual.x() - expected.x).abs() < 1e-10);
                assert!((actual.y() - expected.y).abs() < 1e-10);
                assert!((actual.z() - expected.z).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn curved_surface_pcurve_edges_preserve_partial_and_reversed_iso_segments() {
        let knots = || (KnotVector::bezier_knot(2), KnotVector::bezier_knot(1));
        for basis in [
            Surface::BsplineSurface(BsplineSurface::new(
                knots(),
                vec![
                    vec![TruckPoint3::new(0., 0., 0.), TruckPoint3::new(0., 0., 3.)],
                    vec![TruckPoint3::new(1., 1., 0.), TruckPoint3::new(1., 1., 3.)],
                    vec![TruckPoint3::new(2., 0., 0.), TruckPoint3::new(2., 0., 3.)],
                ],
            )),
            Surface::NurbsSurface(TruckNurbsSurface::new(BsplineSurface::new(
                knots(),
                vec![
                    vec![Vector4::new(0., 0., 0., 1.), Vector4::new(0., 0., 3., 1.)],
                    vec![
                        Vector4::new(0.5, 0.5, 0., 0.5),
                        Vector4::new(0.5, 0.5, 1.5, 0.5),
                    ],
                    vec![Vector4::new(2., 0., 0., 1.), Vector4::new(2., 0., 3., 1.)],
                ],
            ))),
        ] {
            for (a, b, degree) in [
                ([0.2, 0.25], [0.8, 0.25], 2),
                ([0.8, 0.25], [0.2, 0.25], 2),
                ([0.5, 0.9], [0.5, 0.1], 1),
            ] {
                let uv0 = TruckPoint2::new(a[0], a[1]);
                let uv1 = TruckPoint2::new(b[0], b[1]);
                let knots = || KnotVector::from(vec![5., 5., 9., 9.]);
                let cases = [
                    (Curve2D::Line(Line(uv0, uv1)), 0.0..=1.0),
                    (Curve2D::Polyline(PolylineCurve(vec![uv0, uv1])), 0.0..=1.0),
                    (
                        Curve2D::BsplineCurve(BsplineCurve::new(knots(), vec![uv0, uv1])),
                        5.0..=9.0,
                    ),
                    (
                        Curve2D::NurbsCurve(TruckNurbsCurve::new(BsplineCurve::new(
                            knots(),
                            vec![
                                Vector3::new(2. * a[0], 2. * a[1], 2.),
                                Vector3::new(2. * b[0], 2. * b[1], 2.),
                            ],
                        ))),
                        5.0..=9.0,
                    ),
                ];
                for (uv, domain) in cases {
                    let source = Curve3D::ParameterCurve(StepParameterCurve::new(
                        Box::new(uv),
                        Box::new(basis.clone()),
                    ));
                    let curve = edge_curve(&source, 1).unwrap();
                    assert_eq!(curve.degree(), degree);
                    assert_eq!(curve.domain(), domain);
                    for fraction in [0., 0.17, 0.5, 0.83, 1.] {
                        let u = a[0] * (1. - fraction) + b[0] * fraction;
                        let v = a[1] * (1. - fraction) + b[1] * fraction;
                        let t = *domain.start() * (1. - fraction) + *domain.end() * fraction;
                        let expected = basis.evaluate(u, v);
                        let actual = curve.evaluate(t).unwrap();
                        assert!((actual.x() - expected.x).abs() < 1e-11);
                        assert!((actual.y() - expected.y).abs() < 1e-11);
                        assert!((actual.z() - expected.z).abs() < 1e-11);
                    }
                }
            }

            let unequal_weight_iso = Curve3D::ParameterCurve(StepParameterCurve::new(
                Box::new(Curve2D::NurbsCurve(TruckNurbsCurve::new(
                    BsplineCurve::new(
                        KnotVector::bezier_knot(1),
                        vec![Vector3::new(0.2, 0.25, 1.), Vector3::new(1.6, 0.5, 2.)],
                    ),
                ))),
                Box::new(basis.clone()),
            ));
            let curve = edge_curve(&unequal_weight_iso, 1).unwrap();
            assert_eq!(curve.degree(), 3);
            for t in [0., 0.17, 0.5, 0.83, 1.] {
                let uv_fraction = 2. * t / (1. + t);
                let expected = basis.evaluate(0.2 + 0.6 * uv_fraction, 0.25);
                let actual = curve.evaluate(t).unwrap();
                assert!((actual.x() - expected.x).abs() < 1e-11);
                assert!((actual.y() - expected.y).abs() < 1e-11);
                assert!((actual.z() - expected.z).abs() < 1e-11);
            }

            let diagonal = Curve3D::ParameterCurve(StepParameterCurve::new(
                Box::new(Curve2D::Line(Line(
                    TruckPoint2::new(0., 0.),
                    TruckPoint2::new(1., 1.),
                ))),
                Box::new(basis.clone()),
            ));
            let curve = edge_curve(&diagonal, 1).unwrap();
            assert_eq!(curve.degree(), 3);
            for t in [0., 0.17, 0.5, 0.83, 1.] {
                let actual = curve.evaluate(t).unwrap();
                let expected = basis.evaluate(t, t);
                assert!((actual.x() - expected.x).abs() < 1e-11);
                assert!((actual.y() - expected.y).abs() < 1e-11);
                assert!((actual.z() - expected.z).abs() < 1e-11);
            }
        }
    }

    #[test]
    fn curved_extrusion_pcurve_edges_preserve_partial_and_reversed_iso_segments() {
        for directrix in [
            Curve3D::BsplineCurve(BsplineCurve::new(
                KnotVector::bezier_knot(2),
                vec![
                    TruckPoint3::new(0., 0., 0.),
                    TruckPoint3::new(1., 0., 1.),
                    TruckPoint3::new(2., 0., 0.),
                ],
            )),
            Curve3D::NurbsCurve(TruckNurbsCurve::new(BsplineCurve::new(
                KnotVector::bezier_knot(2),
                vec![
                    Vector4::new(0., 0., 0., 1.),
                    Vector4::new(0.5, 0., 0.5, 0.5),
                    Vector4::new(2., 0., 0., 1.),
                ],
            ))),
        ] {
            let basis = Surface::SweepSurface(SweepSurface::ExtrusionSurface(
                StepExtrusionSurface::by_extrusion(directrix, Vector3::new(0., 3., 0.)),
            ));
            for (a, b, degree) in [
                ([0.2, 0.4], [0.8, 0.4], 2),
                ([0.8, 0.4], [0.2, 0.4], 2),
                ([0.25, 0.2], [0.25, 0.9], 1),
                ([0.25, 0.9], [0.25, 0.2], 1),
            ] {
                let uv0 = TruckPoint2::new(a[0], a[1]);
                let uv1 = TruckPoint2::new(b[0], b[1]);
                for (uv, domain) in [
                    (Curve2D::Line(Line(uv0, uv1)), 0.0..=1.0),
                    (Curve2D::Polyline(PolylineCurve(vec![uv0, uv1])), 0.0..=1.0),
                    (
                        Curve2D::BsplineCurve(BsplineCurve::new(
                            KnotVector::from(vec![5., 5., 9., 9.]),
                            vec![uv0, uv1],
                        )),
                        5.0..=9.0,
                    ),
                    (
                        Curve2D::NurbsCurve(TruckNurbsCurve::new(BsplineCurve::new(
                            KnotVector::from(vec![5., 5., 9., 9.]),
                            vec![
                                Vector3::new(2. * a[0], 2. * a[1], 2.),
                                Vector3::new(2. * b[0], 2. * b[1], 2.),
                            ],
                        ))),
                        5.0..=9.0,
                    ),
                ] {
                    let source = Curve3D::ParameterCurve(StepParameterCurve::new(
                        Box::new(uv),
                        Box::new(basis.clone()),
                    ));
                    let curve = edge_curve(&source, 1).unwrap();
                    assert_eq!(curve.degree(), degree);
                    assert_eq!(curve.domain(), domain);
                    for fraction in [0., 0.17, 0.5, 0.83, 1.] {
                        let u = a[0] * (1. - fraction) + b[0] * fraction;
                        let v = a[1] * (1. - fraction) + b[1] * fraction;
                        let t = *domain.start() * (1. - fraction) + *domain.end() * fraction;
                        let expected = basis.evaluate(u, v);
                        let actual = curve.evaluate(t).unwrap();
                        assert!((actual.x() - expected.x).abs() < 1e-11);
                        assert!((actual.y() - expected.y).abs() < 1e-11);
                        assert!((actual.z() - expected.z).abs() < 1e-11);
                    }
                }
            }

            let diagonal = Curve3D::ParameterCurve(StepParameterCurve::new(
                Box::new(Curve2D::Line(Line(
                    TruckPoint2::new(0., 0.),
                    TruckPoint2::new(1., 1.),
                ))),
                Box::new(basis.clone()),
            ));
            assert!(matches!(
                edge_curve(&diagonal, 1),
                Err(StepError::UnsupportedNativeShell {
                    reason: "3D edge p-curve basis is not affine",
                    ..
                })
            ));

            let outside = Curve3D::ParameterCurve(StepParameterCurve::new(
                Box::new(Curve2D::Line(Line(
                    TruckPoint2::new(1.2, 0.),
                    TruckPoint2::new(1.2, 1.),
                ))),
                Box::new(basis.clone()),
            ));
            assert!(matches!(
                edge_curve(&outside, 1),
                Err(StepError::UnsupportedNativeShell {
                    reason: "3D edge p-curve leaves its extrusion surface domain",
                    ..
                })
            ));

            let unequal_weight_iso = Curve3D::ParameterCurve(StepParameterCurve::new(
                Box::new(Curve2D::NurbsCurve(TruckNurbsCurve::new(
                    BsplineCurve::new(
                        KnotVector::bezier_knot(1),
                        vec![Vector3::new(0.2, 0.4, 1.), Vector3::new(1.6, 0.8, 2.)],
                    ),
                ))),
                Box::new(basis),
            ));
            assert!(matches!(
                edge_curve(&unequal_weight_iso, 1),
                Err(StepError::UnsupportedNativeShell {
                    reason: "3D edge p-curve basis is not affine",
                    ..
                })
            ));
        }
    }

    #[test]
    fn conic_extrusion_axial_pcurve_uses_converted_directrix_parameterization() {
        let directrix: Curve3D = builder::circle_arc(
            &Vertex::new(TruckPoint3::new(0., 0., 0.)),
            &Vertex::new(TruckPoint3::new(2., 0., 0.)),
            TruckPoint3::new(1., 0., 1.),
        )
        .curve();
        let (start, end) = directrix.range_tuple();
        let u = start + 0.31 * (end - start);
        let converted = sweep_directrix(&directrix, 1).unwrap();
        let base = converted.evaluate(u).unwrap();
        let basis = Surface::SweepSurface(SweepSurface::ExtrusionSurface(
            StepExtrusionSurface::by_extrusion(directrix, Vector3::new(0., 3., 0.)),
        ));
        let source_base = basis.evaluate(u, 0.);
        assert!((base.x() - source_base.x).hypot(base.z() - source_base.z) > 1e-4);
        let source = Curve3D::ParameterCurve(StepParameterCurve::new(
            Box::new(Curve2D::Line(Line(
                TruckPoint2::new(u, 0.2),
                TruckPoint2::new(u, 0.9),
            ))),
            Box::new(basis),
        ));
        let edge = edge_curve(&source, 1).unwrap();
        for (t, v) in [(0., 0.2), (0.5, 0.55), (1., 0.9)] {
            let point = edge.evaluate(t).unwrap();
            assert!((point.x() - base.x()).abs() < 1e-12);
            assert!((point.y() - base.y() - 3. * v).abs() < 1e-12);
            assert!((point.z() - base.z()).abs() < 1e-12);
        }
    }

    #[test]
    fn revolved_pcurve_edges_preserve_exact_meridians_and_circular_locus() {
        for profile in [
            Curve3D::BsplineCurve(BsplineCurve::new(
                KnotVector::bezier_knot(2),
                vec![
                    TruckPoint3::new(2., 0., -1.),
                    TruckPoint3::new(3., 0., 0.),
                    TruckPoint3::new(2., 0., 1.),
                ],
            )),
            Curve3D::NurbsCurve(TruckNurbsCurve::new(BsplineCurve::new(
                KnotVector::bezier_knot(2),
                vec![
                    Vector4::new(2., 0., -1., 1.),
                    Vector4::new(1.5, 0., 0., 0.5),
                    Vector4::new(2., 0., 1., 1.),
                ],
            ))),
        ] {
            let mut revolution = Processor::new(RevolutionSurface::by_revolution(
                profile,
                TruckPoint3::new(0., 0., 0.),
                Vector3::new(0., 0., 1.),
            ));
            revolution.invert();
            let basis = Surface::SweepSurface(SweepSurface::RevolutionSurface(revolution));
            for (a, b, meridian) in [
                ([0.4, 0.2], [0.4, 0.8], true),
                ([0.4, 0.8], [0.4, 0.2], true),
                ([0.2, 0.35], [1.4, 0.35], false),
                ([1.4, 0.35], [0.2, 0.35], false),
            ] {
                let uv0 = TruckPoint2::new(a[0], a[1]);
                let uv1 = TruckPoint2::new(b[0], b[1]);
                for (uv, domain) in [
                    (Curve2D::Line(Line(uv0, uv1)), 0.0..=1.0),
                    (
                        Curve2D::BsplineCurve(BsplineCurve::new(
                            KnotVector::from(vec![5., 5., 9., 9.]),
                            vec![uv0, uv1],
                        )),
                        5.0..=9.0,
                    ),
                ] {
                    let source = Curve3D::ParameterCurve(StepParameterCurve::new(
                        Box::new(uv),
                        Box::new(basis.clone()),
                    ));
                    let curve = edge_curve(&source, 1).unwrap();
                    assert_eq!(curve.degree(), 2);
                    assert_eq!(curve.domain(), domain);
                    for fraction in [0., 0.17, 0.5, 0.83, 1.] {
                        let t = *domain.start() * (1. - fraction) + *domain.end() * fraction;
                        let point = curve.evaluate(t).unwrap();
                        let u = a[0] * (1. - fraction) + b[0] * fraction;
                        let v = a[1] * (1. - fraction) + b[1] * fraction;
                        let expected = basis.evaluate(u, v);
                        if meridian || fraction == 0. || fraction == 1. {
                            assert!((point.x() - expected.x).abs() < 1e-11);
                            assert!((point.y() - expected.y).abs() < 1e-11);
                            assert!((point.z() - expected.z).abs() < 1e-11);
                        } else {
                            let reference = basis.evaluate(a[0], a[1]);
                            assert!(
                                (point.x().hypot(point.y()) - reference.x.hypot(reference.y)).abs()
                                    < 1e-11
                            );
                            assert!((point.z() - reference.z).abs() < 1e-11);
                            let angle = point.y().atan2(point.x());
                            assert!(angle >= a[0].min(b[0]) - 1e-11);
                            assert!(angle <= a[0].max(b[0]) + 1e-11);
                        }
                    }
                }
            }

            let full_circle = Curve3D::ParameterCurve(StepParameterCurve::new(
                Box::new(Curve2D::Line(Line(
                    TruckPoint2::new(0., 0.35),
                    TruckPoint2::new(std::f64::consts::TAU, 0.35),
                ))),
                Box::new(basis.clone()),
            ));
            let full_circle = edge_curve(&full_circle, 1).unwrap();
            assert_eq!(full_circle.degree(), 2);
            assert_eq!(full_circle.control_points().len(), 9);
            assert!(
                full_circle
                    .evaluate(0.)
                    .unwrap()
                    .distance_to(full_circle.evaluate(1.).unwrap())
                    .unwrap()
                    < 1e-11
            );

            let collapsed = Curve3D::ParameterCurve(StepParameterCurve::new(
                Box::new(Curve2D::Line(Line(
                    TruckPoint2::new(0., 0.35),
                    TruckPoint2::new(0., 0.35),
                ))),
                Box::new(basis.clone()),
            ));
            let associated = StepParameterCurve::new(
                Box::new(Curve2D::Line(Line(
                    TruckPoint2::new(0., 0.35),
                    TruckPoint2::new(std::f64::consts::TAU, 0.35),
                ))),
                Box::new(basis.clone()),
            );
            let wrapper = |geometry| {
                SurfaceCurve3D::new(
                    SurfaceCurveKind::SurfaceCurve,
                    Box::new(collapsed.clone()),
                    geometry,
                    SurfaceCurveRepresentation::Curve3D,
                )
            };
            assert!(
                collapsed_surface_curve_pcurve(&wrapper(vec![
                    SurfaceCurveAssociatedGeometry::ParameterCurve(associated.clone())
                ]))
                .is_some()
            );
            assert!(
                collapsed_surface_curve_pcurve(&wrapper(vec![
                    SurfaceCurveAssociatedGeometry::ParameterCurve(associated.clone()),
                    SurfaceCurveAssociatedGeometry::ParameterCurve(associated),
                ]))
                .is_none()
            );
            let wrong_endpoint = StepParameterCurve::new(
                Box::new(Curve2D::Line(Line(
                    TruckPoint2::new(0.2, 0.35),
                    TruckPoint2::new(1.4, 0.35),
                ))),
                Box::new(basis.clone()),
            );
            assert!(
                collapsed_surface_curve_pcurve(&wrapper(vec![
                    SurfaceCurveAssociatedGeometry::ParameterCurve(wrong_endpoint)
                ]))
                .is_none()
            );

            let outside = Curve3D::ParameterCurve(StepParameterCurve::new(
                Box::new(Curve2D::Line(Line(
                    TruckPoint2::new(0.2, 1.2),
                    TruckPoint2::new(1.4, 1.2),
                ))),
                Box::new(basis),
            ));
            assert!(matches!(
                edge_curve(&outside, 1),
                Err(StepError::UnsupportedNativeShell {
                    reason: "3D edge p-curve leaves its revolution surface domain",
                    ..
                })
            ));
        }
    }

    #[test]
    fn elementary_surface_pcurve_edges_preserve_iso_geometry() {
        let revolved_line = |end| {
            let mut revolution = Processor::new(RevolutionSurface::by_revolution(
                Line(TruckPoint3::new(2., 0., 0.), end),
                TruckPoint3::new(0., 0., 0.),
                Vector3::new(0., 0., 1.),
            ));
            revolution.invert();
            revolution
        };
        for (basis, straight_v) in [
            (
                Surface::ElementarySurface(ElementarySurface::CylindricalSurface(revolved_line(
                    TruckPoint3::new(2., 0., 3.),
                ))),
                true,
            ),
            (
                Surface::ElementarySurface(ElementarySurface::ConicalSurface(revolved_line(
                    TruckPoint3::new(3., 0., 3.),
                ))),
                true,
            ),
            (
                Surface::ElementarySurface(ElementarySurface::Sphere(Processor::new(StepSphere(
                    TruckSphere::new(TruckPoint3::new(0., 0., 0.), 2.),
                )))),
                false,
            ),
            (
                Surface::ElementarySurface(ElementarySurface::ToroidalSurface(Processor::new(
                    Torus::new(TruckPoint3::new(0., 0., 0.), 3., 1.),
                ))),
                false,
            ),
        ] {
            for (a, b, varying_u) in [
                ([0.2, 0.25], [1.4, 0.25], true),
                ([1.4, 0.25], [0.2, 0.25], true),
                ([0.4, 0.2], [0.4, 0.8], false),
                ([0.4, 0.8], [0.4, 0.2], false),
            ] {
                let uv0 = TruckPoint2::new(a[0], a[1]);
                let uv1 = TruckPoint2::new(b[0], b[1]);
                for (uv, domain) in [
                    (Curve2D::Line(Line(uv0, uv1)), 0.0..=1.0),
                    (
                        Curve2D::BsplineCurve(BsplineCurve::new(
                            KnotVector::from(vec![5., 5., 9., 9.]),
                            vec![uv0, uv1],
                        )),
                        5.0..=9.0,
                    ),
                ] {
                    let source = Curve3D::ParameterCurve(StepParameterCurve::new(
                        Box::new(uv),
                        Box::new(basis.clone()),
                    ));
                    let curve = edge_curve(&source, 1).unwrap();
                    assert_eq!(curve.degree(), if !varying_u && straight_v { 1 } else { 2 });
                    assert_eq!(curve.domain(), domain);
                    for fraction in [0., 0.17, 0.5, 0.83, 1.] {
                        let t = *domain.start() * (1. - fraction) + *domain.end() * fraction;
                        let point = curve.evaluate(t).unwrap();
                        let u = a[0] * (1. - fraction) + b[0] * fraction;
                        let v = a[1] * (1. - fraction) + b[1] * fraction;
                        let expected = basis.evaluate(u, v);
                        if (!varying_u && straight_v)
                            || fraction == 0.
                            || fraction == 0.5
                            || fraction == 1.
                        {
                            assert!((point.x() - expected.x).abs() < 1e-11);
                            assert!((point.y() - expected.y).abs() < 1e-11);
                            assert!((point.z() - expected.z).abs() < 1e-11);
                        } else if varying_u {
                            let reference = basis.evaluate(a[0], a[1]);
                            assert!(
                                (point.x().hypot(point.y()) - reference.x.hypot(reference.y)).abs()
                                    < 1e-11
                            );
                            assert!((point.z() - reference.z).abs() < 1e-11);
                        } else if matches!(
                            basis,
                            Surface::ElementarySurface(ElementarySurface::Sphere(_))
                        ) {
                            assert!(
                                (point.x().hypot(point.y()).hypot(point.z()) - 2.).abs() < 1e-11
                            );
                        } else {
                            let radius = point.x().hypot(point.y()) - 3.;
                            assert!((radius * radius + point.z() * point.z() - 1.).abs() < 1e-11);
                        }
                    }
                }
            }
            let diagonal = Curve3D::ParameterCurve(StepParameterCurve::new(
                Box::new(Curve2D::Line(Line(
                    TruckPoint2::new(0.2, 0.2),
                    TruckPoint2::new(1.4, 0.8),
                ))),
                Box::new(basis.clone()),
            ));
            assert!(matches!(
                edge_curve(&diagonal, 1),
                Err(StepError::UnsupportedNativeShell {
                    reason: "3D edge p-curve basis is not affine",
                    ..
                })
            ));
            if matches!(
                basis,
                Surface::ElementarySurface(ElementarySurface::Sphere(_))
            ) {
                let pole = Curve3D::ParameterCurve(StepParameterCurve::new(
                    Box::new(Curve2D::Line(Line(
                        TruckPoint2::new(0.2, std::f64::consts::FRAC_PI_2),
                        TruckPoint2::new(1.4, std::f64::consts::FRAC_PI_2),
                    ))),
                    Box::new(basis),
                ));
                assert!(matches!(
                    edge_curve(&pole, 1),
                    Err(StepError::UnsupportedNativeShell {
                        reason: "sphere pole requires singular trim support",
                        ..
                    })
                ));
            }
        }
    }
}
