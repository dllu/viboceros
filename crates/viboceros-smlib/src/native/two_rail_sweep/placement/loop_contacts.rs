use super::{Error, NurbsCurve, Point3, Tolerance};
use crate::native::curve_intersection_events;
use viboceros_geometry::CurveCurveIntersectionEvent;

/// Align a whole section with a rail station. Closed sections can use both
/// rails as guides even when a rail misses the section's image.
pub(super) fn stations(
    rails: &[NurbsCurve; 2],
    section: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<[f64; 2], Error> {
    let center = section.control_point_bounds().center()?;
    Ok([
        station(&rails[0], section, center, tolerance)?,
        station(&rails[1], section, center, tolerance)?,
    ])
}

pub(super) fn station(
    rail: &NurbsCurve,
    section: &NurbsCurve,
    center: Point3,
    tolerance: Tolerance,
) -> Result<f64, Error> {
    let seed = rail.closest_parameter(center, tolerance)?;
    let nearest = |t| -> Result<(Point3, f64), Error> {
        let point = rail.evaluate(t)?;
        let u = section.closest_parameter(point, tolerance)?;
        let on_section = section.evaluate(u)?;
        Ok((on_section, point.distance_to(on_section)?))
    };
    let (_, distance) = nearest(seed)?;
    let mut station = seed;
    if distance > tolerance.absolute() {
        if let Some(contact) = curve_intersection_events(rail, section, tolerance)?
            .into_iter()
            .map(|event| match event {
                CurveCurveIntersectionEvent::Point(point) => point.first_parameter(),
                CurveCurveIntersectionEvent::Overlap(overlap) => {
                    let interval = overlap.first_interval();
                    seed.clamp(*interval.start(), *interval.end())
                }
            })
            .min_by(|a, b| (a - seed).abs().total_cmp(&(b - seed).abs()))
        {
            station = contact;
        } else {
            // Refine a disjoint guide locally from the projection of the section.
            for _ in 0..8 {
                let (point, _) = nearest(station)?;
                let next = rail.closest_parameter(point, tolerance)?;
                if (next - station).abs() <= 1e-12 {
                    break;
                }
                station = next;
            }
        }
    }
    let point = rail.evaluate(station)?;
    for &knot in &rail.knots()[rail.degree()..=rail.control_points().len()] {
        if (knot - station).abs() <= 1e-10
            && rail.evaluate(knot)?.distance_to(point)? <= tolerance.absolute()
        {
            return Ok(knot);
        }
    }
    Ok(station)
}
