//! Owned command profiles: active ends and the first interior axis contact.
use super::*;

pub(super) fn prepare(
    curve: &NurbsCurve,
    axis: LineSegment,
    tolerance: Tolerance,
) -> Result<NurbsCurve, CommandError> {
    let curve = curve.clamped_to_active_domain()?;
    let zero = Point3::try_new(0., 0., 0.)?;
    let local = curve.transformed(AffineTransform3::from_translation(
        axis.start().vector_to(zero)?,
    ))?;
    let direction = axis.direction(tolerance)?.as_vector();
    let mut min = Real::INFINITY;
    let mut max = Real::NEG_INFINITY;
    for cv in local.control_points() {
        let along = zero.vector_to(cv.point())?.dot(direction)?;
        min = min.min(along);
        max = max.max(along);
    }
    let pad = tolerance.absolute().max((max - min).abs() * 1e-12);
    let line = LineSegment::try_new(
        zero.translated(direction.scaled(min - pad)?)?,
        zero.translated(direction.scaled(max + pad)?)?,
        Tolerance::NUMERICAL_VALIDATION,
    )?;
    let domain = curve.domain();
    let contacts =
        local.intersections_with_curve(&line.to_nurbs()?, Tolerance::NUMERICAL_VALIDATION)?;
    let start = local.evaluate(*domain.start())?;
    let end = local.evaluate(*domain.end())?;
    let contact = contacts.into_iter().find(|c| {
        let t = c.first_parameter();
        t > *domain.start()
            && t < *domain.end()
            && !c.point().is_near(start, tolerance)
            && !c.point().is_near(end, tolerance)
    });
    if let Some(contact) = contact.map(|c| c.first_parameter()) {
        Ok(curve.try_trimmed(*domain.start()..=contact)?)
    } else {
        Ok(curve)
    }
}
