//! Extend only the picked open end to the closed boundary before setback search.
use super::*;
use viboceros_geometry::{
    CurveExtensionBoundary, CurveExtensionSide, CurveExtensionStyle, CurveRef, PolyCurve3,
};

pub(super) fn to_boundary(
    source: &Curve3,
    boundary: &Curve3,
    pick: Point3,
    options: Preferences,
    tolerance: Tolerance,
) -> Result<Curve3, CommandError> {
    // Rhino's mixed-style nonmeeting arc/closed-curve branch is not yet
    // qualified: rigidly transformed inputs selected different continuations.
    // Reject before producing a plan rather than admitting divergent geometry.
    if matches!(source, Curve3::Arc(_))
        && options.styles.arc == CurveArcExtensionStyle::Line
        && options.styles.other == CurveOtherExtensionStyle::Smooth
    {
        return Err(CommandError::Usage(
            "Nonmeeting arc chamfers against closed curves require ExtendOtherCurvesBy=Line",
        ));
    }
    let start = source.as_ref().start_point()?;
    let end = source.as_ref().end_point()?;
    let at_start = pick.distance_to(start)? < pick.distance_to(end)?;
    let side = if at_start {
        CurveExtensionSide::Start
    } else {
        CurveExtensionSide::End
    };
    let style = if matches!(source, Curve3::Arc(_)) {
        if options.styles.arc == CurveArcExtensionStyle::Arc {
            CurveExtensionStyle::Arc
        } else {
            CurveExtensionStyle::Line
        }
    } else if options.styles.other == CurveOtherExtensionStyle::Smooth {
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
