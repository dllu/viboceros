//! Restore face-local parameters on validated trimmed intersection geometry.
use super::*;
use viboceros_geometry::{Brep, BrepLoopType, CurveCurveIntersectionEvent};
pub(super) fn cut(
    brep: &Brep,
    frame: Frame3,
    tolerance: Tolerance,
) -> Result<Vec<Geometry>, CommandError> {
    let source = Geometry::Brep(brep.clone());
    let [face] = brep.faces() else {
        let result = section::section_geometry(&source, frame, 1., true, tolerance)?;
        return canonicalize(brep, result, frame, tolerance);
    };
    let carrier = Geometry::NurbsSurface(face.surface().clone());
    let mut curves = geometry::cut(&carrier, frame, tolerance)?;
    if face.is_reversed() {
        for curve in &mut curves {
            if let Some(c) = curve.curve_ref() {
                *curve = Geometry::from(c.to_owned().reversed(tolerance)?);
            }
        }
    }
    if face.surface().plane(tolerance)?.is_some_and(|plane| {
        plane
            .normal()
            .as_vector()
            .cross(frame.z_axis().as_vector())
            .is_ok_and(|v| v.length().is_ok_and(|v| v <= tolerance.angular()))
            && plane
                .signed_distance_to(frame.origin())
                .is_ok_and(|v| v.abs() <= tolerance.absolute())
    }) {
        return coplanar_face(brep, frame, tolerance);
    }
    if face.is_untrimmed(tolerance)? {
        return Ok(curves);
    }
    let mut output = section::section_geometry(&source, frame, 1., true, tolerance)?;
    let carriers = curves
        .iter()
        .filter_map(|g| g.nurbs_curve_representation().transpose())
        .collect::<Result<Vec<_>, _>>()?;
    let mut owners = Vec::new();
    for result in &output {
        let mut owner = None;
        if let Some(curve) = result.nurbs_curve_representation()?
            && geometry::straight_segment(&curve, tolerance)
        {
            let domain = curve.domain();
            let start = curve.evaluate(*domain.start())?;
            let end = curve.evaluate(*domain.end())?;
            for (index, carrier) in carriers.iter().enumerate() {
                if !geometry::straight_segment(carrier, tolerance) {
                    continue;
                }
                let a = carrier.closest_parameter(start, tolerance)?;
                let b = carrier.closest_parameter(end, tolerance)?;
                if carrier.evaluate(a)?.distance_to(start)? <= tolerance.absolute()
                    && carrier.evaluate(b)?.distance_to(end)? <= tolerance.absolute()
                {
                    owner = Some((index, a, b));
                    break;
                }
            }
        }
        owners.push(owner);
    }
    for (index, owner) in owners.iter().enumerate() {
        let Some((carrier, a, b)) = *owner else {
            continue;
        };
        let curve = output[index].nurbs_curve_representation()?.unwrap();
        let domain = curve.domain();
        let mut start = curve.evaluate(*domain.start())?;
        let mut end = curve.evaluate(*domain.end())?;
        let (a, b) = if a > b {
            std::mem::swap(&mut start, &mut end);
            (b, a)
        } else {
            (a, b)
        };
        let split = owners
            .iter()
            .flatten()
            .filter(|(candidate, _, _)| *candidate == carrier)
            .count()
            > 1;
        let hole_overlap = has_inner_overlap(&carriers[carrier], brep, tolerance)?;
        let domain = if split || hole_overlap {
            0. ..=start.distance_to(end)?
        } else {
            a..=b
        };
        output[index] = Geometry::NurbsCurve(NurbsCurve::try_new(
            1,
            vec![start, end],
            vec![
                *domain.start(),
                *domain.start(),
                *domain.end(),
                *domain.end(),
            ],
        )?);
    }
    if let Some(Some((carrier, _, _))) = owners.first()
        && owners
            .iter()
            .all(|owner| owner.is_some_and(|(index, _, _)| index == *carrier))
    {
        let mut order = (0..output.len()).collect::<Vec<_>>();
        order.sort_by(|a, b| {
            owners[*b]
                .unwrap()
                .1
                .max(owners[*b].unwrap().2)
                .total_cmp(&owners[*a].unwrap().1.max(owners[*a].unwrap().2))
        });
        output = order.into_iter().map(|i| output[i].clone()).collect();
    }
    Ok(output)
}

fn has_inner_overlap(
    curve: &NurbsCurve,
    brep: &Brep,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    for face in brep.faces() {
        for boundary in face
            .loops()
            .iter()
            .filter(|l| l.loop_type() == BrepLoopType::Inner)
        {
            for trim in boundary.trims() {
                if let Some(edge) = trim.edge() {
                    for event in curve
                        .intersection_events_with_curve(brep.edges()[edge].curve(), tolerance)?
                    {
                        if let CurveCurveIntersectionEvent::Overlap(span) = event {
                            let domain = span.first_interval();
                            if curve
                                .evaluate(*domain.start())?
                                .distance_to(curve.evaluate(*domain.end())?)?
                                > tolerance.absolute()
                            {
                                return Ok(true);
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(false)
}
fn linear_vertices(geometry: &Geometry) -> Result<Option<Vec<Point3>>, GeometryError> {
    let Some(curve) = geometry.nurbs_curve_representation()? else {
        return Ok(None);
    };
    let controls = curve.control_points();
    if curve.degree() != 1
        || controls
            .iter()
            .any(|p| p.weight().is_sign_positive() != controls[0].weight().is_sign_positive())
    {
        return Ok(None);
    }
    Ok(Some(controls.iter().map(|p| p.point()).collect()))
}
fn orient_and_rotate(
    mut vertices: Vec<Point3>,
    seam: Option<Point3>,
    positive: bool,
    frame: Frame3,
    tolerance: Tolerance,
) -> Result<Geometry, GeometryError> {
    if vertices.len() < 4 || !vertices[0].is_near(*vertices.last().unwrap(), tolerance) {
        return Err(GeometryError::Degenerate {
            context: "closed Contour polyline",
        });
    }
    vertices.pop();
    let xy = vertices
        .iter()
        .map(|p| {
            [
                projected(*p, frame.origin(), frame.x_axis().as_vector()),
                projected(*p, frame.origin(), frame.y_axis().as_vector()),
            ]
        })
        .collect::<Vec<_>>();
    let mut area = BigRational::zero();
    for i in 0..xy.len() {
        let j = (i + 1) % xy.len();
        area += &xy[i][0] * &xy[j][1] - &xy[i][1] * &xy[j][0];
    }
    if (area > BigRational::zero()) != positive {
        vertices.reverse();
    }
    if let Some(seam) = seam
        && let Some(index) = vertices.iter().position(|p| p.is_near(seam, tolerance))
    {
        vertices.rotate_left(index);
    }
    vertices.push(vertices[0]);
    Ok(Geometry::Polyline(
        Polyline3::try_new(vertices, tolerance)?.try_chord_length_parameterized()?,
    ))
}
fn coplanar_face(
    brep: &Brep,
    frame: Frame3,
    tolerance: Tolerance,
) -> Result<Vec<Geometry>, CommandError> {
    let face = &brep.faces()[0];
    let mut output = Vec::new();
    for (index, boundary) in face.loops().iter().enumerate().rev() {
        for component in brep.loop_boundary_curve_components(0, index)? {
            let geometry = border::assemble(component, tolerance)?;
            let Some(vertices) = linear_vertices(&geometry)? else {
                output.push(geometry);
                continue;
            };
            let mut seam = None;
            if let Some(trim) = boundary.trims().last()
                && let Some(edge) = trim.edge()
            {
                let mut curve = brep.edges()[edge].curve().clone();
                if trim.is_reversed_3d() {
                    curve = curve.reversed()?;
                }
                if curve.degree() == 1 && curve.is_closed()? {
                    seam = Some(curve.control_points()[curve.control_points().len() - 2].point());
                } else {
                    seam = Some(curve.evaluate(*curve.domain().start())?);
                }
            }
            output.push(orient_and_rotate(
                vertices,
                seam,
                (boundary.loop_type() == BrepLoopType::Outer) != face.is_reversed(),
                frame,
                tolerance,
            )?);
        }
    }
    Ok(output)
}
fn canonicalize(
    brep: &Brep,
    mut output: Vec<Geometry>,
    frame: Frame3,
    tolerance: Tolerance,
) -> Result<Vec<Geometry>, CommandError> {
    let normal = frame.z_axis().as_vector();
    let reverse = normal
        .to_array()
        .into_iter()
        .find(|v| *v != 0.)
        .is_some_and(|v| v < 0.);
    let seam_frame = if reverse {
        Frame3::try_from_normal(frame.origin(), normal.scaled(-1.)?, tolerance)?
    } else {
        frame
    };
    for geometry in &mut output {
        if !geometry
            .curve_ref()
            .is_some_and(|c| c.is_closed().unwrap_or(false))
        {
            continue;
        }
        let Some(vertices) = linear_vertices(geometry)? else {
            continue;
        };
        let mut seam = None;
        for face in brep.faces().iter().rev() {
            let surface = face.surface();
            let normal =
                surface.normal_at(surface.parameter_at_u(0.5)?, surface.parameter_at_v(0.5)?)?;
            let facing = normal.as_vector().dot(seam_frame.z_axis().as_vector())?;
            let carrier = Geometry::NurbsSurface(surface.clone());
            for curve in geometry::cut(&carrier, seam_frame, tolerance)? {
                if let Some(curve) = curve.curve_ref() {
                    let domain = curve.domain();
                    let parameter = if facing < 0. {
                        if curve.is_closed()? {
                            (*domain.start()).midpoint(*domain.end())
                        } else {
                            *domain.end()
                        }
                    } else {
                        *domain.start()
                    };
                    let point = curve.evaluate(parameter)?;
                    if vertices.iter().any(|p| p.is_near(point, tolerance)) {
                        seam = Some(point);
                        break;
                    }
                }
            }
            if seam.is_some() {
                break;
            }
        }
        *geometry = orient_and_rotate(vertices, seam, true, frame, tolerance)?;
    }
    Ok(output)
}
