//! Infinite endpoint tangent supports keep native endpoint derivative speed.
use super::*;
use viboceros_geometry::{LineSegment, PolyCurve3};
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
    let mut best = None;
    for at_start in [true, false] {
        let t = if at_start {
            *domain.start()
        } else {
            *domain.end()
        };
        let (point, derivative) = source.as_ref().evaluate_with_derivative(t)?;
        let support = LineSegment::try_new(
            point,
            point.translated(derivative)?,
            Tolerance::NUMERICAL_VALIDATION,
        )?
        .try_reparameterized(t..=t + 1.)?;
        curves[i] = Curve3::Line(support);
        let mut line_seed = seed;
        line_seed[i] = t;
        let Some(contacts) = contacts::solve(
            [&curves[0], &curves[1]],
            line_seed,
            radius,
            plane,
            tolerance,
        )?
        else {
            continue;
        };
        let contact = contacts.parameters[i];
        if (at_start && contact >= t) || (!at_start && contact <= t) {
            continue;
        }
        let length = point.distance_to(contacts.points[i])?;
        if length <= tolerance.absolute() {
            continue;
        }
        if best.as_ref().is_some_and(|(old, _)| length >= *old) {
            continue;
        }
        let extension = if at_start {
            LineSegment::try_new(contacts.points[i], point, Tolerance::NUMERICAL_VALIDATION)?
                .try_reparameterized(contact..=t)?
        } else {
            LineSegment::try_new(point, contacts.points[i], Tolerance::NUMERICAL_VALIDATION)?
                .try_reparameterized(t..=contact)?
        };
        let original = source.to_polycurve()?;
        let extension = Curve3::Line(extension).to_polycurve()?;
        curves[i] = Curve3::PolyCurve(PolyCurve3::concatenate(&if at_start {
            [extension, original]
        } else {
            [original, extension]
        })?);
        best = Some((
            length,
            Candidate {
                curves: curves.clone(),
                contacts,
                reset_arc_domains: [false; 2],
            },
        ));
    }
    Ok(best.map(|(_, candidate)| candidate))
}
