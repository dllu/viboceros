//! Infinite line intersections retain the original native parameter chart.
use crate::CommandError;
use viboceros_geometry::{Curve3, LineSegment, Point3, Real, Tolerance};

/// Intersect finite curves and line supports spanning each partner's bounds.
pub(crate) fn intersections(
    curves: [&Curve3; 2],
    tolerance: Tolerance,
) -> Result<Vec<viboceros_geometry::CurveCurveIntersectionEvent>, CommandError> {
    let supports = [
        curve(curves[0], curves[1], tolerance)?,
        curve(curves[1], curves[0], tolerance)?,
    ];
    crate::curve_intersections::events(
        &supports[0].as_ref().to_nurbs()?,
        &supports[1].as_ref().to_nurbs()?,
        tolerance,
    )
}

pub(crate) fn parameter(
    curve: &Curve3,
    point: Point3,
    tolerance: Tolerance,
) -> Result<Real, CommandError> {
    if let Curve3::Line(line) = curve {
        let f = line
            .start()
            .vector_to(point)?
            .dot(line.direction(tolerance)?.as_vector())?
            / line.length()?;
        let d = line.domain();
        Ok(f.mul_add(*d.end() - *d.start(), *d.start()))
    } else {
        Ok(curve.as_ref().closest_parameter(point, tolerance)?)
    }
}
pub(crate) fn retained(
    curve: &Curve3,
    corner: Real,
    point: Point3,
    forward: bool,
) -> Result<Curve3, CommandError> {
    let d = curve.as_ref().domain();
    let interval = if forward {
        corner..=*d.end()
    } else {
        *d.start()..=corner
    };
    if let Curve3::Line(line) = curve {
        let (a, b) = if forward {
            (point, line.end())
        } else {
            (line.start(), point)
        };
        Ok(Curve3::Line(
            LineSegment::try_new(a, b, Tolerance::NUMERICAL_VALIDATION)?
                .try_reparameterized(interval)?,
        ))
    } else {
        crate::curve_segments::trimmed(curve, interval)
    }
}
pub(crate) fn curve(
    source: &Curve3,
    other: &Curve3,
    tolerance: Tolerance,
) -> Result<Curve3, CommandError> {
    let Curve3::Line(line) = source else {
        return Ok(source.clone());
    };
    let bounds = other.as_ref().tight_bounds(tolerance)?;
    let min = bounds.min();
    let max = bounds.max();
    let u = line.direction(tolerance)?.as_vector();
    let mut limits = [0., line.length()?];
    for x in [min.x(), max.x()] {
        for y in [min.y(), max.y()] {
            for z in [min.z(), max.z()] {
                let t = line.start().vector_to(Point3::try_new(x, y, z)?)?.dot(u)?;
                limits[0] = limits[0].min(t);
                limits[1] = limits[1].max(t);
            }
        }
    }
    let length = line.length()?;
    let d = line.domain();
    let t0 = (limits[0] - tolerance.absolute()) / length;
    let t1 = (limits[1] + tolerance.absolute()) / length;
    Ok(Curve3::Line(
        LineSegment::try_new(
            line.point_at(t0)?,
            line.point_at(t1)?,
            Tolerance::NUMERICAL_VALIDATION,
        )?
        .try_reparameterized(
            t0.mul_add(*d.end() - *d.start(), *d.start())
                ..=t1.mul_add(*d.end() - *d.start(), *d.start()),
        )?,
    ))
}

pub(crate) fn distance(
    curve: &Curve3,
    a: Real,
    b: Real,
    tolerance: Tolerance,
) -> Result<Real, CommandError> {
    if a == b {
        return Ok(0.);
    }
    if let Curve3::Line(line) = curve {
        let d = line.domain();
        return Ok((a - b).abs() / (*d.end() - *d.start()) * line.length()?);
    }
    let interval = a.min(b)..=a.max(b);
    Ok(crate::curve_segments::trimmed(curve, interval)?
        .as_ref()
        .length(tolerance)?)
}
