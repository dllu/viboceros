//! Bounded line extrapolation keeps original parameter charts.
use crate::CommandError;
use viboceros_geometry::{BoundingBox3, Curve3, CurveOtherExtensionStyle, LineSegment, Tolerance};

pub(crate) fn curves(
    curves: [&Curve3; 2],
    style: CurveOtherExtensionStyle,
) -> Result<Option<[Curve3; 2]>, CommandError> {
    let [Curve3::Line(first), Curve3::Line(second)] = curves else {
        return Ok(None);
    };
    let bounds =
        BoundingBox3::from_points([first.start(), first.end(), second.start(), second.end()])?;
    let padding = 100. * bounds.min().distance_to(bounds.max())?;
    let fractions = [first, second].map(|line| {
        if style == CurveOtherExtensionStyle::Smooth {
            Ok(50.)
        } else {
            line.length().map(|length| padding / length)
        }
    });
    let [a, b] = fractions;
    from_fractions(curves, [a?, b?])
}

pub(crate) fn from_fractions(
    curves: [&Curve3; 2],
    fractions: [f64; 2],
) -> Result<Option<[Curve3; 2]>, CommandError> {
    let [Curve3::Line(first), Curve3::Line(second)] = curves else {
        return Ok(None);
    };
    let [a, b] = [(first, fractions[0]), (second, fractions[1])].map(
        |(line, fraction)| -> Result<_, CommandError> {
            let domain = line.domain();
            let width = *domain.end() - *domain.start();
            Ok(Curve3::Line(
                LineSegment::try_new(
                    line.point_at(-fraction)?,
                    line.point_at(1. + fraction)?,
                    Tolerance::NUMERICAL_VALIDATION,
                )?
                .try_reparameterized(
                    *domain.start() - fraction * width..=*domain.end() + fraction * width,
                )?,
            ))
        },
    );
    Ok(Some([a?, b?]))
}
