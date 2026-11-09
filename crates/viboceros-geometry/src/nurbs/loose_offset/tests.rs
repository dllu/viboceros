use super::*;

fn normal() -> UnitVector3 {
    Vector3::try_new(0., 0., 1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap()
}
fn rational() -> NurbsCurve {
    NurbsCurve::try_new_rational(
        3,
        [[0., 0., 0.], [1., 2., 0.], [4., -1., 0.], [5., 1., 0.]]
            .into_iter()
            .zip([1., 0.7, 1.3, 1.])
            .map(|(p, w)| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w).unwrap())
            .collect(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap()
}

#[test]
fn polynomial_and_retained_weights_interpolate_the_same_original_normal_stations() {
    let source = rational();
    let before = source.clone();
    let tol = Tolerance::DEFAULT;
    let polynomial = source
        .try_loose_polynomial_offset(0.3, normal(), tol, None)
        .unwrap();
    let retained = source
        .try_loose_planar_offset(0.3, normal(), tol, None)
        .unwrap();
    assert!(polynomial.control_points().iter().all(|p| p.weight() == 1.));
    assert_eq!(
        retained
            .control_points()
            .iter()
            .map(|p| p.weight())
            .collect::<Vec<_>>(),
        source
            .control_points()
            .iter()
            .map(|p| p.weight())
            .collect::<Vec<_>>()
    );
    assert_eq!(polynomial.knots(), source.knots());
    for station in source.edit_points().unwrap() {
        let tangent = source
            .tangent_at_on_side(station.parameter, ParameterSide::Right)
            .unwrap();
        let expected = station
            .point
            .translated(
                normal()
                    .as_vector()
                    .cross(tangent.as_vector())
                    .unwrap()
                    .normalized_nonzero()
                    .unwrap()
                    .as_vector()
                    .scaled(0.3)
                    .unwrap(),
            )
            .unwrap();
        for curve in [&polynomial, &retained] {
            assert!(
                curve
                    .evaluate(station.parameter)
                    .unwrap()
                    .distance_to(expected)
                    .unwrap()
                    < 1e-12
            );
        }
    }
    assert_eq!(source, before);
}

#[test]
fn polynomial_policy_uses_the_right_tangent_at_corners_and_can_open_a_kinked_seam() {
    let shape = |points: Vec<[Real; 3]>| {
        let end = (points.len() - 1) as Real;
        let knots = [
            vec![0., 0.],
            (1..points.len() - 1).map(|i| i as Real).collect(),
            vec![end, end],
        ]
        .concat();
        NurbsCurve::try_new(
            1,
            points
                .into_iter()
                .map(|p| Point3::try_from(p).unwrap())
                .collect(),
            knots,
        )
        .unwrap()
    };
    let source = shape(vec![[0., 0., 0.], [5., 0., 0.], [5., 4., 0.]]);
    let result = source
        .try_loose_polynomial_offset(-0.5, normal(), Tolerance::DEFAULT, None)
        .unwrap();
    assert_eq!(result.evaluate(1.).unwrap().to_array(), [5.5, 0., 0.]);
    let closed = shape(vec![
        [0., 0., 0.],
        [5., 0., 0.],
        [5., 4., 0.],
        [0., 4., 0.],
        [0., 0., 0.],
    ]);
    assert!(closed.is_closed().unwrap());
    let offset = closed
        .try_loose_polynomial_offset(-0.5, normal(), Tolerance::DEFAULT, None)
        .unwrap();
    assert!(!offset.is_closed().unwrap());
    assert_eq!(
        offset.control_points().first().unwrap().point().to_array(),
        [0., -0.5, 0.]
    );
    assert_eq!(
        offset.control_points().last().unwrap().point().to_array(),
        [-0.5, 0., 0.]
    );
}

#[test]
fn common_weight_gauges_leave_the_complete_polynomial_offset_unchanged() {
    let source = rational();
    let reference = source
        .try_loose_polynomial_offset(0.2, normal(), Tolerance::DEFAULT, None)
        .unwrap();
    for gauge in [-2., 2., 1e-200, 1e200] {
        let shifted = NurbsCurve::try_new_rational(
            source.degree(),
            source
                .control_points()
                .iter()
                .map(|p| WeightedPoint3::try_new(p.point(), gauge * p.weight()).unwrap())
                .collect(),
            source.knots().to_vec(),
        )
        .unwrap();
        let actual = shifted
            .try_loose_polynomial_offset(0.2, normal(), Tolerance::DEFAULT, None)
            .unwrap();
        assert!(
            reference
                .parameter_curve_deviation_bound(&actual, 1e-11)
                .unwrap()
                .is_some()
        );
    }
}

#[test]
fn invalid_distances_and_backend_failures_preserve_the_original() {
    let source = rational();
    let before = source.clone();
    for distance in [0., Real::NAN, Real::INFINITY] {
        assert!(matches!(
            source.try_loose_polynomial_offset(distance, normal(), Tolerance::DEFAULT, None),
            Err(GeometryError::InvalidCurveOffsetDistance)
        ));
    }
    fn missing(_: &crate::CurveEditPointInterpolation<'_>) -> Result<Vec<Real>, GeometryError> {
        Ok(vec![])
    }
    assert!(
        source
            .try_loose_polynomial_offset(0.2, normal(), Tolerance::DEFAULT, Some(missing))
            .is_err()
    );
    assert_eq!(source, before);
}
