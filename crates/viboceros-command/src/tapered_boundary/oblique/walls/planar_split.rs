//! Classify the actual cutter trace on independent straight draft walls.
use super::*;
use crate::curve_intersections::brep_events;

#[derive(Clone, Copy)]
enum Cut {
    Split {
        parameters: [f64; 2],
        section: [Point3; 2],
    },
    Unsplit([Option<Point3>; 2]),
}

pub(super) fn retain(
    stock: &Brep,
    cutter: &Brep,
    plane: Plane,
    displacement: Vector3,
    closed: bool,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let alignment = displacement.dot(plane.normal().as_vector())?;
    let trace = |index: usize| {
        wall_cut(
            stock.faces()[index].surface(),
            cutter,
            plane,
            alignment.signum(),
            tolerance,
        )
    };
    let first = trace(0)?;
    let last = stock.faces().len() - 1;
    // A forward-facing planar cutter that divides the leading wall of a
    // closed sweep can leave its unsplit closing wall as the first piece.
    let (index, cut) = if closed
        && matches!(first, Cut::Split { .. })
        && (alignment > 0.) != cutter.faces()[0].is_reversed()
        && let Cut::Unsplit(points) = trace(last)?
    {
        (last, Cut::Unsplit(points))
    } else {
        (0, first)
    };
    let wall = stock.sub_brep(&[index], tolerance)?;
    match cut {
        Cut::Split {
            parameters,
            section,
        } => trim(&wall, parameters, section, tolerance),
        Cut::Unsplit(points) if last > 0 => imprint(wall, points, tolerance),
        Cut::Unsplit(_) => Err(REJECTED),
    }
}

fn wall_cut(
    wall: &NurbsSurface,
    cutter: &Brep,
    plane: Plane,
    sign: f64,
    tolerance: Tolerance,
) -> Result<Cut, CommandError> {
    let u = wall.domain_u();
    let v = wall.domain_v();
    let mut section = [wall.evaluate(*u.start(), *v.start())?; 2];
    let mut parameters = [0.; 2];
    for (i, v) in [*v.start(), *v.end()].into_iter().enumerate() {
        let start = wall.evaluate(*u.start(), v)?;
        let end = wall.evaluate(*u.end(), v)?;
        let a = plane.signed_distance_to(start)? * sign;
        let b = plane.signed_distance_to(end)? * sign;
        if a >= -tolerance.absolute() || b <= tolerance.absolute() {
            return Err(REJECTED);
        }
        parameters[i] = wall.parameter_at_u(a / (a - b))?;
        section[i] = wall.evaluate(parameters[i], v)?;
    }
    let trace = LineSegment::try_new(section[0], section[1], tolerance)?.to_nurbs()?;
    let events = brep_events(&trace, cutter, None, tolerance)?;
    let mut intersections = [None; 2];
    for event in events {
        let CurveBrepIntersectionEvent::Overlap(overlap) = event else {
            continue;
        };
        let at_start = overlap.start().point().is_near(section[0], tolerance);
        let at_end = overlap.end().point().is_near(section[1], tolerance);
        if at_start && at_end {
            return Ok(Cut::Split {
                parameters,
                section,
            });
        }
        if at_start {
            intersections[0] = Some(section[0]);
        }
        if at_end {
            intersections[1] = Some(section[1]);
        }
    }
    Ok(Cut::Unsplit(intersections))
}

fn trim(
    wall: &Brep,
    parameters: [f64; 2],
    section: [Point3; 2],
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let face = &wall.faces()[0];
    let surface = face.surface();
    let trace = LineSegment::try_new(section[0], section[1], tolerance)?.to_nurbs()?;
    // The cut is across opposite rulings. The complete base-side piece is
    // retained even where its lower wall lies beyond the finite cutter.
    let [base, _] = Brep::try_split_rectangular_surface_face_south_north(
        surface.clone(),
        surface.domain_u(),
        surface.domain_v(),
        parameters,
        trace,
        face.is_reversed(),
        tolerance,
    )?;
    Ok(base)
}

fn imprint(
    wall: Brep,
    points: [Option<Point3>; 2],
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let mut splits = Vec::new();
    for point in points.into_iter().flatten() {
        let mut split = None;
        for (index, edge) in wall.edges().iter().enumerate() {
            let curve = edge.curve();
            let parameter = curve.closest_parameter(point, tolerance)?;
            let domain = curve.domain();
            if parameter > *domain.start()
                && parameter < *domain.end()
                && curve.evaluate(parameter)?.is_near(point, tolerance)
            {
                split = Some((index, vec![parameter]));
                break;
            }
        }
        splits.push(split.ok_or(REJECTED)?);
    }
    if splits.is_empty() {
        Ok(wall)
    } else {
        Ok(wall.try_split_edges_at_parameters(&splits, tolerance)?)
    }
}
