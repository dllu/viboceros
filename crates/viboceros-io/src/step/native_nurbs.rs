//! Native conversion of STEP NURBS shell geometry and face-local p-curves.
use super::{StepError, Table, native_planar, reported_trimmed_shell};
use monstertruck::meshing::prelude::{BoundedCurve, ParametricCurve, ParametricSurface};
use monstertruck::step::load::step_geometry::{
    Conic2D, Conic3D, Curve2D, Curve3D, ElementarySurface, Surface,
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
        let periodic_revolution = matches!(
            face.surface,
            Surface::ElementarySurface(
                ElementarySurface::CylindricalSurface(_) | ElementarySurface::ConicalSurface(_)
            )
        );
        for boundary in &face.boundaries {
            let mut trims = Vec::with_capacity(boundary.len());
            let mut previous_end = None;
            for use_ in boundary {
                let source = use_
                    .trim_curve
                    .as_ref()
                    .ok_or_else(|| unsupported("missing UV trim"))?;
                let mut curve = trim_curve(source.curve().as_ref(), id)?;
                if periodic_revolution {
                    if let Some(end) = previous_end {
                        curve = align_revolution_trim(curve, end, tolerance)?;
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
        let native =
            if boundaries.len() == 1 {
                BrepFace::try_new(
                    surface,
                    !face.orientation,
                    vec![BrepLoop::try_new(
                        BrepLoopType::Outer,
                        boundaries.remove(0),
                    )?],
                )?
            } else {
                if boundaries.iter().flatten().any(|trim| {
                    trim.curve().degree() != 1 || trim.curve().control_points().len() != 2
                }) {
                    return Err(unsupported("multiple loops require straight UV boundaries"));
                }
                BrepFace::try_from_polygon_boundaries(surface, !face.orientation, boundaries)?
            };
        faces.push(native);
    }
    Ok(Brep::try_new(vertices, edges, faces, tolerance)?)
}

/// STEP readers may unwrap a circular 3D edge into a different full-turn UV
/// interval from its explicit seam p-curves. Keep each p-curve's shape and
/// align its periodic coordinate with the preceding trim in the loop.
fn align_revolution_trim(
    curve: NurbsCurve2,
    previous_end: Point2,
    tolerance: Tolerance,
) -> Result<NurbsCurve2, StepError> {
    let start = curve.start_point()?;
    if (previous_end.y() - start.y()).abs() > tolerance.absolute() {
        return Ok(curve);
    }
    let turns = ((previous_end.x() - start.x()) / std::f64::consts::TAU).round();
    let offset = turns * std::f64::consts::TAU;
    if !offset.is_finite()
        || offset == 0.
        || (previous_end.x() - start.x() - offset).abs() > tolerance.angular()
    {
        return Ok(curve);
    }
    let controls = curve
        .control_points()
        .iter()
        .map(|control| {
            WeightedPoint2::try_new(
                Point2::try_new(control.point().x() + offset, control.point().y())?,
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
        Curve3D::Line(_) | Curve3D::Polyline(_) => native_planar::curves::linear_edge(curve, id)
            .map_err(|error| match error {
                StepError::UnsupportedPlanarShell { reason, .. } => unsupported(reason),
                other => other,
            }),
        Curve3D::Conic(Conic3D::Ellipse(_)) => conic_edge(curve, id),
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
        _ => Err(unsupported("3D edge curve is not a supported B-spline")),
    }
}

fn trim_curve(curve: &Curve2D, id: u64) -> Result<NurbsCurve2, StepError> {
    let unsupported = |reason| StepError::UnsupportedNativeShell { shell: id, reason };
    match curve {
        Curve2D::Line(_) | Curve2D::Polyline(_) => native_planar::curves::linear_trim(curve, id)
            .map_err(|error| match error {
                StepError::UnsupportedPlanarShell { reason, .. } => unsupported(reason),
                other => other,
            }),
        Curve2D::Conic(Conic2D::Ellipse(_)) => conic_trim(curve, id),
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
        _ => Err(unsupported("UV trim is not a supported B-spline")),
    }
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

fn conic_edge(curve: &Curve3D, id: u64) -> Result<NurbsCurve, StepError> {
    let (start, end) = curve.range_tuple();
    let spans = arc_span_count(end - start, id)?;
    let step = (end - start) / spans as f64;
    let mut controls = Vec::with_capacity(2 * spans + 1);
    for index in 0..spans {
        let t0 = start + step * index as f64;
        let t1 = if index + 1 == spans { end } else { t0 + step };
        let p0 = curve.evaluate(t0);
        let pm = curve.evaluate((t0 + t1) / 2.);
        let p1 = curve.evaluate(t1);
        let weight = ((t1 - t0) / 2.).cos();
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

fn conic_trim(curve: &Curve2D, id: u64) -> Result<NurbsCurve2, StepError> {
    let (start, end) = curve.range_tuple();
    let spans = arc_span_count(end - start, id)?;
    let step = (end - start) / spans as f64;
    let mut controls = Vec::with_capacity(2 * spans + 1);
    for index in 0..spans {
        let t0 = start + step * index as f64;
        let t1 = if index + 1 == spans { end } else { t0 + step };
        let p0 = curve.evaluate(t0);
        let pm = curve.evaluate((t0 + t1) / 2.);
        let p1 = curve.evaluate(t1);
        let weight = ((t1 - t0) / 2.).cos();
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
        Surface::ElementarySurface(
            ElementarySurface::CylindricalSurface(revolution)
            | ElementarySurface::ConicalSurface(revolution),
        ) => {
            let mut min = [f64::INFINITY; 2];
            let mut max = [f64::NEG_INFINITY; 2];
            for trim in boundaries.iter().flatten() {
                if trim.curve().degree() != 1 || trim.curve().control_points().len() != 2 {
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
        _ => Err(unsupported(
            "surface is not a supported plane, revolved line, or B-spline",
        )),
    }
}
