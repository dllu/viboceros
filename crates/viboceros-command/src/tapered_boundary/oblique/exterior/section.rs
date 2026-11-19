//! Compare the finite cutter with the complete section of its carrier.
use super::slivers::{self, OutsideSpan};
use super::*;
use crate::curve_intersections::events;
use viboceros_geometry::{BrepLoop, CurveCurveIntersectionEvent};

pub(in crate::tapered_boundary::oblique) enum Coverage {
    Complete(Brep),
    RimContact,
    MissedRim,
    Unresolved,
}

pub(in crate::tapered_boundary::oblique) fn recover(
    sweep: &Shape,
    stock: &Brep,
    source: &Brep,
    cutter: &Brep,
    displacement: Vector3,
    cap: bool,
    tolerance: Tolerance,
) -> Result<Coverage, CommandError> {
    let face = &cutter.faces()[0];
    let plane = face.surface().plane(tolerance)?;
    let support = match plane {
        Some(plane) => support::planar(stock, plane, displacement, tolerance),
        None => support::warped(stock, face.surface(), displacement, tolerance),
    };
    let section = support.and_then(|support| {
        clip(
            sweep,
            stock,
            source,
            &support,
            displacement,
            true,
            tolerance,
        )
    });
    let section = match section {
        Ok(section) => section,
        Err(CommandError::SurfaceExtrusionBoundaryRejected) => return Ok(Coverage::Unresolved),
        Err(error) => return Err(error),
    };
    let cap_index = section.faces().len() - 1;
    let end = &section.faces()[cap_index];
    let outline = face.loops()[0]
        .trims()
        .iter()
        .map(|trim| Ok(cutter.edges()[trim.edge().ok_or(REJECTED)?].curve()))
        .collect::<Result<Vec<_>, CommandError>>()?;
    let mut covered = false;
    let mut uncovered = false;
    let mut gaps = Vec::new();
    let mut contacts = Vec::new();
    for rim in end.loops() {
        let rim = spans(&section, rim, face, &outline, tolerance)?;
        let outside = !rim.outside.is_empty();
        if rim.inside {
            gaps.extend(rim.outside);
        }
        contacts.extend(rim.contacts);
        covered |= rim.inside || rim.boundary && !outside;
        uncovered |= outside && !rim.inside;
    }
    let stitched = !gaps.is_empty();
    let section = if !stitched {
        if covered && !uncovered && !contacts.is_empty() {
            section
                .try_split_edges_at_parameters(&contacts, tolerance)
                .unwrap_or(section)
        } else {
            section
        }
    } else if !uncovered
        && let Ok(Some(joined)) = slivers::stitch(&section, &gaps, &contacts, &outline, tolerance)
    {
        joined
    } else {
        return Ok(Coverage::RimContact);
    };
    match (covered, uncovered) {
        (true, false) => {}
        (false, true) => return Ok(Coverage::MissedRim),
        _ => return Ok(Coverage::Unresolved),
    }

    if !cap {
        let base = source.faces()[0]
            .surface()
            .plane(tolerance)?
            .ok_or(REJECTED)?;
        let walls = (0..cap_index)
            .filter(|&i| !on_plane(section.faces()[i].surface(), base, tolerance))
            .collect::<Vec<_>>();
        return Ok(Coverage::Complete(section.sub_brep(&walls, tolerance)?));
    }
    let restored = if plane.is_some() {
        mount_planar_cap(section, cap_index, face.surface(), tolerance).map(Some)
    } else {
        let stock = section.sub_brep(&(0..cap_index).collect::<Vec<_>>(), tolerance)?;
        stock
            .try_cap_on_surface(face.surface(), tolerance)
            .map_err(CommandError::from)
    };
    match restored {
        Ok(Some(result)) => Ok(Coverage::Complete(result)),
        _ if stitched => Ok(Coverage::RimContact),
        Ok(None) => Ok(Coverage::Unresolved),
        Err(error) => Err(error),
    }
}

#[derive(Default)]
struct RimCoverage {
    inside: bool,
    outside: Vec<OutsideSpan>,
    boundary: bool,
    contacts: Vec<(usize, Vec<f64>)>,
}

fn spans(
    section: &Brep,
    rim: &BrepLoop,
    face: &BrepFace,
    outline: &[&NurbsCurve],
    tolerance: Tolerance,
) -> Result<RimCoverage, CommandError> {
    let mut coverage = RimCoverage::default();
    for trim in rim.trims() {
        let edge = trim.edge().ok_or(REJECTED)?;
        let curve = section.edges()[edge].curve();
        let domain = curve.domain();
        let mut limits = vec![*domain.start(), *domain.end()];
        for boundary in outline {
            for event in events(curve, boundary, tolerance)? {
                match event {
                    CurveCurveIntersectionEvent::Point(hit) => limits.push(hit.first_parameter()),
                    CurveCurveIntersectionEvent::Overlap(shared) => {
                        limits.extend([
                            shared.start().first_parameter(),
                            shared.end().first_parameter(),
                        ]);
                    }
                }
            }
        }
        limits.sort_unstable_by(f64::total_cmp);
        limits.dedup();
        let contacts = interior_contacts(curve, &limits, tolerance)?;
        if !contacts.is_empty() {
            coverage.contacts.push((edge, contacts));
        }
        // With the rim split at its contacts on the shared carrier, the
        // interior of each interval stays on one side of the cutter's trim.
        for interval in limits.windows(2) {
            let point = curve.evaluate(interval[0].midpoint(interval[1]))?;
            let between_contacts = interval[0] > *domain.start() && interval[1] < *domain.end();
            let side = if let Some(side) =
                near_outline(face, outline, point, between_contacts, tolerance)?
            {
                side
            } else if contains(face, point, tolerance)? {
                RimSide::Inside
            } else {
                RimSide::Outside
            };
            match side {
                RimSide::OnBoundary => coverage.boundary = true,
                RimSide::Inside => coverage.inside = true,
                RimSide::Outside => {
                    if let Some(previous) = coverage.outside.last_mut()
                        && previous.edge == edge
                        && previous.domain[1] == interval[0]
                    {
                        previous.domain[1] = interval[1];
                    } else {
                        coverage.outside.push(OutsideSpan {
                            edge,
                            domain: [interval[0], interval[1]],
                        });
                    }
                }
            }
        }
    }
    Ok(coverage)
}

fn interior_contacts(
    curve: &NurbsCurve,
    limits: &[f64],
    tolerance: Tolerance,
) -> Result<Vec<f64>, GeometryError> {
    let [start, .., end] = limits else {
        return Ok(Vec::new());
    };
    let mut prior = curve.evaluate(*start)?;
    let end = curve.evaluate(*end)?;
    let mut contacts = Vec::new();
    for &parameter in &limits[1..limits.len() - 1] {
        let point = curve.evaluate(parameter)?;
        if !point.is_near(prior, tolerance) && !point.is_near(end, tolerance) {
            contacts.push(parameter);
            prior = point;
        }
    }
    Ok(contacts)
}

enum RimSide {
    Inside,
    OnBoundary,
    Outside,
}

fn near_outline(
    face: &BrepFace,
    outline: &[&NurbsCurve],
    point: Point3,
    between_contacts: bool,
    tolerance: Tolerance,
) -> Result<Option<RimSide>, GeometryError> {
    for (edge, trim) in outline.iter().zip(face.loops()[0].trims()) {
        let parameter = edge.closest_parameter(point, tolerance)?;
        let near = edge.evaluate(parameter)?;
        if !near.is_near(point, tolerance) {
            continue;
        }
        if between_contacts {
            // Contact tolerance alone can hide a short interval on the
            // exterior side. The oriented outline distinguishes that clip
            // from tangency before the replacement edge is certified.
            let tangent = edge
                .derivative_at(parameter)?
                .normalized_nonzero()?
                .as_vector();
            let (u, v) = face.surface().closest_parameters(near, tolerance)?;
            let normal = face.surface().normal_at(u, v)?.as_vector();
            let inward =
                normal
                    .cross(tangent)?
                    .scaled(if trim.is_reversed_3d() { -1. } else { 1. })?;
            let roundoff =
                near.to_array().into_iter().map(f64::abs).fold(1., f64::max) * (64. * f64::EPSILON);
            if near.vector_to(point)?.dot(inward)? < -roundoff {
                return Ok(Some(RimSide::Outside));
            }
        }
        return Ok(Some(RimSide::OnBoundary));
    }
    Ok(None)
}

fn contains(face: &BrepFace, point: Point3, tolerance: Tolerance) -> Result<bool, GeometryError> {
    let (u, v) = face.surface().closest_parameters(point, tolerance)?;
    Ok(face.surface().evaluate(u, v)?.is_near(point, tolerance)
        && face.contains_parameters(u, v, tolerance)?)
}
