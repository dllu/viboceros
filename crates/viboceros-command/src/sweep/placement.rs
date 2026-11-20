//! Automatic stations retain section order; reversing the rail changes traversal.
use super::*;
use viboceros_geometry::Plane;

pub(super) fn sections(
    source: CurveRef<'_>,
    profiles: Vec<NurbsCurve>,
    tolerance: Tolerance,
) -> Result<(NurbsCurve, Vec<SweepSection>), CommandError> {
    sections_with_direction(source, profiles, tolerance, false)
}

pub(super) fn sections_with_point_ends(
    source: CurveRef<'_>,
    profiles: Vec<NurbsCurve>,
    tolerance: Tolerance,
) -> Result<(NurbsCurve, Vec<SweepSection>), CommandError> {
    sections_with_direction(source, profiles, tolerance, true)
}

fn sections_with_direction(
    source: CurveRef<'_>,
    profiles: Vec<NurbsCurve>,
    tolerance: Tolerance,
    retain_direction: bool,
) -> Result<(NurbsCurve, Vec<SweepSection>), CommandError> {
    let mut rail = source.to_nurbs()?;
    let mut sections = profile_contacts(source, &rail, profiles, tolerance)?;
    let reverse = if sections.len() == 1 {
        if rail.is_closed()? {
            false
        } else {
            let sampler = viboceros_geometry::ArcLengthSampler::try_new(
                CurveRef::NurbsCurve(&rail),
                tolerance,
            )?;
            let distance = sampler.distance_at_parameter(sections[0].parameter)?;
            distance > sampler.total_length() * (0.5 + 8. * Real::EPSILON)
        }
    } else {
        sections[0].parameter > sections[1].parameter
    };
    if reverse && !retain_direction {
        rail = rail.reversed()?;
        for section in &mut sections {
            section.parameter = -section.parameter;
        }
    }
    Ok((rail, sections))
}

pub(super) fn profile_contacts(
    source: CurveRef<'_>,
    rail: &NurbsCurve,
    profiles: Vec<NurbsCurve>,
    tolerance: Tolerance,
) -> Result<Vec<SweepSection>, CommandError> {
    let mut sections = Vec::with_capacity(profiles.len());
    let nodes = if rail.degree() == 1
        && rail.control_points().iter().all(|c| c.weight() > 0.)
        && !rail
            .full_multiplicity_kink_parameters(tolerance)?
            .is_empty()
    {
        std::iter::once(*rail.domain().start())
            .chain(rail.spans().map(|(_, end)| end))
            .map(|t| Ok((t, rail.evaluate(t)?)))
            .collect::<Result<Vec<_>, GeometryError>>()?
    } else {
        vec![]
    };
    for curve in profiles {
        if !nodes.is_empty()
            && curve.is_closed()?
            && let Some(parameter) = enclosed_rail_node(&curve, &nodes, tolerance)?
        {
            sections.push(SweepSection { parameter, curve });
            continue;
        }
        let minimum = {
            let seed = viboceros_smlib::curve_minimum_distance(
                CurveRef::NurbsCurve(rail),
                CurveRef::NurbsCurve(&curve),
                [[0., 1.]; 2],
                tolerance,
            )
            .map_err(knot_refinement::native_geometry_error)?
            .ok_or(GeometryError::InvalidSweep {
                context: "could not place a section on the rail",
            })?;
            rail.minimum_distance_with_curve_seeded(&curve, tolerance, seed.parameters)?
        };
        let minimum = minimum.ok_or(GeometryError::InvalidSweep {context:"section placement exceeded the certified curve-distance query limits; supply explicit Parameters"})?;
        // The public Rhino command transports analytic arcs using their native
        // station directly in the rational rail chart. Keep that compatibility
        // policy here: the geometry query's minimum witness remains unchanged.
        let parameter = if matches!(source, CurveRef::Arc(_)) {
            source.parameter_from_nurbs(minimum.parameters[0])?
        } else {
            minimum.parameters[0]
        };
        sections.push(SweepSection { parameter, curve });
    }
    Ok(sections)
}

// A closed section can enclose a corner or endpoint. Use its station when
// that vertex is the sole rail contact inside the section's region.
fn enclosed_rail_node(
    curve: &NurbsCurve,
    nodes: &[(Real, Point3)],
    tolerance: Tolerance,
) -> Result<Option<Real>, GeometryError> {
    if curve
        .control_points()
        .iter()
        .any(|control| control.weight() <= 0.)
    {
        return Ok(None);
    }
    let bounds = curve.control_point_bounds();
    let min = bounds.min().to_array();
    let max = bounds.max().to_array();
    let slack = tolerance.absolute();
    let within_bounds = |p: Point3| {
        p.to_array()
            .into_iter()
            .enumerate()
            .all(|(i, x)| min[i] - slack <= x && x <= max[i] + slack)
    };
    if !nodes.iter().any(|&(_, p)| within_bounds(p)) {
        return Ok(None);
    }
    let face = match Brep::try_planar_face(curve, tolerance) {
        Ok(face) => face,
        Err(GeometryError::InvalidPlanarFaceBoundary) => return Ok(None),
        Err(error) => return Err(error),
    };
    let surface = face.faces()[0].surface();
    let frame = surface.frame_at(
        *surface.domain_u().start(),
        *surface.domain_v().start(),
        tolerance,
    )?;
    let plane = Plane::new(frame.origin(), frame.z_axis());
    let inside = |point| -> Result<bool, GeometryError> {
        if within_bounds(point)
            && let Some((_, u, v)) = face.closest_face_parameters(point, tolerance)?
        {
            return Ok(surface.evaluate(u, v)?.distance_to(point)? <= slack);
        }
        Ok(false)
    };
    let distances = nodes
        .iter()
        .map(|&(_, point)| plane.signed_distance_to(point))
        .collect::<Result<Vec<_>, _>>()?;
    let repeats_endpoint = nodes[0].1 == nodes[nodes.len() - 1].1;
    let mut hit = None;
    for (&(t, point), &distance) in nodes[..nodes.len() - usize::from(repeats_endpoint)]
        .iter()
        .zip(&distances)
    {
        if distance.abs() <= slack && inside(point)? && hit.replace(t).is_some() {
            return Ok(None);
        }
    }
    if hit.is_none() {
        return Ok(None);
    }
    for (segment, distance) in nodes.windows(2).zip(distances.windows(2)) {
        let a = segment[0].1.to_array();
        let b = segment[1].1.to_array();
        let da = distance[0].abs();
        let db = distance[1].abs();
        if da <= slack && db <= slack {
            if (0..3).all(|i| a[i].min(b[i]) <= max[i] + slack && a[i].max(b[i]) >= min[i] - slack)
            {
                return Ok(None);
            }
        } else if da > slack && db > slack && distance[0].signum() != distance[1].signum() {
            let scale = da.max(db);
            let fraction = (da / scale) / (da / scale + db / scale);
            let point = Point3::try_from(std::array::from_fn(|i| {
                (1. - fraction).mul_add(a[i], fraction * b[i])
            }))?;
            if inside(point)? {
                return Ok(None);
            }
        }
    }
    Ok(hit)
}

pub(super) fn full_rail_anchor(
    rail: &NurbsCurve,
    sections: &[SweepSection],
) -> Result<bool, GeometryError> {
    if sections.len() != 1 {
        return Ok(false);
    }
    let domain = rail.domain();
    let t = sections[0].parameter;
    if t <= *domain.start() || t >= *domain.end() {
        return Ok(false);
    }
    let scale = domain.end() - domain.start();
    if !scale.is_finite() {
        return Ok(false);
    }
    Ok(rail
        .greville_parameters_in_active_domain()?
        .into_iter()
        .any(|g| (g - t).abs() <= 64. * Real::EPSILON * scale))
}
