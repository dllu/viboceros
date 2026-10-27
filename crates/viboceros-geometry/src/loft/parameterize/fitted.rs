//! Region rules specific to the adaptive fitter.
use super::*;

// Composite fitting retains G2 regions at the measured absolute curvature
// tolerance, in model units. Existing general matching keeps its own policy.
pub(in crate::loft) fn region_breaks(
    curve: &NurbsCurve,
    angle: Real,
) -> Result<Vec<Real>, GeometryError> {
    let reference = crate::CurveRef::NurbsCurve(curve);
    let mut breaks = Vec::new();
    for t in independent_breaks(curve) {
        let left = curve
            .tangent_at_on_side(t, crate::ParameterSide::Left)?
            .as_vector();
        let right = curve
            .tangent_at_on_side(t, crate::ParameterSide::Right)?
            .as_vector();
        let a = reference.curvature_vector_on_side(t, crate::ParameterSide::Left)?;
        let b = reference.curvature_vector_on_side(t, crate::ParameterSide::Right)?;
        let difference = (a.x() - b.x()).hypot(a.y() - b.y()).hypot(a.z() - b.z());
        if left.dot(right)? < angle.cos() || difference > 0.01 {
            breaks.push(t);
        }
    }
    Ok(breaks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Point3;

    fn small_relative_curvature_jump() -> NurbsCurve {
        NurbsCurve::try_new(
            3,
            [
                [-3., 0.],
                [-2., 3.],
                [-1., 0.],
                [0., 0.],
                [1., 0.],
                [2., 3.03],
                [3., 0.],
            ]
            .map(|p| Point3::try_new(p[0], p[1], 0.).unwrap())
            .to_vec(),
            [vec![0.; 4], vec![0.5; 3], vec![1.; 4]].concat(),
        )
        .unwrap()
    }

    #[test]
    fn absolute_fitting_threshold_retains_a_small_relative_curvature_jump() {
        let curve = small_relative_curvature_jump();
        assert!(
            super::super::region_breaks(&curve, coordinate_scale(&curve), 1_f64.to_radians().cos())
                .unwrap()
                .is_empty()
        );
        assert_eq!(region_breaks(&curve, 1_f64.to_radians()).unwrap(), [0.5]);
    }

    #[test]
    fn fitted_charts_keep_retained_regions_even_for_matching_bases() {
        let source = [small_relative_curvature_jump()];
        let before = source.clone();
        let geometric = prepare_refit(&source, None, &mut |c: &NurbsCurve| {
            c.clamped_to_active_domain()
        })
        .unwrap();
        assert_eq!(geometric[0].knots(), source[0].knots());
        let fitted =
            prepare_fitted(&source, &mut |c: &NurbsCurve| c.clamped_to_active_domain()).unwrap();
        assert_eq!(fitted[0].control_points(), source[0].control_points());
        let boundary = fitted[0].knots()[4];
        assert!((boundary - 0.5).abs() > 1e-4);
        for (old, new) in [(0., 0.), (0.5, boundary), (1., 1.)] {
            assert!(
                source[0]
                    .evaluate(old)
                    .unwrap()
                    .distance_to(fitted[0].evaluate(new).unwrap())
                    .unwrap()
                    < 1e-14
            );
        }
        assert_eq!(source, before);
    }
}
