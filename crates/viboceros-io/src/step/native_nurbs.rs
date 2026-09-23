//! Native conversion of STEP NURBS shell geometry and face-local p-curves.
use super::{StepError, Table, native_planar, reported_trimmed_shell};
use monstertruck::core::cgmath64::{InnerSpace, Transform as _};
use monstertruck::meshing::prelude::{BoundedCurve, ParametricCurve, ParametricSurface};
use monstertruck::step::load::step_geometry::{
    Conic2D, Conic3D, Curve2D, Curve3D, ElementarySurface, Surface, SweepSurface,
};
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
        Curve3D::SurfaceCurve(surface_curve) => edge_curve(surface_curve.leader(), id),
        Curve3D::IntersectionCurve(intersection_curve) => {
            edge_curve(intersection_curve.leader(), id)
        }
        Curve3D::ParameterCurve(parameter_curve) => {
            let basis = parameter_curve.surface().as_ref();
            if !matches!(
                basis,
                Surface::ElementarySurface(ElementarySurface::Plane(_))
            ) && !matches!(
                basis,
                Surface::SweepSurface(SweepSurface::ExtrusionSurface(extrusion))
                    if matches!(extrusion.entity_curve(), Curve3D::Line(_))
            ) {
                return Err(unsupported("3D edge p-curve basis is not affine"));
            }
            let uv = trim_curve(parameter_curve.curve().as_ref(), id)?;
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

fn surface(
    source: &Surface,
    boundaries: &[Vec<BrepTrim>],
    id: u64,
    tolerance: Tolerance,
) -> Result<NurbsSurface, StepError> {
    let unsupported = |reason| StepError::UnsupportedNativeShell { shell: id, reason };
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
            let origin = revolution.entity().origin();
            let axis = revolution.entity().axis();
            let rotate = |point: Point3, angle: f64| -> Result<Point3, StepError> {
                let point = monstertruck::modeling::Point3::new(point.x(), point.y(), point.z());
                let displacement = point - origin;
                let rotated = origin
                    + displacement * angle.cos()
                    + axis.cross(displacement) * angle.sin()
                    + axis * axis.dot(displacement) * (1. - angle.cos());
                point3(revolution.transform().transform_point(rotated))
            };
            let mut controls =
                Vec::with_capacity((2 * spans + 1) * directrix.control_points().len());
            for control in directrix.control_points() {
                for index in 0..spans {
                    let start = u0 + step * index as f64;
                    let end = if index + 1 == spans { u1 } else { start + step };
                    let p0 = rotate(control.point(), start)?;
                    let pm = rotate(control.point(), (start + end) / 2.)?;
                    let p1 = rotate(control.point(), end)?;
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
        BsplineCurve, KnotVector, Line, NurbsCurve as TruckNurbsCurve, Plane, Point3 as TruckPoint3,
    };
    use monstertruck::step::load::step_geometry::{StepExtrusionSurface, StepParameterCurve};

    #[test]
    fn planar_pcurve_edge_lifts_rational_controls_without_losing_weights() {
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
}
