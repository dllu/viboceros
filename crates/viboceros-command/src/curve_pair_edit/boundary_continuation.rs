//! Choose the least added endpoint continuation reaching a closed curve boundary.
use crate::CommandError;
use viboceros_geometry::{
    Curve3, CurveArcExtensionStyle, CurveOtherExtensionStyle, GeometryError, LineSegment, Point3,
    Real, Tolerance,
};
use viboceros_geometry::{
    CurveExtensionBoundary, CurveExtensionSide, CurveExtensionStyle, CurveRef, PolyCurve3,
};

#[derive(Clone, Copy)]
pub(crate) struct Styles {
    pub arc: CurveArcExtensionStyle,
    pub other: CurveOtherExtensionStyle,
}

pub(crate) fn to_boundary(
    source: &Curve3,
    boundary: &Curve3,
    pick: Point3,
    styles: Styles,
    tolerance: Tolerance,
) -> Result<Curve3, CommandError> {
    let start = source.as_ref().start_point()?;
    let end = source.as_ref().end_point()?;
    let preferred_start = pick.distance_to(start)? < pick.distance_to(end)?;
    let bounds = boundary.as_ref().tight_bounds(tolerance)?;
    let min = bounds.min().to_array();
    let max = bounds.max().to_array();
    let lower_bound = |point: Point3| -> Result<Real, CommandError> {
        let p = point.to_array();
        Ok(point.distance_to(Point3::try_from(std::array::from_fn(|i| {
            p[i].clamp(min[i], max[i])
        }))?)?)
    };
    let mut ends = [(true, lower_bound(start)?), (false, lower_bound(end)?)];
    if ends[0].1 > ends[1].1 || (ends[0].1 == ends[1].1 && !preferred_start) {
        ends.swap(0, 1);
    }
    let original_length = source.as_ref().length(tolerance)?;
    let mut best = None;
    for (at_start, lower) in ends {
        // Any continuation must reach the boundary's enclosing box. Avoid an
        // unbounded extrapolation search when even that lower bound is longer
        // than the candidate already found. Include both integration errors.
        if best.as_ref().is_some_and(|(length, _): &(Real, Curve3)| {
            let margin = 4.
                * tolerance
                    .absolute()
                    .max(tolerance.relative() * length.max(original_length));
            lower > *length - original_length + margin
        }) {
            continue;
        }
        match from_end(source, boundary, at_start, styles, tolerance) {
            Ok(curve) => {
                let length = curve.as_ref().length(tolerance)?;
                if best.as_ref().is_none_or(|(old, _)| length < *old) {
                    best = Some((length, curve));
                }
            }
            Err(CommandError::Geometry(GeometryError::CurveExtensionBoundaryNotFound)) => {}
            Err(error) => return Err(error),
        }
    }
    best.map(|(_, curve)| curve)
        .ok_or(GeometryError::CurveExtensionBoundaryNotFound.into())
}

fn from_end(
    source: &Curve3,
    boundary: &Curve3,
    at_start: bool,
    styles: Styles,
    tolerance: Tolerance,
) -> Result<Curve3, CommandError> {
    let side = if at_start {
        CurveExtensionSide::Start
    } else {
        CurveExtensionSide::End
    };
    let style = if matches!(source, Curve3::Arc(_)) {
        if styles.arc == CurveArcExtensionStyle::Arc {
            CurveExtensionStyle::Arc
        } else {
            CurveExtensionStyle::Line
        }
    } else if styles.other == CurveOtherExtensionStyle::Smooth {
        CurveExtensionStyle::Smooth
    } else {
        CurveExtensionStyle::Line
    };
    let nurbs = source.as_ref().to_nurbs()?;
    let boundaries = [CurveExtensionBoundary::Curve(boundary.as_ref().to_nurbs()?)];
    if style == CurveExtensionStyle::Smooth {
        return Ok(Curve3::NurbsCurve(
            crate::extension_backend::curve_boundary(
                &nurbs,
                side,
                style,
                &boundaries,
                tolerance,
                false,
            )?,
        ));
    }
    let pieces = crate::extension_backend::curve_boundary_pieces(
        &nurbs,
        side,
        style,
        &boundaries,
        tolerance,
    )?;
    let piece = pieces
        .first()
        .ok_or(CommandError::Usage("No extension meets the closed curve"))?;
    if let Curve3::Arc(arc) = source
        && style == CurveExtensionStyle::Arc
    {
        let p = if at_start {
            CurveRef::NurbsCurve(piece).start_point()?
        } else {
            CurveRef::NurbsCurve(piece).end_point()?
        };
        let radial = arc.center().vector_to(p)?;
        let angle = radial
            .dot(arc.y_axis().as_vector())?
            .atan2(radial.dot(arc.x_axis().as_vector())?)
            .rem_euclid(std::f64::consts::TAU);
        return Ok(Curve3::Arc(
            arc.try_extended_to_circle_angle(angle, !at_start)?,
        ));
    }
    // Preserve the original endpoint speed in the tangent line's chart.
    // Rationalizing an analytic arc changes that speed, so use the native
    // source evaluator rather than the boundary solver's NURBS chart.
    let a = CurveRef::NurbsCurve(piece).start_point()?;
    let b = CurveRef::NurbsCurve(piece).end_point()?;
    let line = LineSegment::try_new(a, b, Tolerance::NUMERICAL_VALIDATION)?;
    let d = source.as_ref().domain();
    let t = if at_start { *d.start() } else { *d.end() };
    let speed = source.as_ref().evaluate_with_derivative(t)?.1.length()?;
    let length = line.length()? / speed;
    let line = line.try_reparameterized(if at_start {
        *d.start() - length..=*d.start()
    } else {
        *d.end()..=*d.end() + length
    })?;
    let original = source.to_polycurve()?;
    let extension = Curve3::Line(line).to_polycurve()?;
    Ok(Curve3::PolyCurve(PolyCurve3::concatenate(&if at_start {
        [extension, original]
    } else {
        [original, extension]
    })?))
}
