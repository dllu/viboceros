#![cfg(feature = "native")]
use viboceros_geometry::{NurbsCurve, Point3, WeightedPoint3};
use viboceros_smlib::reduce_curve_knots_in_range;

#[test]
fn native_basis_proposals_cannot_bypass_strict_zero() {
    let source = NurbsCurve::try_new(
        3,
        [
            [0., 0., 0.],
            [1., 0., 0.],
            [2., 0., 0.],
            [3., 0., 0.],
            [3., 1., 0.],
            [3., 2., 0.],
            [3., 3., 0.],
        ]
        .map(|p| Point3::try_from(p).unwrap())
        .to_vec(),
        [vec![0.; 4], vec![0.5; 3], vec![1.; 4]].concat(),
    )
    .unwrap();
    let before = source.clone();
    let strict = reduce_curve_knots_in_range(&source, 1, 4, Some(0.)).unwrap();
    assert_eq!(strict.curve, source);
    assert_eq!(strict.removed, 0);
    assert_eq!(strict.maximum_deviation_bound, Some(0.));
    let loose = reduce_curve_knots_in_range(&source, 1, 4, None).unwrap();
    assert_eq!(loose.removed, 2);
    assert_eq!(loose.maximum_deviation_bound, None);
    assert!(
        source
            .parameter_curve_deviation_bound(&loose.curve, 1e-3)
            .unwrap()
            .is_none()
    );
    assert_eq!(source, before);
}

#[test]
fn native_constant_channels_survive_zero_error_removal_under_either_gauge() {
    for gauge in [1., -2.] {
        let source = NurbsCurve::try_new_rational(
            3,
            vec![
                WeightedPoint3::try_new(Point3::try_from([2., 3., 4.]).unwrap(), gauge).unwrap();
                6
            ],
            [vec![0.; 4], vec![0.5; 2], vec![1.; 4]].concat(),
        )
        .unwrap();
        let before = source.clone();
        let result = reduce_curve_knots_in_range(&source, 1, 4, Some(0.)).unwrap();
        assert_eq!(result.removed, 1);
        assert_eq!(result.maximum_deviation_bound, Some(0.));
        assert!(
            result
                .curve
                .control_points()
                .iter()
                .all(|p| p.weight() == gauge)
        );
        assert_eq!(
            source
                .parameter_curve_deviation_bound(&result.curve, 0.)
                .unwrap(),
            Some(0.)
        );
        assert_eq!(source, before);
    }
}
