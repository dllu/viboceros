//! Analytic, fitted and original-basis offsets with endpoint correspondence.
use super::*;
use crate::{Curve3, CurveOffsetCornerStyle, WeightedPoint3};
pub(super) fn shifted_start(
    curve: &NurbsCurve,
    normal: UnitVector3,
    distance: Real,
) -> Result<Point3, GeometryError> {
    let t = *curve.domain().start();
    let tangent = curve.tangent_at_on_side(t, ParameterSide::Right)?;
    curve.evaluate(t)?.translated(
        normal
            .as_vector()
            .cross(tangent.as_vector())?
            .scaled(distance)?,
    )
}

fn analytic_offset(
    curve: &NurbsCurve,
    normal: UnitVector3,
    distance: Real,
    tolerance: Tolerance,
) -> Result<Option<NurbsCurve>, GeometryError> {
    let tight = Tolerance::try_new(
        tolerance.absolute() * 0.01,
        tolerance.relative(),
        tolerance.angular(),
    )?;
    let transform = if matches!(curve.analytic_form(tight)?, Some(CurveSegment3::Line(_))) {
        let t = *curve.domain().start();
        let shift = normal
            .as_vector()
            .cross(
                curve
                    .tangent_at_on_side(t, ParameterSide::Right)?
                    .as_vector(),
            )?
            .scaled(distance)?;
        crate::AffineTransform3::from_translation(shift)
    } else if let Some(center) = curve.circular_center(tight)? {
        let p = curve.evaluate(*curve.domain().start())?;
        let radius = center.distance_to(p)?;
        let target = shifted_start(curve, normal, distance)?;
        let left = p.vector_to(target)?;
        let radial = center.vector_to(p)?.normalized_nonzero()?.as_vector();
        let new_radius = radius + left.dot(radial)?;
        if new_radius <= tolerance.absolute() {
            return Err(invalid("inset collapses a circular boundary"));
        }
        crate::AffineTransform3::try_uniform_scale(center, new_radius / radius)?
    } else {
        return Ok(None);
    };
    let candidate = curve.transformed(transform)?;
    // Recognition bounds positions. Match the shared offset fitter's normal-
    // locus sampling policy too, before accepting an analytic shortcut.
    for (a, b) in curve.spans() {
        for i in 0..=16 {
            let t = a * (1. - i as Real / 16.) + b * (i as Real / 16.);
            let side = if i == 16 {
                ParameterSide::Left
            } else {
                ParameterSide::Right
            };
            let expected = curve.evaluate(t)?.translated(
                normal
                    .as_vector()
                    .cross(curve.tangent_at_on_side(t, side)?.as_vector())?
                    .scaled(distance)?,
            )?;
            if candidate.evaluate(t)?.distance_to(expected)? > tolerance.absolute() * 0.25 {
                return Ok(None);
            }
        }
    }
    Ok(Some(candidate))
}

pub(super) fn offset(
    curve: &NurbsCurve,
    normal: UnitVector3,
    distance: Real,
    tolerance: Tolerance,
    loose: bool,
    operations: PlanarInsetOperations,
) -> Result<(NurbsCurve, bool), GeometryError> {
    curve.tight_bounds(tolerance)?;
    if let Some(result) = analytic_offset(curve, normal, distance, tolerance)? {
        return Ok((result, true));
    }
    if loose {
        return curve
            .try_loose_planar_offset(distance, normal, tolerance, operations.edit_points)
            .map(|c| (c, true));
    }
    let fit = Tolerance::try_new(
        tolerance.absolute() * 0.25,
        tolerance.relative(),
        tolerance.angular(),
    )?;
    let regular = Curve3::NurbsCurve(curve.clone()).try_offset_with_operations(
        distance,
        normal,
        fit,
        CurveOffsetCornerStyle::Sharp,
        operations.curves,
    );
    match regular {
        Ok(c) => Ok((c.as_ref().to_nurbs()?, true)),
        Err(GeometryError::Degenerate {
            context: "NURBS offset cusp",
        }) if curve.is_closed()? => Ok((
            crate::curve_offset::offset_closed_region(
                curve,
                distance,
                normal,
                fit,
                operations.curves,
            )?,
            false,
        )),
        Err(e) => Err(e),
    }
}

pub(super) fn aligned(
    curve: NurbsCurve,
    a: Point3,
    b: Point3,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    let curve = curve.clamped_to_active_domain()?;
    if curve.evaluate(*curve.domain().start())?.distance_to(a)? > tolerance.absolute() * 0.5
        || curve.evaluate(*curve.domain().end())?.distance_to(b)? > tolerance.absolute() * 0.5
    {
        return Err(invalid(
            "smooth inset endpoint correspondence is unresolved",
        ));
    }
    let mut controls = curve.control_points().to_vec();
    let last = controls.len() - 1;
    controls[0] = WeightedPoint3::try_new(a, controls[0].weight())?;
    controls[last] = WeightedPoint3::try_new(b, controls[last].weight())?;
    NurbsCurve::try_new_rational(curve.degree(), controls, curve.knots().to_vec())
}
