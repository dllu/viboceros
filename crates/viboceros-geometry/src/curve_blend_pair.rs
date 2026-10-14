//! Endpoint blends with independent position, tangent, or curvature constraints.

use crate::{
    Curve3, EndpointInterpolation, GeometryError, NurbsCurve, ParameterSide, Point3, Real,
    Tolerance, UnitVector3, Vector3, curve_pair_support::selected_end,
};

/// Continuity imposed at one end of a blend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveBlendContinuity {
    Position,
    Tangency,
    Curvature,
}

/// Endpoint continuity and handle lengths for a curve blend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveBlendOptions {
    pub continuity: [CurveBlendContinuity; 2],
    pub handles: [Option<Real>; 2],
    /// Multipliers for the default handle lengths. An explicit handle length
    /// takes precedence at that end.
    pub bulges: [Real; 2],
    /// Requests endpoint-specific defaults even for equal continuity at the
    /// first curve's end and second curve's start. Other picks or mixed
    /// continuity always use endpoint-specific defaults.
    pub endpoint_specific: bool,
}

impl Default for CurveBlendOptions {
    fn default() -> Self {
        Self {
            continuity: [CurveBlendContinuity::Tangency; 2],
            handles: [None, None],
            bulges: [1.0, 1.0],
            endpoint_specific: false,
        }
    }
}

/// Creates the minimum-degree, single-span nonrational blend connecting
/// selected open-curve ends, from a line for G0/G0 to a quintic for G2/G2.
/// The handle lengths are model-space distances and can be adjusted independently.
pub fn try_blend_curve(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    options: CurveBlendOptions,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    try_blend_curve_with_interpolation(
        first,
        first_pick,
        second,
        second_pick,
        options,
        tolerance,
        &mut crate::try_interpolate_endpoints,
    )
}

/// Preserve Blend's endpoint selection, handle policy and domain while
/// delegating its exact minimum-degree endpoint interpolation to a backend.
pub fn try_blend_curve_with_interpolation<E: From<GeometryError>>(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    options: CurveBlendOptions,
    tolerance: Tolerance,
    interpolate: &mut impl FnMut(&EndpointInterpolation<'_>) -> Result<Vec<[Real; 4]>, E>,
) -> Result<NurbsCurve, E> {
    let first_end = selected_end(first, first_pick, tolerance)?;
    let second_end = selected_end(second, second_pick, tolerance)?;
    let endpoint_specific = options.endpoint_specific
        || options.continuity[0] != options.continuity[1]
        || !first_end
        || second_end;
    let start = endpoint_point(first, first_end)?;
    let end = endpoint_point(second, second_end)?;
    let chord = start.vector_to(end)?;
    let chord_length = chord.length()?;
    if chord_length <= tolerance.absolute() {
        return Err(GeometryError::Degenerate {
            context: "curve blend endpoints",
        }
        .into());
    }
    let default_handle = |continuity| match continuity {
        CurveBlendContinuity::Position => chord_length / 3.0,
        CurveBlendContinuity::Tangency => chord_length,
        CurveBlendContinuity::Curvature => chord_length * 0.4,
    };
    if options
        .bulges
        .iter()
        .any(|bulge| !bulge.is_finite() || *bulge <= 0.0)
    {
        return Err(GeometryError::Degenerate {
            context: "curve blend bulge",
        }
        .into());
    }
    let mut first_handle =
        options.handles[0].unwrap_or_else(|| default_handle(options.continuity[0]));
    let mut second_handle =
        options.handles[1].unwrap_or_else(|| default_handle(options.continuity[1]));
    if !first_handle.is_finite()
        || !second_handle.is_finite()
        || first_handle <= 0.0
        || second_handle <= 0.0
    {
        return Err(GeometryError::Degenerate {
            context: "curve blend handle length",
        }
        .into());
    }
    if options.continuity == [CurveBlendContinuity::Position; 2] {
        let domain_end = if endpoint_specific { 1.0 } else { chord_length };
        return NurbsCurve::try_new(1, vec![start, end], vec![0.0, 0.0, domain_end, domain_end])
            .map_err(E::from);
    }
    let degree = continuity_control_count(options.continuity[0])
        + continuity_control_count(options.continuity[1])
        - 1;
    let first_direction = match endpoint_tangent(first, first_end) {
        Ok(tangent) => Some(if first_end {
            tangent
        } else {
            tangent.opposite()
        }),
        Err(_) if options.continuity[0] == CurveBlendContinuity::Position => None,
        Err(error) => return Err(error.into()),
    };
    let second_direction = match endpoint_tangent(second, second_end) {
        Ok(tangent) => Some(if second_end {
            tangent.opposite()
        } else {
            tangent
        }),
        Err(_) if options.continuity[1] == CurveBlendContinuity::Position => None,
        Err(error) => return Err(error.into()),
    };
    if endpoint_specific
        && let (Some(first_direction), Some(second_direction)) = (first_direction, second_direction)
    {
        let chord_direction = chord.normalized_nonzero()?.as_vector();
        // The endpoint-specific Rhino blend uses the absolute chord projections
        // on both source tangent lines. The resulting endpoint speeds are
        // independent of the blend degree; divide by degree for Bézier handles.
        let a = chord_direction.dot(first_direction.as_vector())?.abs();
        let b = chord_direction.dot(second_direction.as_vector())?.abs();
        let speed_scale = 2.0 / degree as Real * chord_length;
        if options.handles[0].is_none() {
            first_handle = speed_scale * (1.4 - a + (a - b) * (0.3 + 0.5 * b));
        }
        if options.handles[1].is_none() {
            second_handle = speed_scale * (1.4 - b + (b - a) * (0.3 + 0.5 * a));
        }
    }
    if options.handles[0].is_none() {
        first_handle *= options.bulges[0];
    }
    if options.handles[1].is_none() {
        second_handle *= options.bulges[1];
    }
    if !first_handle.is_finite() || !second_handle.is_finite() {
        return Err(GeometryError::Degenerate {
            context: "curve blend handle length",
        }
        .into());
    }
    let orders = options
        .continuity
        .map(|g| (continuity_control_count(g) - 1) as u8);
    let mut targets = vec![[0.; 4]; degree + 1];
    targets[0] = [0., 0., 0., 1.];
    let xyz = chord.to_array();
    targets[degree] = [xyz[0], xyz[1], xyz[2], 1.];
    for (side, (source, at_end, direction, handle)) in [
        (first, first_end, first_direction, first_handle),
        (second, second_end, second_direction, second_handle),
    ]
    .into_iter()
    .enumerate()
    {
        if orders[side] == 0 {
            continue;
        }
        let direction = direction.expect("constrained blend end has a tangent");
        let derivative = direction
            .as_vector()
            .scaled(handle)?
            .scaled(degree as Real)?
            .to_array();
        let row = if side == 0 { 1 } else { degree - 1 };
        targets[row] = [derivative[0], derivative[1], derivative[2], 0.];
        if orders[side] == 2 {
            let speed = degree as Real * handle;
            let derivative = endpoint_curvature(source, at_end)?
                .to_array()
                .map(|c| c * speed * speed);
            let row = if side == 0 { 2 } else { degree - 2 };
            targets[row] = [derivative[0], derivative[1], derivative[2], 0.];
        }
    }
    let input = EndpointInterpolation {
        orders,
        column_count: 1,
        targets: &targets,
    };
    input.validate()?;
    let net = interpolate(&input)?;
    if net.len() != degree + 1
        || net
            .iter()
            .any(|h| h.iter().any(|v| !v.is_finite()) || (h[3] - 1.).abs() > 1e-12)
    {
        return Err(GeometryError::InvalidEndpointInterpolation {
            context: "curve backend returned invalid polynomial controls",
        }
        .into());
    }
    let controls = net
        .iter()
        .map(|h| Point3::try_new(h[0], h[1], h[2]))
        .collect::<Result<Vec<_>, _>>()?;
    let knots = std::iter::repeat_n(0., degree + 1)
        .chain(std::iter::repeat_n(1., degree + 1))
        .collect();
    let local = NurbsCurve::try_new(degree, controls, knots)?;
    for (end, &end_order) in orders.iter().enumerate() {
        let (point, first, second) = local.evaluate_with_second_derivative_on_side(
            end as Real,
            if end == 0 {
                ParameterSide::Right
            } else {
                ParameterSide::Left
            },
        )?;
        let actual = [point.to_array(), first.to_array(), second.to_array()];
        for order in 0..=usize::from(end_order) {
            let row = if end == 0 { order } else { degree - order };
            for (k, actual) in actual[order].into_iter().enumerate() {
                let target = targets[row][k];
                if (actual - target).abs() > 1e-9 * actual.abs().max(target.abs()).max(1.) {
                    return Err(GeometryError::InvalidEndpointInterpolation {
                        context: "curve backend did not preserve the requested endpoint jets",
                    }
                    .into());
                }
            }
        }
    }
    let mut controls = local
        .control_points()
        .iter()
        .map(|h| {
            let p = h.point();
            Point3::try_new(start.x() + p.x(), start.y() + p.y(), start.z() + p.z())
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Retain the original endpoint bits after the common-origin solve.
    controls[0] = start;
    controls[degree] = end;
    let mut knots = vec![0.0; degree + 1];
    knots.extend(vec![1.0; degree + 1]);
    let blend = NurbsCurve::try_new(degree, controls, knots)?;
    if endpoint_specific {
        Ok(blend)
    } else {
        blend
            .try_reparameterized(0.0..=blend.length(tolerance)?)
            .map_err(E::from)
    }
}

fn continuity_control_count(continuity: CurveBlendContinuity) -> usize {
    match continuity {
        CurveBlendContinuity::Position => 1,
        CurveBlendContinuity::Tangency => 2,
        CurveBlendContinuity::Curvature => 3,
    }
}

fn endpoint_curvature(curve: &Curve3, at_end: bool) -> Result<Vector3, GeometryError> {
    let reference = curve.as_ref();
    let parameter = if at_end {
        *reference.domain().end()
    } else {
        *reference.domain().start()
    };
    reference.curvature_vector(parameter)
}

fn endpoint_point(curve: &Curve3, at_end: bool) -> Result<Point3, GeometryError> {
    let reference = curve.as_ref();
    let parameter = if at_end {
        *reference.domain().end()
    } else {
        *reference.domain().start()
    };
    reference.evaluate(parameter)
}

fn endpoint_tangent(curve: &Curve3, at_end: bool) -> Result<UnitVector3, GeometryError> {
    let reference = curve.as_ref();
    let parameter = if at_end {
        *reference.domain().end()
    } else {
        *reference.domain().start()
    };
    let side = if at_end {
        ParameterSide::Left
    } else {
        ParameterSide::Right
    };
    Ok(reference
        .evaluate_with_tangent_on_side(parameter, side)?
        .tangent())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CircularArc3, CurveRef, LineSegment, Vector3};

    fn p(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.0).unwrap()
    }

    fn line(start: Point3, end: Point3) -> Curve3 {
        Curve3::Line(LineSegment::try_new(start, end, Tolerance::DEFAULT).unwrap())
    }

    #[test]
    fn bad_interpolation_results_and_backend_errors_preserve_the_source_curves() {
        let first = line(p(0., 0.), p(1., 0.));
        let second = line(p(4., 1.), p(4., 2.));
        let before = [first.clone(), second.clone()];
        for failure in 0..4 {
            let mut interpolate = |input: &EndpointInterpolation<'_>| {
                if failure == 3 {
                    return Err(GeometryError::InterpolationBackendFailed {
                        message: "injected failure".into(),
                    });
                }
                let mut net = crate::try_interpolate_endpoints(input)?;
                match failure {
                    0 => net.clear(),
                    1 => net[1][3] = 0.5,
                    2 => net[1][0] += 1.,
                    _ => unreachable!(),
                }
                Ok(net)
            };
            assert!(
                try_blend_curve_with_interpolation(
                    &first,
                    p(1., 0.),
                    &second,
                    p(4., 1.),
                    CurveBlendOptions::default(),
                    Tolerance::DEFAULT,
                    &mut interpolate
                )
                .is_err()
            );
            assert_eq!([first.clone(), second.clone()], before);
        }
    }

    #[test]
    fn tangent_blend_matches_both_oriented_curve_ends() {
        let first = line(p(0., 0.), p(1., 0.));
        let second = line(p(4., 1.), p(4., 2.));
        let blend = try_blend_curve(
            &first,
            p(1., 0.),
            &second,
            p(4., 1.),
            CurveBlendOptions::default(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(blend.degree(), 3);
        let start = CurveRef::NurbsCurve(&blend)
            .evaluate_with_tangent_on_side(*blend.domain().start(), ParameterSide::Right)
            .unwrap();
        let end = CurveRef::NurbsCurve(&blend)
            .evaluate_with_tangent_on_side(*blend.domain().end(), ParameterSide::Left)
            .unwrap();
        assert!(start.point().distance_to(p(1., 0.)).unwrap() < 1e-12);
        assert!(end.point().distance_to(p(4., 1.)).unwrap() < 1e-12);
        assert!(
            start
                .tangent()
                .as_vector()
                .angle_to(Vector3::try_new(1., 0., 0.).unwrap())
                .unwrap()
                < 1e-12
        );
        assert!(
            end.tangent()
                .as_vector()
                .angle_to(Vector3::try_new(0., 1., 0.).unwrap())
                .unwrap()
                < 1e-12
        );
    }

    #[test]
    fn reversed_inputs_still_make_an_outward_tangent_blend() {
        let first = line(p(1., 0.), p(0., 0.));
        let second = line(p(4., 2.), p(4., 1.));
        let blend = try_blend_curve(
            &first,
            p(1., 0.),
            &second,
            p(4., 1.),
            CurveBlendOptions {
                handles: [Some(0.5), Some(0.75)],
                ..Default::default()
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        let controls = blend.control_points();
        assert!(controls[1].point().distance_to(p(1.5, 0.)).unwrap() < 1e-12);
        assert!(controls[2].point().distance_to(p(4., 0.25)).unwrap() < 1e-12);
    }

    #[test]
    fn mixed_position_and_tangency_use_a_quadratic() {
        let first = line(p(0., -1.), p(0., 0.));
        let second = line(p(3., 1.), p(3., 2.));
        let blend = try_blend_curve(
            &first,
            p(0., 0.),
            &second,
            p(3., 1.),
            CurveBlendOptions {
                continuity: [
                    CurveBlendContinuity::Position,
                    CurveBlendContinuity::Tangency,
                ],
                handles: [Some(1.), Some(1.)],
                ..Default::default()
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        let controls = blend.control_points();
        assert_eq!(blend.degree(), 2);
        assert_eq!(controls.len(), 3);
        assert!(controls[0].point().distance_to(p(0., 0.)).unwrap() < 1e-12);
        assert!(controls[1].point().distance_to(p(3., 0.)).unwrap() < 1e-12);
        assert!(controls[2].point().distance_to(p(3., 1.)).unwrap() < 1e-12);
    }

    #[test]
    fn position_only_end_does_not_require_a_source_tangent() {
        let source = Curve3::NurbsCurve(
            NurbsCurve::try_new(
                1,
                vec![p(0., 0.), p(0., 0.), p(1., 0.)],
                vec![0., 0., 1., 2., 2.],
            )
            .unwrap(),
        );
        assert!(
            source
                .as_ref()
                .evaluate_with_tangent_on_side(0., ParameterSide::Right)
                .is_err()
        );
        let second = line(p(3., 1.), p(3., 2.));
        let blend = try_blend_curve(
            &source,
            p(0., 0.),
            &second,
            p(3., 1.),
            CurveBlendOptions {
                continuity: [
                    CurveBlendContinuity::Position,
                    CurveBlendContinuity::Tangency,
                ],
                handles: [None, Some(1.)],
                ..Default::default()
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(blend.degree(), 2);
        assert!(
            blend.control_points()[0]
                .point()
                .distance_to(p(0., 0.))
                .unwrap()
                < 1e-12
        );
        assert!(
            blend.control_points()[2]
                .point()
                .distance_to(p(3., 1.))
                .unwrap()
                < 1e-12
        );
    }

    #[test]
    fn quintic_matches_circular_curvature_and_a_straight_end() {
        let diagonal = 2.0_f64.sqrt();
        let arc = CircularArc3::try_from_three_points(
            p(2., 0.),
            p(diagonal, diagonal),
            p(0., 2.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let first = Curve3::Arc(arc);
        let second = line(p(-4., 3.), p(-4., 4.));
        let blend = try_blend_curve(
            &first,
            p(0., 2.),
            &second,
            p(-4., 3.),
            CurveBlendOptions {
                continuity: [
                    CurveBlendContinuity::Curvature,
                    CurveBlendContinuity::Curvature,
                ],
                handles: [Some(1.), Some(1.5)],
                ..Default::default()
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(blend.degree(), 5);
        let expected = CurveRef::Arc(&arc)
            .curvature_vector(*arc.domain().end())
            .unwrap();
        let actual = CurveRef::NurbsCurve(&blend)
            .curvature_vector(*blend.domain().start())
            .unwrap();
        let difference = Vector3::try_new(
            actual.x() - expected.x(),
            actual.y() - expected.y(),
            actual.z() - expected.z(),
        )
        .unwrap();
        assert!(difference.length().unwrap() < 1e-12);
        let end_curvature = CurveRef::NurbsCurve(&blend)
            .curvature_vector(*blend.domain().end())
            .unwrap();
        assert!(end_curvature.length().unwrap() < 1e-12);
    }

    #[test]
    fn quartic_matches_curvature_at_a_second_curves_start() {
        let diagonal = 2.0_f64.sqrt();
        let arc = CircularArc3::try_from_three_points(
            p(2., 0.),
            p(diagonal, diagonal),
            p(0., 2.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let blend = try_blend_curve(
            &line(p(-4., -1.), p(-4., 0.)),
            p(-4., 0.),
            &Curve3::Arc(arc),
            p(2., 0.),
            CurveBlendOptions {
                continuity: [
                    CurveBlendContinuity::Tangency,
                    CurveBlendContinuity::Curvature,
                ],
                handles: [Some(1.), Some(1.)],
                ..Default::default()
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(blend.degree(), 4);
        let expected = CurveRef::Arc(&arc)
            .curvature_vector(*arc.domain().start())
            .unwrap();
        let actual = CurveRef::NurbsCurve(&blend)
            .curvature_vector(*blend.domain().end())
            .unwrap();
        let difference = Vector3::try_new(
            actual.x() - expected.x(),
            actual.y() - expected.y(),
            actual.z() - expected.z(),
        )
        .unwrap();
        assert!(difference.length().unwrap() < 1e-12);
        let tangent = CurveRef::NurbsCurve(&blend)
            .evaluate_with_tangent_on_side(*blend.domain().end(), ParameterSide::Left)
            .unwrap()
            .tangent()
            .as_vector();
        assert!(
            tangent
                .angle_to(Vector3::try_new(0., 1., 0.).unwrap())
                .unwrap()
                < 1e-12
        );
    }

    #[test]
    fn cubic_position_to_curvature_matches_the_curved_end() {
        let diagonal = 2.0_f64.sqrt();
        let arc = CircularArc3::try_from_three_points(
            p(2., 0.),
            p(diagonal, diagonal),
            p(0., 2.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let blend = try_blend_curve(
            &line(p(-4., -1.), p(-4., 0.)),
            p(-4., 0.),
            &Curve3::Arc(arc),
            p(2., 0.),
            CurveBlendOptions {
                continuity: [
                    CurveBlendContinuity::Position,
                    CurveBlendContinuity::Curvature,
                ],
                handles: [None, Some(1.)],
                ..Default::default()
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(blend.degree(), 3);
        let expected = CurveRef::Arc(&arc)
            .curvature_vector(*arc.domain().start())
            .unwrap();
        let actual = CurveRef::NurbsCurve(&blend)
            .curvature_vector(*blend.domain().end())
            .unwrap();
        let difference = Vector3::try_new(
            actual.x() - expected.x(),
            actual.y() - expected.y(),
            actual.z() - expected.z(),
        )
        .unwrap();
        assert!(difference.length().unwrap() < 1e-12);
    }

    #[test]
    fn default_line_blends_match_rhino_common_control_shape() {
        let first = line(p(0., 0.), p(1., 0.));
        let second = line(p(4., 1.), p(4., 2.));
        let chord = 10.0_f64.sqrt();
        for (continuity, degree, expected) in [
            (
                CurveBlendContinuity::Position,
                1,
                vec![p(1., 0.), p(4., 1.)],
            ),
            (
                CurveBlendContinuity::Tangency,
                3,
                vec![p(1., 0.), p(1. + chord, 0.), p(4., 1. - chord), p(4., 1.)],
            ),
            (
                CurveBlendContinuity::Curvature,
                5,
                vec![
                    p(1., 0.),
                    p(1. + 0.4 * chord, 0.),
                    p(1. + 0.8 * chord, 0.),
                    p(4., 1. - 0.8 * chord),
                    p(4., 1. - 0.4 * chord),
                    p(4., 1.),
                ],
            ),
        ] {
            let blend = try_blend_curve(
                &first,
                p(1., 0.),
                &second,
                p(4., 1.),
                CurveBlendOptions {
                    continuity: [continuity; 2],
                    ..Default::default()
                },
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert_eq!(blend.degree(), degree);
            assert_eq!(blend.control_points().len(), expected.len());
            for (actual, expected) in blend.control_points().iter().zip(expected) {
                assert!(actual.point().distance_to(expected).unwrap() < 1e-12);
            }
        }
    }
}
