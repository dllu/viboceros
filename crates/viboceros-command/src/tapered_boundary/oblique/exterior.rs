//! Detect a finite cutter's outer edge crossing a continuous swept wall.
use super::*;
use crate::curve_intersections::surface_events;
use viboceros_geometry::CurveSurfaceIntersectionEvent;

mod section;
mod slivers;

pub(super) use section::{Coverage, recover};

pub(super) fn crosses_rim(
    stock: &Brep,
    base: Plane,
    cutter: &Brep,
    displacement: Vector3,
    tolerance: Tolerance,
) -> Result<bool, CommandError> {
    let face = &cutter.faces()[0];
    let chart = face.surface();
    if let Some(plane) = chart.plane(tolerance)? {
        if displacement.dot(plane.normal().as_vector())?.abs() <= tolerance.absolute() {
            return Ok(false);
        }
    } else if !regular_projected_patch(chart, displacement, tolerance)? {
        return Ok(false);
    }

    let normal = base.normal().as_vector();
    let toward = normal.scaled(displacement.dot(normal)?.signum())?;
    let base_height = axial_point(base.origin(), toward)?;
    let [near, far] = axial_extent(cutter.padded_trim_bounds()?, toward)?;
    let epsilon = tolerance.absolute();
    if near <= base_height + epsilon {
        return Ok(false);
    }
    // All of the far rim clears the cutter. Contacts in the interior of a
    // wall can then be attributed to the finite cutter's exterior.
    for edge in naked_edges(stock) {
        for control in edge.curve().control_points() {
            if control.weight() <= 0. || axial_point(control.point(), toward)? <= far + epsilon {
                return Ok(false);
            }
        }
    }

    let exterior = face.loops()[0]
        .trims()
        .iter()
        .map(|trim| Ok(cutter.edges()[trim.edge().ok_or(REJECTED)?].curve()))
        .collect::<Result<Vec<_>, CommandError>>()?;
    for wall in stock.faces() {
        let surface = wall.surface();
        if on_plane(surface, base, tolerance) {
            continue;
        }
        for &boundary in &exterior {
            let domain = boundary.domain();
            for event in surface_events(boundary, surface, tolerance)? {
                let CurveSurfaceIntersectionEvent::Point(hit) = event else {
                    continue;
                };
                let t = hit.curve_parameter();
                if t <= *domain.start()
                    || t >= *domain.end()
                    || !wall.contains_parameters(hit.u(), hit.v(), tolerance)?
                {
                    continue;
                }
                let (Ok(tangent), Ok(normal)) = (
                    boundary.derivative_at(t)?.normalized_nonzero(),
                    surface.normal_at(hit.u(), hit.v()),
                ) else {
                    continue;
                };
                if tangent.as_vector().dot(normal.as_vector())?.abs() > tolerance.angular() {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}
