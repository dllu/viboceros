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

#[derive(Clone, Copy)]
enum CurvatureControlRule {
    SingleSpan,
    OneSidedMultiSpan,
    AverageMultiSpan,
    PreparedFiveControl(Real),
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
        if source_single_span {
            CurvatureControlRule::SingleSpan
        } else {
            CurvatureControlRule::OneSidedMultiSpan
        },
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
            CurvatureControlRule::SingleSpan,
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
            CurvatureControlRule::SingleSpan,
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
        first_nurbs.spans().count() == 1,
        if first_nurbs.spans().count() == 1 {
            CurvatureControlRule::SingleSpan
        } else {
            CurvatureControlRule::AverageMultiSpan
        },
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
        if second_nurbs.spans().count() == 1 {
            CurvatureControlRule::SingleSpan
        } else {
            CurvatureControlRule::AverageMultiSpan
        },
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
    curvature_rule: CurvatureControlRule,
) -> Result<NurbsCurve, GeometryError> {
    if require_single_span && original.spans().count() != 1 {
        return Err(GeometryError::InvalidPolyCurve {
            context: "Match currently requires a single-span source curve",
        });
    }
    let matched_controls = continuity_control_count(continuity);
    let preserved_controls = preserve_control_count(preserve);
    // A short multi-span source needs disjoint control sets for the edited and
    // preserved ends. The five-control G2/G2 cases use the uniform Greville
    // preparation observed in Rhino. Other short curves are refined inside
    // the selected end span without changing their geometry before the match.
    let mut effective_curvature_rule = curvature_rule;
    let prepared = if require_single_span {
        original.clone()
    } else if original.degree() == 2
        && original.control_points().len() < matched_controls + preserved_controls
        && continuity == CurveBlendContinuity::Curvature
        && preserve == CurveMatchPreserveEnd::Curvature
    {
        let interpolant = crate::curve_rebuild::interpolate_affine_greville(original, 6)?;
        if at_end {
            interpolant.try_reparameterized(-4.0..=0.0)?
        } else {
            interpolant
        }
    } else if original.degree() == 3
        && original.control_points().len() == 5
        && continuity == CurveBlendContinuity::Curvature
        && preserve == CurveMatchPreserveEnd::Curvature
    {
        match prepare_five_control_g2(original, at_end)? {
            Some((prepared, coefficient)) => {
                if let CurvatureControlRule::OneSidedMultiSpan = curvature_rule {
                    effective_curvature_rule =
                        CurvatureControlRule::PreparedFiveControl(coefficient);
                }
                prepared
            }
            None => refine_for_disjoint_end_controls(
                original,
                at_end,
                matched_controls + preserved_controls,
            )?,
        }
    } else {
        refine_for_disjoint_end_controls(original, at_end, matched_controls + preserved_controls)?
    };
    let desired_degree = if require_single_span {
        original
            .degree()
            .max(matched_controls + preserved_controls - 1)
    } else {
        prepared.degree()
    };
    if continuity == CurveBlendContinuity::Curvature && desired_degree < 2 {
        return Err(GeometryError::InvalidPolyCurve {
            context: "Match curvature requires at least a quadratic source span",
        });
    }
    let elevated = prepared.try_change_degree(desired_degree, false)?;
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
            let zero_tangential_coefficient = || {
                a / b * (1.0 + knot_ratio + 2.0 * degree * (a - 1.0) * knot_ratio / (degree - 1.0))
            };
            let tangential_coefficient = match effective_curvature_rule {
                CurvatureControlRule::PreparedFiveControl(coefficient) => coefficient,
                CurvatureControlRule::SingleSpan => {
                    2.0 * (degree * a * a - a) / ((degree - 1.0) * b)
                }
                CurvatureControlRule::OneSidedMultiSpan if b == 1.0 => {
                    // Captured Rhino cases use the curvature-derived projection
                    // while it differs by at most 10% of the mean magnitude;
                    // otherwise they retain the original second control's
                    // projection onto the endpoint handle.
                    let first =
                        endpoint.vector_to(elevated.control_points()[adjacent_index].point())?;
                    let second =
                        endpoint.vector_to(elevated.control_points()[second_index].point())?;
                    let original_projection = second.dot(first)? / (handle * handle);
                    let curvature_projection = zero_tangential_coefficient();
                    if curvature_projection.is_finite()
                        && (curvature_projection - original_projection).abs()
                            <= 0.05 * curvature_projection.abs() + 0.05 * original_projection.abs()
                    {
                        curvature_projection
                    } else {
                        original_projection
                    }
                }
                CurvatureControlRule::OneSidedMultiSpan
                | CurvatureControlRule::AverageMultiSpan => zero_tangential_coefficient(),
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

/// Rhino's five-control cubic G2/G2 path first interpolates a six-control
/// uniform polynomial at affine Greville stations. Its handle length at the
/// untouched end equals the corresponding interpolant handle length. The far
/// second-control tangential offset comes from the source's homogeneous XYZ
/// control-distance ratio and the curvature-derived transverse offset.
fn prepare_five_control_g2(
    original: &NurbsCurve,
    at_end: bool,
) -> Result<Option<(NurbsCurve, Real)>, GeometryError> {
    let interpolant = crate::curve_rebuild::interpolate_affine_greville(original, 6)?;
    let selected_index = if at_end { 4 } else { 1 };
    let selected_endpoint = if at_end { 5 } else { 0 };
    let selected_handle = interpolant.control_points()[selected_endpoint]
        .point()
        .distance_to(interpolant.control_points()[selected_index].point())?;
    let source_controls = original.control_points();
    let (endpoint, adjacent, second, parameter) = if at_end {
        (
            source_controls[4],
            source_controls[3],
            source_controls[2],
            *original.domain().end(),
        )
    } else {
        (
            source_controls[0],
            source_controls[1],
            source_controls[2],
            *original.domain().start(),
        )
    };
    let adjacent_distance = homogeneous_spatial_distance(endpoint, adjacent)?;
    if selected_handle == 0.0 || adjacent_distance == 0.0 {
        return Ok(None);
    }
    let radial =
        selected_handle * homogeneous_spatial_distance(endpoint, second)? / adjacent_distance;
    let curvature = CurveRef::NurbsCurve(original).curvature_vector(parameter)?;
    let transverse = curvature
        .scaled(3.0 * selected_handle * selected_handle)?
        .length()?;
    if !radial.is_finite() {
        return Ok(None);
    }
    let radial_coefficient = radial_tangential_distance(radial, transverse) / selected_handle;
    if !radial_coefficient.is_finite() {
        return Ok(None);
    }
    let selected_coefficient =
        if (radial_coefficient - 3.0).abs() <= 0.05 * (radial_coefficient.abs() + 3.0) {
            3.0
        } else {
            radial_coefficient
        };
    let first = interpolant.control_points();
    let last = first.len() - 1;
    let (endpoint_index, adjacent_index, second_index, far_at_end) = if at_end {
        (0, 1, 2, false)
    } else {
        (last, last - 1, last - 2, true)
    };
    let endpoint = first[endpoint_index].point();
    let handle = endpoint.distance_to(first[adjacent_index].point())?;
    let source_controls = original.control_points();
    let source_last = source_controls.len() - 1;
    let (source_endpoint, source_adjacent, source_second) = if far_at_end {
        (
            source_controls[source_last],
            source_controls[source_last - 1],
            source_controls[source_last - 2],
        )
    } else {
        (source_controls[0], source_controls[1], source_controls[2])
    };
    let adjacent_distance = homogeneous_spatial_distance(source_endpoint, source_adjacent)?;
    if handle == 0.0 || adjacent_distance == 0.0 {
        return Ok(None);
    }
    let second_distance = homogeneous_spatial_distance(source_endpoint, source_second)?;
    let radius = handle * (second_distance / adjacent_distance);
    let parameter = if far_at_end {
        *original.domain().end()
    } else {
        *original.domain().start()
    };
    let side = if far_at_end {
        ParameterSide::Left
    } else {
        ParameterSide::Right
    };
    let tangent = CurveRef::NurbsCurve(original)
        .evaluate_with_tangent_on_side(parameter, side)?
        .tangent()
        .as_vector();
    let inward = tangent.scaled(if far_at_end { -1.0 } else { 1.0 })?;
    let curvature = CurveRef::NurbsCurve(original).curvature_vector(parameter)?;
    // A clamped uniform cubic with three spans has endpoint knot ratio two.
    let transverse = curvature.scaled(3.0 * handle * handle)?;
    let transverse_length = transverse.length()?;
    if !radius.is_finite() {
        return Ok(None);
    }
    let tangential = radial_tangential_distance(radius, transverse_length);
    if !tangential.is_finite() {
        return Ok(None);
    }
    let mut controls = first
        .iter()
        .map(|control| control.point())
        .collect::<Vec<_>>();
    controls[adjacent_index] = endpoint.translated(inward.scaled(handle)?)?;
    controls[second_index] = endpoint
        .translated(inward.scaled(tangential)?)?
        .translated(transverse)?;
    let prepared = NurbsCurve::try_new(3, controls, interpolant.knots().to_vec())?;
    if at_end {
        Ok(Some((
            prepared.try_reparameterized(-3.0..=0.0)?,
            selected_coefficient,
        )))
    } else {
        Ok(Some((prepared, selected_coefficient)))
    }
}

fn radial_tangential_distance(radius: Real, transverse: Real) -> Real {
    // Rhino takes the absolute squared difference when the homogeneous
    // radius is smaller than the curvature offset. Splitting the square root
    // avoids overflowing either squared value at large model scales.
    let high = radius.max(transverse);
    if high == 0.0 {
        return 0.0;
    }
    let low = radius.min(transverse);
    (radius - transverse).abs().sqrt() * high.sqrt() * (1.0 + low / high).sqrt()
}

fn homogeneous_spatial_distance(
    first: WeightedPoint3,
    second: WeightedPoint3,
) -> Result<Real, GeometryError> {
    let a = first.point();
    let b = second.point();
    Vector3::try_new(
        a.x() * first.weight() - b.x() * second.weight(),
        a.y() * first.weight() - b.y() * second.weight(),
        a.z() * first.weight() - b.z() * second.weight(),
    )?
    .length()
}

fn refine_for_disjoint_end_controls(
    original: &NurbsCurve,
    at_end: bool,
    required_controls: usize,
) -> Result<NurbsCurve, GeometryError> {
    let mut refined = original.clone();
    while refined.control_points().len() < required_controls {
        let knots = refined.knots();
        let start = *refined.domain().start();
        let end = *refined.domain().end();
        let interval = if at_end {
            knots
                .iter()
                .rev()
                .copied()
                .find(|knot| *knot < end)
                .map(|previous| (previous, end))
        } else {
            knots
                .iter()
                .copied()
                .find(|knot| *knot > start)
                .map(|next| (start, next))
        }
        .ok_or(GeometryError::Degenerate {
            context: "Match source has no refinable end span",
        })?;
        let parameter = interval.0.midpoint(interval.1);
        if parameter <= interval.0 || parameter >= interval.1 {
            return Err(GeometryError::Degenerate {
                context: "Match source end span is too narrow to refine",
            });
        }
        refined = refined.try_insert_knot(parameter, 1)?;
    }
    Ok(refined)
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
    fn multispan_g2_rational_tangential_choice_matches_live_rhino_controls() {
        for (second_x, expected_y) in [
            (2.0, -1.171_428_571_428_570_8),
            (2.4, -1.171_428_571_428_570_8),
            (2.41, -1.41),
        ] {
            let source = Curve3::NurbsCurve(
                NurbsCurve::try_new_rational(
                    3,
                    [
                        (0.0, 0.0, 1.0),
                        (1.0, 0.0, 0.8),
                        (second_x, 1.0, 1.0),
                        (3.0, 1.0, 0.7),
                        (4.0, 0.0, 1.0),
                    ]
                    .into_iter()
                    .map(|(x, y, weight)| WeightedPoint3::try_new(point(x, y), weight).unwrap())
                    .collect(),
                    vec![0.0, 0.0, 0.0, 0.0, 0.7, 3.0, 3.0, 3.0, 3.0],
                )
                .unwrap(),
            );
            let matched = try_match_curve_end(
                &source,
                false,
                &reference(),
                false,
                CurveBlendContinuity::Curvature,
                CurveMatchPreserveEnd::None,
                Tolerance::DEFAULT,
            )
            .unwrap();
            let second = matched.control_points()[2].point();
            assert!((second.x() - 8.114_285_714_285_721).abs() < 1e-11);
            assert!((second.y() - expected_y).abs() < 1e-11);
        }
    }

    #[test]
    fn multispan_g2_preserves_disjoint_far_curvature_controls() {
        let source = Curve3::NurbsCurve(
            NurbsCurve::try_new(
                3,
                [
                    point(0.0, 0.0),
                    point(1.0, 0.0),
                    point(2.0, 1.0),
                    point(3.0, 1.0),
                    point(4.0, 1.0),
                    point(5.0, 0.0),
                ]
                .to_vec(),
                vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0, 3.0, 3.0],
            )
            .unwrap(),
        );
        for (at_end, expected) in [
            (
                false,
                [
                    [4.0, 1.0],
                    [4.0, 0.0],
                    [7.0, -1.0],
                    [3.0, 1.0],
                    [4.0, 1.0],
                    [5.0, 0.0],
                ],
            ),
            (
                true,
                [
                    [0.0, 0.0],
                    [1.0, 0.0],
                    [2.0, 1.0],
                    [10.0, -1.121_320_343_559_642_4],
                    [4.0, -0.414_213_562_373_095_15],
                    [4.0, 1.0],
                ],
            ),
        ] {
            let matched = try_match_curve_end(
                &source,
                at_end,
                &reference(),
                false,
                CurveBlendContinuity::Curvature,
                CurveMatchPreserveEnd::Curvature,
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert_eq!(matched.degree(), 3);
            assert_eq!(matched.knots(), source.as_ref().to_nurbs().unwrap().knots());
            for (control, expected) in matched.control_points().iter().zip(expected) {
                let point = control.point();
                assert!((point.x() - expected[0]).abs() < 1e-12);
                assert!((point.y() - expected[1]).abs() < 1e-12);
            }
            let far_parameter = if at_end { 0.0 } else { 3.0 };
            let before = source.as_ref().curvature_vector(far_parameter).unwrap();
            let after = CurveRef::NurbsCurve(&matched)
                .curvature_vector(far_parameter)
                .unwrap();
            assert!((before.x() - after.x()).abs() < 1e-12);
            assert!((before.y() - after.y()).abs() < 1e-12);
        }
    }

    #[test]
    fn five_control_multispan_g2_prepares_and_preserves_far_curvature() {
        for (at_end, rational, uneven_knots) in [
            (false, false, false),
            (true, false, false),
            (false, true, true),
            (true, true, true),
        ] {
            let weights = if rational {
                [1.0, 0.8, 1.2, 0.7, 1.0]
            } else {
                [1.0; 5]
            };
            let interior = if uneven_knots { 0.7 } else { 1.0 };
            let end = if uneven_knots { 3.0 } else { 2.0 };
            let source = Curve3::NurbsCurve(
                NurbsCurve::try_new_rational(
                    3,
                    [
                        point(0.0, 0.0),
                        point(1.0, 0.0),
                        point(2.0, 1.0),
                        point(3.0, 1.0),
                        point(4.0, 0.0),
                    ]
                    .into_iter()
                    .zip(weights)
                    .map(|(point, weight)| WeightedPoint3::try_new(point, weight).unwrap())
                    .collect(),
                    vec![0.0, 0.0, 0.0, 0.0, interior, end, end, end, end],
                )
                .unwrap(),
            );
            let matched = try_match_curve_end(
                &source,
                at_end,
                &reference(),
                false,
                CurveBlendContinuity::Curvature,
                CurveMatchPreserveEnd::Curvature,
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert_eq!(matched.control_points().len(), 6);
            let far = if at_end { 0.0 } else { end };
            let matched_far = if at_end {
                *matched.domain().start()
            } else {
                *matched.domain().end()
            };
            let original_curvature = source.as_ref().curvature_vector(far).unwrap();
            let matched_curvature = CurveRef::NurbsCurve(&matched)
                .curvature_vector(matched_far)
                .unwrap();
            assert!(
                (original_curvature.x() - matched_curvature.x()).abs() < 1e-10,
                "at_end={at_end} rational={rational} uneven={uneven_knots} before={original_curvature:?} after={matched_curvature:?}"
            );
            assert!((original_curvature.y() - matched_curvature.y()).abs() < 1e-10);
            assert!((original_curvature.z() - matched_curvature.z()).abs() < 1e-10);
            let original_point = source.as_ref().evaluate(far).unwrap();
            let matched_point = matched.evaluate(matched_far).unwrap();
            assert!(original_point.distance_to(matched_point).unwrap() < 1e-10);
            let expected_controls: &[(usize, [Real; 2])] = match (at_end, rational, uneven_knots) {
                (false, false, false) => &[
                    (1, [4.0, 1.0 / 3.0]),
                    (3, [2.747_375_722_722_700_3, 0.808_179_832_832_856_2]),
                    (4, [10.0 / 3.0, 2.0 / 3.0]),
                ],
                (true, false, false) => &[
                    (1, [2.0 / 3.0, 0.0]),
                    (2, [1.422_916_497_207_299_2, 4.0 / 9.0]),
                ],
                (false, true, true) => &[
                    (1, [4.0, -0.356_607_679_527_325_5]),
                    (3, [3.558_214_165_460_0, 0.059_386_796_179_0]),
                    (4, [3.680_884_775_481_0, 0.319_115_224_519_0]),
                ],
                _ => &[],
            };
            for (index, expected) in expected_controls {
                let actual = matched.control_points()[*index].point();
                assert!((actual.x() - expected[0]).abs() < 1e-10);
                assert!((actual.y() - expected[1]).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn average_five_control_multispan_g2_preserves_both_far_curvatures() {
        let first = Curve3::NurbsCurve(
            NurbsCurve::try_new(
                3,
                [
                    point(0.0, 0.0),
                    point(1.0, 0.0),
                    point(2.0, 1.0),
                    point(3.0, 1.0),
                    point(4.0, 0.0),
                ]
                .to_vec(),
                vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0],
            )
            .unwrap(),
        );
        let second = Curve3::NurbsCurve(
            NurbsCurve::try_new(
                3,
                [
                    point(5.0, -1.0),
                    point(6.0, -1.0),
                    point(7.0, 0.0),
                    point(8.0, 0.0),
                    point(9.0, -1.0),
                ]
                .to_vec(),
                vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0],
            )
            .unwrap(),
        );
        let (matched_first, matched_second) = try_average_match_curve_ends(
            &first,
            false,
            &second,
            true,
            CurveBlendContinuity::Curvature,
            CurveMatchPreserveEnd::Curvature,
            Tolerance::DEFAULT,
        )
        .unwrap();
        for (original, matched, far) in [
            (&first, &matched_first, 2.0),
            (&second, &matched_second, 0.0),
        ] {
            assert_eq!(matched.control_points().len(), 6);
            let before = original.as_ref().curvature_vector(far).unwrap();
            let matched_far = if far == 0.0 {
                *matched.domain().start()
            } else {
                *matched.domain().end()
            };
            let after = CurveRef::NurbsCurve(matched)
                .curvature_vector(matched_far)
                .unwrap();
            assert!((before.x() - after.x()).abs() < 1e-10);
            assert!((before.y() - after.y()).abs() < 1e-10);
        }
    }

    #[test]
    fn translated_rational_five_control_g2_matches_rhino_end_controls() {
        for (shift, expected_second_y, expected_far_second) in [
            (
                -10.0,
                0.434_973_418_047_429_4,
                [-7.841_560_011_392_847, 1.157_601_399_958_361_2],
            ),
            (
                10.0,
                -1.098_863_453_792_275,
                [13.343_837_707_420_112, -0.027_796_318_854_605_595],
            ),
        ] {
            let source = Curve3::NurbsCurve(
                NurbsCurve::try_new_rational(
                    3,
                    [
                        (0.0, 0.0, 1.0),
                        (1.0, 0.0, 0.8),
                        (2.0, 1.0, 1.2),
                        (3.0, 1.0, 0.7),
                        (4.0, 0.0, 1.0),
                    ]
                    .into_iter()
                    .map(|(x, y, weight)| {
                        WeightedPoint3::try_new(point(x + shift, y), weight).unwrap()
                    })
                    .collect(),
                    vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0],
                )
                .unwrap(),
            );
            let reference = Curve3::Arc(
                CircularArc3::try_from_three_points(
                    point(4.0 + shift, 1.0),
                    point(5.0 + shift, 2.0),
                    point(6.0 + shift, 1.0),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            );
            let matched = try_match_curve_end(
                &source,
                false,
                &reference,
                false,
                CurveBlendContinuity::Curvature,
                CurveMatchPreserveEnd::Curvature,
                Tolerance::DEFAULT,
            )
            .unwrap();
            let second = matched.control_points()[2].point();
            let far_second = matched.control_points()[3].point();
            assert!((second.y() - expected_second_y).abs() < 1e-10);
            assert!((far_second.x() - expected_far_second[0]).abs() < 1e-10);
            assert!((far_second.y() - expected_far_second[1]).abs() < 1e-10);
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
