//! Circular wraparound and opposite-end tangent continuation after consumption.
use super::*;
use viboceros_geometry::PolyCurve3;

pub(super) struct Continuation {
    pub cut: Point3,
    pub curve: Curve3,
    pub reset_arc_domain: bool,
}

pub(super) fn solve(
    source: &Curve3,
    corner: Real,
    center: Point3,
    forward: bool,
    distance: Real,
    options: Preferences,
) -> Result<Continuation, CommandError> {
    if let Curve3::Arc(arc) = source
        && options.styles.arc == CurveArcExtensionStyle::Arc
    {
        let ratio = distance / (2. * arc.radius());
        if ratio > 1. {
            return Err(CommandError::Usage(
                "Chamfer distance exceeds the supporting circle",
            ));
        }
        let d = arc.domain();
        let angle = (corner - *d.start()) / (*d.end() - *d.start()) * arc.sweep_radians()
            + if forward { 1. } else { -1. } * 2. * ratio.asin();
        let extended =
            arc.try_extended_to_circle_angle(angle.rem_euclid(std::f64::consts::TAU), !forward)?;
        let cut = extended.point_at(if forward { 0. } else { 1. })?;
        return Ok(Continuation {
            cut,
            curve: Curve3::Arc(extended),
            reset_arc_domain: true,
        });
    }
    if !matches!(source, Curve3::Arc(_)) && options.styles.other == CurveOtherExtensionStyle::Smooth
    {
        return Err(CommandError::Usage(
            "Chamfer distance consumes the smooth source",
        ));
    }
    let curve = source.as_ref();
    let d = curve.domain();
    let far = if forward { *d.end() } else { *d.start() };
    let (point, derivative) = curve.evaluate_with_derivative(far)?;
    let direction = derivative
        .normalized_nonzero()?
        .as_vector()
        .scaled(if forward { 1. } else { -1. })?;
    let offset = center.vector_to(point)?;
    let b = offset.dot(direction)?;
    let c = offset.dot(offset)? - distance * distance;
    let discriminant = b * b - c;
    if discriminant < 0. {
        return Err(CommandError::Usage(
            "No tangent extension reaches the setback",
        ));
    }
    let root = discriminant.sqrt();
    let length = if b > 0. { -c / (b + root) } else { -b + root };
    if length < 0. {
        return Err(CommandError::Usage(
            "No outward tangent reaches the setback",
        ));
    }
    let cut = point.translated(direction.scaled(length)?)?;
    let near = if forward { *d.start() } else { *d.end() };
    let (point, derivative) = curve.evaluate_with_derivative(near)?;
    let speed = derivative.length()?;
    let direction = derivative
        .normalized_nonzero()?
        .as_vector()
        .scaled(if forward { -1. } else { 1. })?;
    let length = distance + point.distance_to(center)?;
    let tip = point.translated(direction.scaled(length)?)?;
    let line = LineSegment::try_new(
        if forward { tip } else { point },
        if forward { point } else { tip },
        Tolerance::NUMERICAL_VALIDATION,
    )?
    .try_reparameterized(if forward {
        near - length / speed..=near
    } else {
        near..=near + length / speed
    })?;
    let original = source.to_polycurve()?;
    let extension = Curve3::Line(line).to_polycurve()?;
    let curve = Curve3::PolyCurve(PolyCurve3::concatenate(&if forward {
        [extension, original]
    } else {
        [original, extension]
    })?);
    Ok(Continuation {
        cut,
        curve,
        reset_arc_domain: false,
    })
}
