use super::*;
use crate::CurveRef;

pub(super) fn split_sharp_knots(
    source: &PolyCurve3,
    tolerance: Tolerance,
) -> Result<Option<PolyCurve3>, GeometryError> {
    let mut segments = Vec::with_capacity(source.segments().len());
    let mut changed = false;
    for segment in source.segments() {
        let CurveSegment3::NurbsCurve(curve) = segment else {
            segments.push(segment.clone());
            continue;
        };
        let mut breaks = vec![*curve.domain().start()];
        for (knot, _) in curve.interior_knot_groups() {
            let source = CurveRef::NurbsCurve(curve);
            let left = source.evaluate_with_tangent_on_side(knot, ParameterSide::Left)?;
            let right = source.evaluate_with_tangent_on_side(knot, ParameterSide::Right)?;
            if !curve_points_coincident(left.point(), right.point()) {
                return Err(unsupported_curved_corner());
            }
            if tangent_angle(left.tangent().as_vector(), right.tangent().as_vector())?
                > tolerance.angular()
            {
                breaks.push(knot);
            }
        }
        if breaks.len() == 1 {
            segments.push(segment.clone());
            continue;
        }
        changed = true;
        breaks.push(*curve.domain().end());
        for pair in breaks.windows(2) {
            segments.push(CurveSegment3::NurbsCurve(
                curve.try_trimmed(pair[0]..=pair[1])?,
            ));
        }
    }
    if changed {
        Ok(Some(PolyCurve3::try_new(segments)?))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.).unwrap()
    }

    #[test]
    fn single_nurbs_leaf_sharp_knot_matches_separate_native_leaves() {
        let controls = vec![p(0., 0.), p(2., 0.), p(2., 2.), p(4., 2.), p(4., 4.)];
        let joined =
            NurbsCurve::try_new(2, controls.clone(), vec![0., 0., 0., 1., 1., 2., 2., 2.]).unwrap();
        let separate = PolyCurve3::try_new(vec![
            CurveSegment3::NurbsCurve(
                NurbsCurve::try_new(2, controls[..3].to_vec(), vec![0., 0., 0., 1., 1., 1.])
                    .unwrap(),
            ),
            CurveSegment3::NurbsCurve(
                NurbsCurve::try_new(2, controls[2..].to_vec(), vec![1., 1., 1., 2., 2., 2.])
                    .unwrap(),
            ),
        ])
        .unwrap();
        let actual = PolyCurve3::try_new(vec![CurveSegment3::NurbsCurve(joined)])
            .unwrap()
            .try_fillet_corners(0.5, Tolerance::DEFAULT)
            .unwrap();
        let expected = separate
            .try_fillet_corners(0.5, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(actual.segments().len(), 3);
        assert!(matches!(actual.segments()[0], CurveSegment3::NurbsCurve(_)));
        assert!(matches!(actual.segments()[1], CurveSegment3::Arc(_)));
        assert!(matches!(actual.segments()[2], CurveSegment3::NurbsCurve(_)));
        let actual_samples = CurveRef::PolyCurve(&actual)
            .sample_equal_length_points(32, true, Tolerance::DEFAULT)
            .unwrap();
        let expected_samples = CurveRef::PolyCurve(&expected)
            .sample_equal_length_points(32, true, Tolerance::DEFAULT)
            .unwrap();
        for (a, b) in actual_samples.into_iter().zip(expected_samples) {
            assert!(a.distance_to(b).unwrap() < 1e-10);
        }
    }
}
