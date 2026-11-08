//! Complete the supporting circle, then restore the original arc chart.
use super::*;
pub(super) fn solve(
    original: [&Curve3; 2],
    seed: [Real; 2],
    i: usize,
    radius: Real,
    plane: Frame3,
    tolerance: Tolerance,
) -> Result<Option<Candidate>, CommandError> {
    let source = original[i];
    let domain = source.as_ref().domain();
    let mut curves = original.map(Clone::clone);
    if let Curve3::Arc(arc) = source {
        let width = (*domain.end() - *domain.start()) * std::f64::consts::TAU / arc.sweep_radians();
        curves[i] = Curve3::Arc(
            arc.closed()
                .try_reparameterized(*domain.start()..=*domain.start() + width)?,
        );
        if let Some(mut contacts) =
            contacts::solve([&curves[0], &curves[1]], seed, radius, plane, tolerance)?
        {
            let radial = arc.center().vector_to(contacts.points[i])?;
            let angle = radial
                .dot(arc.y_axis().as_vector())?
                .atan2(radial.dot(arc.x_axis().as_vector())?)
                .rem_euclid(std::f64::consts::TAU);
            let at_end = angle - arc.sweep_radians() < std::f64::consts::TAU - angle;
            let outside = angle > arc.sweep_radians();
            let extended = if outside {
                arc.try_extended_to_circle_angle(angle, at_end)?
            } else {
                *arc
            };
            contacts.parameters[i] = viboceros_geometry::CurveRef::Arc(&extended)
                .closest_parameter(contacts.points[i], tolerance)?;
            curves[i] = Curve3::Arc(extended);
            let mut reset_arc_domains = [false; 2];
            reset_arc_domains[i] = outside;
            return Ok(Some(Candidate {
                curves,
                contacts,
                reset_arc_domains,
            }));
        }
        return Ok(None);
    }
    Ok(None)
}
