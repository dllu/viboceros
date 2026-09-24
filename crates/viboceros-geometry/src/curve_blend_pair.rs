//! Endpoint blends with independent position, tangent, or curvature constraints.

use crate::{
    Curve3, GeometryError, NurbsCurve, ParameterSide, Point3, Real, Tolerance, UnitVector3,
    Vector3, curve_pair_support::selected_end,
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
}

impl Default for CurveBlendOptions {
    fn default() -> Self {
        Self {
            continuity: [CurveBlendContinuity::Tangency; 2],
            handles: [None, None],
        }
    }
}

/// Creates a single-span nonrational cubic or quintic connecting selected open-curve ends.
/// The handle lengths are model-space distances and can be adjusted independently.
pub fn try_blend_curve(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    options: CurveBlendOptions,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    let first_end = selected_end(first, first_pick, tolerance)?;
    let second_end = selected_end(second, second_pick, tolerance)?;
    let (start, first_tangent) = endpoint(first, first_end)?;
    let (end, second_tangent) = endpoint(second, second_end)?;
    let chord = start.vector_to(end)?;
    let chord_length = chord.length()?;
    if chord_length <= tolerance.absolute() {
        return Err(GeometryError::Degenerate {
            context: "curve blend endpoints",
        });
    }
    let default_handle = |continuity| match continuity {
        CurveBlendContinuity::Position => chord_length / 3.0,
        CurveBlendContinuity::Tangency => chord_length,
        CurveBlendContinuity::Curvature => chord_length * 0.4,
    };
    let first_handle = options.handles[0].unwrap_or_else(|| default_handle(options.continuity[0]));
    let second_handle = options.handles[1].unwrap_or_else(|| default_handle(options.continuity[1]));
    if !first_handle.is_finite()
        || !second_handle.is_finite()
        || first_handle <= 0.0
        || second_handle <= 0.0
    {
        return Err(GeometryError::Degenerate {
            context: "curve blend handle length",
        });
    }
    if options.continuity == [CurveBlendContinuity::Position; 2] {
        return NurbsCurve::try_new(
            1,
            vec![start, end],
            vec![0.0, 0.0, chord_length, chord_length],
        );
    }
    let chord_direction = chord.normalized_nonzero()?;
    let first_direction = match options.continuity[0] {
        CurveBlendContinuity::Position => chord_direction,
        CurveBlendContinuity::Tangency | CurveBlendContinuity::Curvature => {
            if first_end {
                first_tangent
            } else {
                first_tangent.opposite()
            }
        }
    };
    let second_direction = match options.continuity[1] {
        CurveBlendContinuity::Position => chord_direction,
        CurveBlendContinuity::Tangency | CurveBlendContinuity::Curvature => {
            if second_end {
                second_tangent.opposite()
            } else {
                second_tangent
            }
        }
    };
    let first_control = start.translated(first_direction.as_vector().scaled(first_handle)?)?;
    let second_control = end.translated(second_direction.as_vector().scaled(-second_handle)?)?;
    if options
        .continuity
        .contains(&CurveBlendContinuity::Curvature)
    {
        let first_curvature = endpoint_curvature(first, first_end, options.continuity[0])?;
        let second_curvature = endpoint_curvature(second, second_end, options.continuity[1])?;
        let first_second = second_control_from_endpoint(
            start,
            first_direction,
            first_handle,
            first_curvature,
            true,
        )?;
        let second_second = second_control_from_endpoint(
            end,
            second_direction,
            second_handle,
            second_curvature,
            false,
        )?;
        let blend = NurbsCurve::try_new(
            5,
            vec![
                start,
                first_control,
                first_second,
                second_second,
                second_control,
                end,
            ],
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
        )?;
        return blend.try_reparameterized(0.0..=blend.length(tolerance)?);
    }
    let blend = NurbsCurve::try_new(
        3,
        vec![start, first_control, second_control, end],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
    )?;
    blend.try_reparameterized(0.0..=blend.length(tolerance)?)
}

fn endpoint_curvature(
    curve: &Curve3,
    at_end: bool,
    continuity: CurveBlendContinuity,
) -> Result<Vector3, GeometryError> {
    if continuity != CurveBlendContinuity::Curvature {
        return Vector3::try_new(0.0, 0.0, 0.0);
    }
    let reference = curve.as_ref();
    let parameter = if at_end {
        *reference.domain().end()
    } else {
        *reference.domain().start()
    };
    reference.curvature_vector(parameter)
}

/// For a quintic B-spline, endpoint speed is `5h` and second derivative is
/// `20(P0 - 2P1 + P2)`. The normal acceleration required by the source is
/// speed squared times its curvature vector. Tangential acceleration is zero.
fn second_control_from_endpoint(
    endpoint: Point3,
    direction: UnitVector3,
    handle: Real,
    curvature: Vector3,
    at_start: bool,
) -> Result<Point3, GeometryError> {
    let tangent_offset = direction.as_vector().scaled(if at_start {
        2.0 * handle
    } else {
        -2.0 * handle
    })?;
    let curvature_offset = curvature.scaled(1.25 * handle * handle)?;
    let tangent = endpoint.translated(tangent_offset)?;
    tangent.translated(curvature_offset)
}

fn endpoint(curve: &Curve3, at_end: bool) -> Result<(Point3, UnitVector3), GeometryError> {
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
    let sample = reference.evaluate_with_tangent_on_side(parameter, side)?;
    Ok((sample.point(), sample.tangent()))
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
    fn position_continuity_follows_the_chord_independently_of_tangency() {
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
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        let controls = blend.control_points();
        let chord_direction = p(0., 0.)
            .vector_to(p(3., 1.))
            .unwrap()
            .normalized_nonzero()
            .unwrap();
        let first_handle = controls[0].point().vector_to(controls[1].point()).unwrap();
        assert!(first_handle.angle_to(chord_direction.as_vector()).unwrap() < 1e-12);
        assert!(controls[2].point().distance_to(p(3., 0.)).unwrap() < 1e-12);
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
    fn quintic_matches_curvature_at_a_second_curves_start() {
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
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
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
