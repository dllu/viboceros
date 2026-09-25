//! End matching for open NURBS curves, retaining their rational weights.

use crate::{
    Curve3, CurveBlendContinuity, CurveRef, GeometryError, NurbsCurve, ParameterSide, Point3, Real,
    Tolerance, UnitVector3, Vector3, WeightedPoint3,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveMatchPreserveEnd {
    None,
    Position,
    Tangency,
    Curvature,
}

struct MatchTarget {
    point: Point3,
    tangent: Option<UnitVector3>,
    curvature: Option<Vector3>,
}

/// Changes the selected end of one open curve to meet another with G0, G1, or
/// G2 continuity. Multi-span position matching first trims toward the nearest
/// location to the reference end. Degree elevation leaves a single-span
/// rational locus unchanged before its end controls are modified.
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
    let original = source.as_ref().to_nurbs()?;
    let original = if continuity == CurveBlendContinuity::Position && original.spans().count() > 1 {
        trim_toward_target(source.as_ref(), source_at_end, sample.point(), tolerance)?
    } else {
        original
    };
    let source_single_span = original.spans().count() == 1;
    if !source_single_span
        && continuity == CurveBlendContinuity::Curvature
        && preserve == CurveMatchPreserveEnd::Curvature
    {
        return Err(GeometryError::InvalidPolyCurve {
            context: "Match multi-span curvature with far curvature preservation needs knot edits",
        });
    }
    let matched = match_end_to_target(
        &original,
        source_at_end,
        MatchTarget {
            point: sample.point(),
            tangent: Some(desired_tangent),
            curvature: (continuity == CurveBlendContinuity::Curvature)
                .then(|| reference_curve.curvature_vector(reference_parameter))
                .transpose()?,
        },
        continuity,
        preserve,
        source_single_span,
        true,
    )?;
    require_continuity(
        &matched,
        source_at_end,
        reference_curve,
        reference_at_end,
        continuity,
        tolerance,
    )?;
    Ok(matched)
}

/// Moves both selected ends to their midpoint. Position matching first trims
/// each curve to the nearest location to that midpoint, then restores its
/// original domain before moving the endpoint control.
/// For G1 and G2, bisects the original tangents.
/// For G2, the target curvature vector is the average of the original endpoint
/// vectors. Both curves retain their original end-handle lengths.
pub fn try_average_match_curve_ends(
    first: &Curve3,
    first_at_end: bool,
    second: &Curve3,
    second_at_end: bool,
    continuity: CurveBlendContinuity,
    preserve: CurveMatchPreserveEnd,
    tolerance: Tolerance,
) -> Result<(NurbsCurve, NurbsCurve), GeometryError> {
    if first.as_ref().is_closed()? || second.as_ref().is_closed()? {
        return Err(GeometryError::InvalidPolyCurve {
            context: "Match requires open curves",
        });
    }
    let first_curve = first.as_ref();
    let second_curve = second.as_ref();
    let first_parameter = if first_at_end {
        *first_curve.domain().end()
    } else {
        *first_curve.domain().start()
    };
    let second_parameter = if second_at_end {
        *second_curve.domain().end()
    } else {
        *second_curve.domain().start()
    };
    let first_side = if first_at_end {
        ParameterSide::Left
    } else {
        ParameterSide::Right
    };
    let second_side = if second_at_end {
        ParameterSide::Left
    } else {
        ParameterSide::Right
    };
    let first_point = if first_at_end {
        first_curve.end_point()?
    } else {
        first_curve.start_point()?
    };
    let second_point = if second_at_end {
        second_curve.end_point()?
    } else {
        second_curve.start_point()?
    };
    let midpoint = first_point.midpoint(second_point)?;
    if continuity == CurveBlendContinuity::Position {
        let first_nurbs = trim_toward_target(first_curve, first_at_end, midpoint, tolerance)?;
        let second_nurbs = trim_toward_target(second_curve, second_at_end, midpoint, tolerance)?;
        let first_output = match_end_to_target(
            &first_nurbs,
            first_at_end,
            MatchTarget {
                point: midpoint,
                tangent: None,
                curvature: None,
            },
            continuity,
            preserve,
            first_nurbs.spans().count() == 1,
            false,
        )?;
        let second_output = match_end_to_target(
            &second_nurbs,
            second_at_end,
            MatchTarget {
                point: midpoint,
                tangent: None,
                curvature: None,
            },
            continuity,
            preserve,
            second_nurbs.spans().count() == 1,
            false,
        )?;
        require_continuity(
            &first_output,
            first_at_end,
            CurveRef::NurbsCurve(&second_output),
            second_at_end,
            continuity,
            tolerance,
        )?;
        return Ok((first_output, second_output));
    }
    let first_sample = first_curve.evaluate_with_tangent_on_side(first_parameter, first_side)?;
    let second_sample =
        second_curve.evaluate_with_tangent_on_side(second_parameter, second_side)?;
    let second_direction = if first_at_end == second_at_end {
        second_sample.tangent().opposite()
    } else {
        second_sample.tangent()
    };
    let tangent_sum = Vector3::try_new(
        first_sample.tangent().x() + second_direction.x(),
        first_sample.tangent().y() + second_direction.y(),
        first_sample.tangent().z() + second_direction.z(),
    )?;
    let tangent = tangent_sum.normalized(tolerance)?;
    let first_nurbs = first_curve.to_nurbs()?;
    let second_nurbs = second_curve.to_nurbs()?;
    if continuity == CurveBlendContinuity::Curvature
        && second_nurbs.spans().count() > 1
        && matches!(
            preserve,
            CurveMatchPreserveEnd::Tangency | CurveMatchPreserveEnd::Curvature
        )
    {
        return Err(GeometryError::InvalidPolyCurve {
            context: "average Match curvature with a preserved multi-span opposite end",
        });
    }
    let curvature = if continuity == CurveBlendContinuity::Curvature {
        let first_curvature = first_curve.curvature_vector(first_parameter)?;
        let second_curvature = second_curve.curvature_vector(second_parameter)?;
        Some(Vector3::try_new(
            first_curvature.x().midpoint(second_curvature.x()),
            first_curvature.y().midpoint(second_curvature.y()),
            first_curvature.z().midpoint(second_curvature.z()),
        )?)
    } else {
        None
    };
    let first_output = match_end_to_target(
        &first_nurbs,
        first_at_end,
        MatchTarget {
            point: midpoint,
            tangent: Some(tangent),
            curvature,
        },
        continuity,
        preserve,
        true,
        false,
    )?;
    let second_tangent = if first_at_end == second_at_end {
        tangent.opposite()
    } else {
        tangent
    };
    let second_output = match_end_to_target(
        &second_nurbs,
        second_at_end,
        MatchTarget {
            point: midpoint,
            tangent: Some(second_tangent),
            curvature,
        },
        continuity,
        preserve,
        second_nurbs.spans().count() == 1,
        false,
    )?;
    require_continuity(
        &first_output,
        first_at_end,
        CurveRef::NurbsCurve(&second_output),
        second_at_end,
        continuity,
        tolerance,
    )?;
    Ok((first_output, second_output))
}

fn trim_toward_target(
    curve: CurveRef<'_>,
    selected_at_end: bool,
    midpoint: Point3,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    let original = curve.to_nurbs()?;
    let domain = original.domain();
    let closest = curve.closest_parameter(midpoint, tolerance)?;
    if closest <= *domain.start() || closest >= *domain.end() {
        return Ok(original);
    }
    let interval = if selected_at_end {
        *domain.start()..=closest
    } else {
        closest..=*domain.end()
    };
    original.try_trimmed(interval)?.try_reparameterized(domain)
}

fn match_end_to_target(
    original: &NurbsCurve,
    at_end: bool,
    target: MatchTarget,
    continuity: CurveBlendContinuity,
    preserve: CurveMatchPreserveEnd,
    require_single_span: bool,
    one_sided_match: bool,
) -> Result<NurbsCurve, GeometryError> {
    if require_single_span && original.spans().count() != 1 {
        return Err(GeometryError::InvalidPolyCurve {
            context: "Match currently requires a single-span source curve",
        });
    }
    let matched_controls = continuity_control_count(continuity);
    let preserved_controls = preserve_control_count(preserve);
    if !require_single_span
        && original.control_points().len() < matched_controls + preserved_controls
    {
        return Err(GeometryError::InvalidPolyCurve {
            context: "Match requires more controls to preserve the opposite end",
        });
    }
    let desired_degree = if require_single_span {
        original
            .degree()
            .max(matched_controls + preserved_controls - 1)
    } else {
        original.degree()
    };
    if continuity == CurveBlendContinuity::Curvature && desired_degree < 2 {
        return Err(GeometryError::InvalidPolyCurve {
            context: "Match curvature requires at least a quadratic source span",
        });
    }
    let elevated = original.try_change_degree(desired_degree, false)?;
    let mut controls = elevated.control_points().to_vec();
    let last = controls.len() - 1;
    let endpoint_index = if at_end { last } else { 0 };
    let adjacent_index = if at_end { last - 1 } else { 1 };
    let second_index = if at_end { last.saturating_sub(2) } else { 2 };
    let endpoint = controls[endpoint_index].point();
    let handle = endpoint.distance_to(controls[adjacent_index].point())?;
    controls[endpoint_index] =
        WeightedPoint3::try_new(target.point, controls[endpoint_index].weight())?;
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
        let direction_sign = if at_end { -1.0 } else { 1.0 };
        let offset = target
            .tangent
            .ok_or(GeometryError::Degenerate {
                context: "Match target tangent",
            })?
            .as_vector()
            .scaled(direction_sign * a.signum() * handle)?;
        let adjacent = target.point.translated(offset)?;
        controls[adjacent_index] =
            WeightedPoint3::try_new(adjacent, controls[adjacent_index].weight())?;
        if continuity == CurveBlendContinuity::Curvature {
            let degree = desired_degree as Real;
            let knot_ratio = endpoint_curvature_knot_ratio(&elevated, at_end)?;
            let tangential_coefficient = if require_single_span || !one_sided_match {
                2.0 * (degree * a * a - a) / ((degree - 1.0) * b)
            } else if b == 1.0 {
                // Rhino keeps the original second control's projection onto
                // the endpoint handle when their weights are equal.
                let first =
                    endpoint.vector_to(elevated.control_points()[adjacent_index].point())?;
                let second = endpoint.vector_to(elevated.control_points()[second_index].point())?;
                second.dot(first)? / (handle * handle)
            } else {
                // For unequal endpoint/second weights Rhino instead chooses
                // zero tangential second derivative at the new end.
                a / b * (1.0 + knot_ratio + 2.0 * degree * (a - 1.0) * knot_ratio / (degree - 1.0))
            };
            let curvature_coefficient =
                degree * a * a * handle * handle / ((degree - 1.0) * b) * knot_ratio;
            let curvature = target.curvature.ok_or(GeometryError::Degenerate {
                context: "Match target curvature",
            })?;
            let second = target
                .point
                .translated(offset.scaled(tangential_coefficient)?)?
                .translated(curvature.scaled(curvature_coefficient)?)?;
            controls[second_index] =
                WeightedPoint3::try_new(second, controls[second_index].weight())?;
        }
    }
    let matched =
        NurbsCurve::try_new_rational(desired_degree, controls, elevated.knots().to_vec())?;
    Ok(matched)
}

/// Ratio of the second and first endpoint derivative knot denominators.
/// A single Bézier span has ratio one; a nonuniform B-spline can have a
/// different second-derivative scale at the same endpoint handle length.
fn endpoint_curvature_knot_ratio(curve: &NurbsCurve, at_end: bool) -> Result<Real, GeometryError> {
    let knots = curve.knots();
    let degree = curve.degree();
    let (first, second) = if at_end {
        let end = *curve.domain().end();
        (
            end - knots[knots.len() - degree - 2],
            end - knots[knots.len() - degree - 3],
        )
    } else {
        let start = *curve.domain().start();
        (knots[degree + 1] - start, knots[degree + 2] - start)
    };
    let ratio = second / first;
    if !ratio.is_finite() || ratio <= 0.0 {
        return Err(GeometryError::Degenerate {
            context: "Match endpoint knot interval",
        });
    }
    Ok(ratio)
}

fn require_continuity(
    matched: &NurbsCurve,
    matched_at_end: bool,
    reference: CurveRef<'_>,
    reference_at_end: bool,
    continuity: CurveBlendContinuity,
    tolerance: Tolerance,
) -> Result<(), GeometryError> {
    let matched_endpoint = if matched_at_end {
        CurveRef::NurbsCurve(matched).end_point()?
    } else {
        CurveRef::NurbsCurve(matched).start_point()?
    };
    let reference_endpoint = if reference_at_end {
        reference.end_point()?
    } else {
        reference.start_point()?
    };
    if matched_endpoint.distance_to(reference_endpoint)? > tolerance.absolute() {
        return Err(GeometryError::Degenerate {
            context: "Match continuity could not be reached",
        });
    }
    if continuity == CurveBlendContinuity::Position {
        return Ok(());
    }
    let matched_parameter = if matched_at_end {
        *matched.domain().end()
    } else {
        *matched.domain().start()
    };
    let reference_parameter = if reference_at_end {
        *reference.domain().end()
    } else {
        *reference.domain().start()
    };
    let matched_tangent = CurveRef::NurbsCurve(matched)
        .evaluate_with_tangent_on_side(
            matched_parameter,
            if matched_at_end {
                ParameterSide::Left
            } else {
                ParameterSide::Right
            },
        )?
        .tangent();
    let reference_tangent = reference
        .evaluate_with_tangent_on_side(
            reference_parameter,
            if reference_at_end {
                ParameterSide::Left
            } else {
                ParameterSide::Right
            },
        )?
        .tangent();
    let reference_direction = if matched_at_end == reference_at_end {
        reference_tangent.opposite()
    } else {
        reference_tangent
    };
    if matched_tangent
        .as_vector()
        .angle_to(reference_direction.as_vector())?
        > tolerance.angular()
    {
        return Err(GeometryError::Degenerate {
            context: "Match continuity could not be reached",
        });
    }
    if continuity == CurveBlendContinuity::Tangency {
        return Ok(());
    }
    let report = crate::curve_end_continuity(
        CurveRef::NurbsCurve(matched),
        matched_at_end,
        reference,
        reference_at_end,
        tolerance,
    )?;
    if report.level != crate::CurveContinuityLevel::CurvatureOrHigher {
        return Err(GeometryError::Degenerate {
            context: "Match continuity could not be reached",
        });
    }
    Ok(())
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
