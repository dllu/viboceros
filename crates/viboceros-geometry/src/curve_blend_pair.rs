//! Cubic endpoint blends with independent position or tangent constraints.

use crate::{
    Curve3, GeometryError, NurbsCurve, ParameterSide, Point3, Real, Tolerance, UnitVector3,
    curve_pair_support::selected_end,
};

/// Continuity imposed at one end of a cubic blend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveBlendContinuity {
    Position,
    Tangency,
}

/// Endpoint continuity and cubic handle lengths for a curve blend.
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

/// Creates a single-span nonrational cubic connecting selected open-curve ends.
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
    let default_handle = chord_length / 3.0;
    let first_handle = options.handles[0].unwrap_or(default_handle);
    let second_handle = options.handles[1].unwrap_or(default_handle);
    if !first_handle.is_finite()
        || !second_handle.is_finite()
        || first_handle <= 0.0
        || second_handle <= 0.0
    {
        return Err(GeometryError::Degenerate {
            context: "curve blend handle length",
        });
    }
    let chord_direction = chord.normalized_nonzero()?;
    let first_direction = match options.continuity[0] {
        CurveBlendContinuity::Position => chord_direction,
        CurveBlendContinuity::Tangency => {
            if first_end {
                first_tangent
            } else {
                first_tangent.opposite()
            }
        }
    };
    let second_direction = match options.continuity[1] {
        CurveBlendContinuity::Position => chord_direction,
        CurveBlendContinuity::Tangency => {
            if second_end {
                second_tangent.opposite()
            } else {
                second_tangent
            }
        }
    };
    let first_control = start.translated(first_direction.as_vector().scaled(first_handle)?)?;
    let second_control = end.translated(second_direction.as_vector().scaled(-second_handle)?)?;
    NurbsCurve::try_new(
        3,
        vec![start, first_control, second_control, end],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
    )
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
    use crate::{CurveRef, LineSegment, Vector3};

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
}
