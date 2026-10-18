//! Restore native UV gaps only along certified collapsed surface boundaries.
use super::*;

#[cfg(test)]
mod tests;

// At a clamped natural boundary, these are the surface's stored row/column,
// without the rounded homogeneous evaluation of a general extracted isocurve.
fn natural_boundary_controls(
    surface: &NurbsSurface,
    iso: SurfaceIso,
) -> Option<Vec<WeightedPoint3>> {
    let nu = surface.control_point_count_u();
    let nv = surface.control_point_count_v();
    let (knots, degree, count, end) = match iso {
        SurfaceIso::South => (surface.knots_v(), surface.degree_v(), nv, false),
        SurfaceIso::North => (surface.knots_v(), surface.degree_v(), nv, true),
        SurfaceIso::West => (surface.knots_u(), surface.degree_u(), nu, false),
        SurfaceIso::East => (surface.knots_u(), surface.degree_u(), nu, true),
        _ => return None,
    };
    let clamp = if end {
        &knots[count..]
    } else {
        &knots[..=degree]
    };
    if clamp.iter().any(|k| *k != clamp[0]) {
        return None;
    }
    let controls = surface.control_points();
    Some(match iso {
        SurfaceIso::South => controls[..nu].to_vec(),
        SurfaceIso::North => controls[(nv - 1) * nu..].to_vec(),
        SurfaceIso::West => (0..nv).map(|j| controls[j * nu]).collect(),
        SurfaceIso::East => (0..nv).map(|j| controls[j * nu + nu - 1]).collect(),
        _ => unreachable!(),
    })
}

// UV units are independent of model units. Scale only by this axis's extent;
// translated origins and a unit floor must not merge opposite chart sides.
fn parameter_epsilon(domain: std::ops::RangeInclusive<f64>) -> f64 {
    let factor = 256. * f64::EPSILON;
    let width = domain.end() - domain.start();
    if width.is_finite() {
        width * factor
    } else {
        domain.end() * factor - domain.start() * factor
    }
}

fn pole_connector(
    surface: &NurbsSurface,
    vertex: BrepVertex,
    end: Point2,
    start: Point2,
    tolerance: Tolerance,
) -> Result<Option<(NurbsCurve2, SurfaceIso)>, Error> {
    let domains = [surface.domain_u(), surface.domain_v()];
    let mut side = None;
    for (axis, boundary, iso) in [
        (1, *domains[1].start(), SurfaceIso::South),
        (1, *domains[1].end(), SurfaceIso::North),
        (0, *domains[0].start(), SurfaceIso::West),
        (0, *domains[0].end(), SurfaceIso::East),
    ] {
        let epsilon = parameter_epsilon(domains[axis].clone());
        if (end.to_array()[axis] - boundary).abs() <= epsilon
            && (start.to_array()[axis] - boundary).abs() <= epsilon
        {
            side = Some((axis, boundary, iso));
            break;
        }
    }
    let Some((axis, boundary, iso)) = side else {
        return Ok(None);
    };
    let snap = |point: Point2| {
        let mut p = point.to_array();
        p[axis] = boundary;
        p[1 - axis] = p[1 - axis].clamp(*domains[1 - axis].start(), *domains[1 - axis].end());
        Point2::try_from(p)
    };
    let curve = NurbsCurve2::try_new_rational(
        1,
        vec![
            WeightedPoint2::try_new(snap(end)?, 1.)?,
            WeightedPoint2::try_new(snap(start)?, 1.)?,
        ],
        vec![0., 0., 1., 1.],
    )?;
    // Model-space tolerance is used only for the spatial collapse proof. A
    // single-signed clamped row/column lies in its Euclidean control hull,
    // independently of unrelated interior weights. Other boundaries retain
    // the continuous surface-image certificate.
    let epsilon = tolerance.absolute().max(vertex.tolerance());
    let (controls, clamped) = match natural_boundary_controls(surface, iso) {
        Some(controls) => (controls, true),
        None => (
            match axis {
                0 => surface.isocurve_v(boundary)?,
                _ => surface.isocurve_u(boundary)?,
            }
            .control_points()
            .to_vec(),
            false,
        ),
    };
    let sign = controls[0].weight().is_sign_positive();
    if controls.iter().any(|control| {
        control.weight().is_sign_positive() != sign
            || control
                .point()
                .distance_to(vertex.point())
                .map_or(true, |d| d > epsilon)
    }) {
        // A short gap on a regular boundary is endpoint uncertainty, not a
        // pole. Require the complete boundary to collapse, as on import.
        return Ok(None);
    }
    let mut certified = false;
    if clamped {
        let sign = controls[0].weight().is_sign_positive();
        certified = true;
        for control in &controls {
            if control.weight().is_sign_positive() != sign
                || control
                    .point()
                    .distance_bound_to(vertex.point(), epsilon)?
                    .is_none()
            {
                certified = false;
                break;
            }
        }
    }
    if !certified {
        let support = NurbsCurve::try_new_rational(
            1,
            vec![WeightedPoint3::try_new(vertex.point(), 1.)?; 2],
            vec![0., 0., 1., 1.],
        )?;
        let endpoint = |value| -> Result<WeightedPoint2, GeometryError> {
            let mut p = [0.; 2];
            p[axis] = boundary;
            p[1 - axis] = value;
            WeightedPoint2::try_new(Point2::try_from(p)?, 1.)
        };
        let whole = NurbsCurve2::try_new_rational(
            1,
            vec![
                endpoint(*domains[1 - axis].start())?,
                endpoint(*domains[1 - axis].end())?,
            ],
            vec![0., 0., 1., 1.],
        )?;
        certified = surface
            .parameter_curve_deviation_bound(&whole, &support, epsilon)?
            .is_some();
    }
    if !certified {
        return Ok(None);
    }
    Ok(Some((curve, iso)))
}

pub(super) fn close_pole_gaps(
    surface: &NurbsSurface,
    vertices: &[BrepVertex],
    trims: Vec<BrepTrim>,
    tolerance: Tolerance,
) -> Result<Vec<BrepTrim>, Error> {
    let mut gaps = Vec::with_capacity(trims.len());
    for (index, trim) in trims.iter().enumerate() {
        let next = &trims[(index + 1) % trims.len()];
        let end = trim.curve().end_point()?;
        let start = next.curve().start_point()?;
        if end == start {
            gaps.push(None);
            continue;
        }
        let index = trim.vertices()[1];
        if index != next.vertices()[0] {
            return Err(Error::InvalidOutput);
        }
        let vertex = *vertices.get(index).ok_or(Error::InvalidOutput)?;
        if let Some((curve, iso)) = pole_connector(surface, vertex, end, start, tolerance)? {
            gaps.push(Some(BrepTrim::try_new(
                [index, index],
                None,
                false,
                curve,
                BrepTrimType::Singular,
                iso,
                [0.; 2],
            )?));
        } else if (end.x() - start.x()).abs() > tolerance.absolute()
            || (end.y() - start.y()).abs() > tolerance.absolute()
        {
            return Err(Error::Kernel(format!(
                "UV loop gap is not a certified pole boundary: {end:?} to {start:?}"
            )));
        } else {
            gaps.push(None);
        }
        // Ordinary endpoint uncertainty retains the shared B-rep validation
        // policy. It must not suppress a certified pole connector, however
        // small that connector's parameter-space span is.
    }
    let mut completed =
        Vec::with_capacity(trims.len() + gaps.iter().filter(|g| g.is_some()).count());
    for (trim, gap) in trims.into_iter().zip(gaps) {
        completed.push(trim);
        if let Some(gap) = gap {
            completed.push(gap);
        }
    }
    Ok(completed)
}
