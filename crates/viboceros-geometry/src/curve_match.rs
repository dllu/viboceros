//! End matching for a single NURBS span, retaining its rational weights.

use crate::{
    Curve3, CurveBlendContinuity, CurveRef, GeometryError, NurbsCurve, ParameterSide, Real,
    Tolerance, WeightedPoint3,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveMatchPreserveEnd {
    None,
    Position,
    Tangency,
    Curvature,
}

/// Changes the selected end of one open, single-span curve to meet another
/// open curve with G0, G1, or G2 continuity. Degree elevation leaves the
/// original rational locus unchanged before the end controls are modified.
/// The opposite end's requested position, tangent, or curvature is preserved
/// by reserving that many untouched Bézier controls.
pub fn try_match_curve_end(
    source: &Curve3,
    source_at_end: bool,
    reference: &Curve3,
    reference_at_end: bool,
    continuity: CurveBlendContinuity,
    preserve: CurveMatchPreserveEnd,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    if source.as_ref().is_closed()? || reference.as_ref().is_closed()? {
        return Err(GeometryError::InvalidPolyCurve {
            context: "Match requires open curves",
        });
    }
    let original = source.as_ref().to_nurbs()?;
    if original.spans().count() != 1 {
        return Err(GeometryError::InvalidPolyCurve {
            context: "Match currently requires a single-span source curve",
        });
    }
    let matched_controls = continuity_control_count(continuity);
    let preserved_controls = preserve_control_count(preserve);
    let desired_degree = original
        .degree()
        .max(matched_controls + preserved_controls - 1);
    let elevated = original.try_change_degree(desired_degree, false)?;
    let mut controls = elevated.control_points().to_vec();
    let last = controls.len() - 1;
    let endpoint_index = if source_at_end { last } else { 0 };
    let adjacent_index = if source_at_end { last - 1 } else { 1 };
    let second_index = if source_at_end {
        last.saturating_sub(2)
    } else {
        2
    };
    let endpoint = controls[endpoint_index].point();
    let handle = endpoint.distance_to(controls[adjacent_index].point())?;
    let reference_curve = reference.as_ref();
    let reference_parameter = if reference_at_end {
        *reference_curve.domain().end()
    } else {
        *reference_curve.domain().start()
    };
    let reference_side = if reference_at_end {
        ParameterSide::Left
    } else {
        ParameterSide::Right
    };
    let sample =
        reference_curve.evaluate_with_tangent_on_side(reference_parameter, reference_side)?;
    let mut desired_tangent = sample.tangent();
    if source_at_end == reference_at_end {
        desired_tangent = desired_tangent.opposite();
    }
    // Two selected starts (or two selected ends) must point in opposite
    // natural directions at the join; unlike ends point the same way.
    controls[endpoint_index] =
        WeightedPoint3::try_new(sample.point(), controls[endpoint_index].weight())?;
    if continuity != CurveBlendContinuity::Position {
        if handle == 0.0 {
            return Err(GeometryError::Degenerate {
                context: "Match source endpoint handle",
            });
        }
        let a = controls[adjacent_index].weight() / controls[endpoint_index].weight();
        let b = if continuity == CurveBlendContinuity::Curvature {
            controls[second_index].weight() / controls[endpoint_index].weight()
        } else {
            1.0
        };
        if a == 0.0 || b == 0.0 {
            return Err(GeometryError::Degenerate {
                context: "Match endpoint weights",
            });
        }
        let direction_sign = if source_at_end { -1.0 } else { 1.0 };
        let offset = desired_tangent
            .as_vector()
            .scaled(direction_sign * a.signum() * handle)?;
        let adjacent = sample.point().translated(offset)?;
        controls[adjacent_index] =
            WeightedPoint3::try_new(adjacent, controls[adjacent_index].weight())?;
        if continuity == CurveBlendContinuity::Curvature {
            let degree = desired_degree as Real;
            let tangential_coefficient = 2.0 * (degree * a * a - a) / ((degree - 1.0) * b);
            let curvature_coefficient = degree * a * a * handle * handle / ((degree - 1.0) * b);
            let curvature = reference_curve.curvature_vector(reference_parameter)?;
            let second = sample
                .point()
                .translated(offset.scaled(tangential_coefficient)?)?
                .translated(curvature.scaled(curvature_coefficient)?)?;
            controls[second_index] =
                WeightedPoint3::try_new(second, controls[second_index].weight())?;
        }
    }
    let matched =
        NurbsCurve::try_new_rational(desired_degree, controls, elevated.knots().to_vec())?;
    let report = if source_at_end {
        crate::curve_end_continuity(
            CurveRef::NurbsCurve(&matched),
            true,
            reference_curve,
            reference_at_end,
            tolerance,
        )?
    } else {
        crate::curve_end_continuity(
            CurveRef::NurbsCurve(&matched),
            false,
            reference_curve,
            reference_at_end,
            tolerance,
        )?
    };
    let achieved = match continuity {
        CurveBlendContinuity::Position => report.gap <= tolerance.absolute(),
        CurveBlendContinuity::Tangency => matches!(
            report.level,
            crate::CurveContinuityLevel::Tangency | crate::CurveContinuityLevel::CurvatureOrHigher
        ),
        CurveBlendContinuity::Curvature => {
            report.level == crate::CurveContinuityLevel::CurvatureOrHigher
        }
    };
    if !achieved {
        return Err(GeometryError::Degenerate {
            context: "Match continuity could not be reached",
        });
    }
    Ok(matched)
}

const fn continuity_control_count(continuity: CurveBlendContinuity) -> usize {
    match continuity {
        CurveBlendContinuity::Position => 1,
        CurveBlendContinuity::Tangency => 2,
        CurveBlendContinuity::Curvature => 3,
    }
}

const fn preserve_control_count(preserve: CurveMatchPreserveEnd) -> usize {
    match preserve {
        CurveMatchPreserveEnd::None => 0,
        CurveMatchPreserveEnd::Position => 1,
        CurveMatchPreserveEnd::Tangency => 2,
        CurveMatchPreserveEnd::Curvature => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CircularArc3, LineSegment, Point3};

    fn point(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.0).unwrap()
    }

    fn reference() -> Curve3 {
        Curve3::Arc(
            CircularArc3::try_from_three_points(
                point(4.0, 1.0),
                point(5.0, 2.0),
                point(6.0, 1.0),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        )
    }

    #[test]
    fn cubic_g2_preserving_far_curvature_matches_live_rhino_controls() {
        let source = Curve3::NurbsCurve(
            NurbsCurve::try_new(
                3,
                [0.0, 1.0, 2.0, 3.0].map(|x| point(x, 0.0)).to_vec(),
                vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            )
            .unwrap(),
        );
        let matched = try_match_curve_end(
            &source,
            false,
            &reference(),
            false,
            CurveBlendContinuity::Curvature,
            CurveMatchPreserveEnd::Curvature,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(matched.degree(), 5);
        let points = matched
            .control_points()
            .iter()
            .map(|control| control.point().to_array())
            .collect::<Vec<_>>();
        let expected = [
            [4.0, 1.0, 0.0],
            [4.0, 0.4, 0.0],
            [4.45, -0.2, 0.0],
            [1.8, 0.0, 0.0],
            [2.4, 0.0, 0.0],
            [3.0, 0.0, 0.0],
        ];
        for (actual, expected) in points.into_iter().zip(expected) {
            for (a, e) in actual.into_iter().zip(expected) {
                assert!((a - e).abs() < 1e-12, "{a} vs {e}");
            }
        }
    }

    #[test]
    fn line_match_changes_degree_only_for_needed_constraints() {
        let source = Curve3::Line(
            LineSegment::try_new(point(0.0, 0.0), point(3.0, 0.0), Tolerance::DEFAULT).unwrap(),
        );
        let matched = try_match_curve_end(
            &source,
            false,
            &reference(),
            false,
            CurveBlendContinuity::Tangency,
            CurveMatchPreserveEnd::None,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(matched.degree(), 1);
        assert_eq!(
            matched.control_points()[1].point().to_array(),
            [4.0, -2.0, 0.0]
        );
    }
}
