//! Geometric endpoint continuity, independent of NURBS parameter speed.

use crate::{CurveRef, GeometryError, ParameterSide, Real, Tolerance};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveContinuityLevel {
    Disconnected,
    Position,
    Tangency,
    CurvatureOrHigher,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveContinuityReport {
    pub level: CurveContinuityLevel,
    pub gap: Real,
    pub tangent_angle_radians: Real,
    pub curvature_vector_difference: Real,
}

/// Compares the selected ends as though the first curve runs into the join
/// and the second runs out of it. Reversed source parameterizations therefore
/// have the same geometric result. Curvature is compared as a vector, so equal
/// scalar radii bending to opposite sides do not satisfy G2.
pub fn curve_end_continuity(
    first: CurveRef<'_>,
    first_at_end: bool,
    second: CurveRef<'_>,
    second_at_end: bool,
    tolerance: Tolerance,
) -> Result<CurveContinuityReport, GeometryError> {
    if first.is_closed()? || second.is_closed()? {
        return Err(GeometryError::InvalidPolyCurve {
            context: "GCon requires open curves",
        });
    }
    let first_parameter = if first_at_end {
        *first.domain().end()
    } else {
        *first.domain().start()
    };
    let second_parameter = if second_at_end {
        *second.domain().end()
    } else {
        *second.domain().start()
    };
    let first_sample = first.evaluate_with_tangent_on_side(
        first_parameter,
        if first_at_end {
            ParameterSide::Left
        } else {
            ParameterSide::Right
        },
    )?;
    let second_sample = second.evaluate_with_tangent_on_side(
        second_parameter,
        if second_at_end {
            ParameterSide::Left
        } else {
            ParameterSide::Right
        },
    )?;
    let gap = first_sample.point().distance_to(second_sample.point())?;
    let first_direction = if first_at_end {
        first_sample.tangent()
    } else {
        first_sample.tangent().opposite()
    };
    let second_direction = if second_at_end {
        second_sample.tangent().opposite()
    } else {
        second_sample.tangent()
    };
    let tangent_angle = first_direction
        .as_vector()
        .angle_to(second_direction.as_vector())?;
    let first_curvature = first.curvature_vector(first_parameter)?;
    let second_curvature = second.curvature_vector(second_parameter)?;
    let curvature_difference = crate::Vector3::try_new(
        first_curvature.x() - second_curvature.x(),
        first_curvature.y() - second_curvature.y(),
        first_curvature.z() - second_curvature.z(),
    )?
    .length()?;
    let length_scale = first.length(tolerance)?.min(second.length(tolerance)?);
    let absolute_curvature_tolerance = tolerance.absolute() / length_scale / length_scale;
    let relative_curvature_tolerance =
        tolerance.relative() * first_curvature.length()?.max(second_curvature.length()?);
    let level = if gap > tolerance.absolute() {
        CurveContinuityLevel::Disconnected
    } else if tangent_angle > tolerance.angular() {
        CurveContinuityLevel::Position
    } else if curvature_difference > absolute_curvature_tolerance.max(relative_curvature_tolerance)
    {
        CurveContinuityLevel::Tangency
    } else {
        CurveContinuityLevel::CurvatureOrHigher
    };
    Ok(CurveContinuityReport {
        level,
        gap,
        tangent_angle_radians: tangent_angle,
        curvature_vector_difference: curvature_difference,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CircularArc3, LineSegment, Point3};

    fn p(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.0).unwrap()
    }

    #[test]
    fn distinguishes_gap_kink_and_matching_straight_curves() {
        let a = LineSegment::try_new(p(0.0, 0.0), p(1.0, 0.0), Tolerance::DEFAULT).unwrap();
        let b = LineSegment::try_new(p(1.0, 0.0), p(2.0, 0.0), Tolerance::DEFAULT).unwrap();
        let kink = LineSegment::try_new(p(1.0, 0.0), p(1.0, 1.0), Tolerance::DEFAULT).unwrap();
        let gap = LineSegment::try_new(p(1.1, 0.0), p(2.1, 0.0), Tolerance::DEFAULT).unwrap();
        let same = curve_end_continuity(
            CurveRef::Line(&a),
            true,
            CurveRef::Line(&b),
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(same.level, CurveContinuityLevel::CurvatureOrHigher);
        assert_eq!(same.gap, 0.0);
        assert_eq!(same.tangent_angle_radians, 0.0);
        assert_eq!(same.curvature_vector_difference, 0.0);
        let corner = curve_end_continuity(
            CurveRef::Line(&a),
            true,
            CurveRef::Line(&kink),
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(corner.level, CurveContinuityLevel::Position);
        assert!((corner.tangent_angle_radians - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert_eq!(
            curve_end_continuity(
                CurveRef::Line(&a),
                true,
                CurveRef::Line(&gap),
                false,
                Tolerance::DEFAULT
            )
            .unwrap()
            .level,
            CurveContinuityLevel::Disconnected
        );
    }

    #[test]
    fn curvature_vector_rejects_opposite_bends_at_a_tangent_join() {
        let upper = CircularArc3::try_from_three_points(
            p(0.0, 0.0),
            p(1.0, 1.0),
            p(2.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let lower = CircularArc3::try_from_three_points(
            p(2.0, 0.0),
            p(3.0, -1.0),
            p(4.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let report = curve_end_continuity(
            CurveRef::Arc(&upper),
            true,
            CurveRef::Arc(&lower),
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(report.level, CurveContinuityLevel::Tangency);
        assert!(report.curvature_vector_difference > 1.0);
    }
}
