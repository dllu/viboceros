use super::*;

fn unequal_source() -> NurbsCurve {
    NurbsCurve::try_new(
        3,
        [
            [0., 0.],
            [0.0625, 0.],
            [0.1875, 0.],
            [0.375, 1e-5],
            [0.625, 2e-5],
            [0.75, 2e-5],
        ]
        .into_iter()
        .map(|p| Point3::try_new(p[0], p[1], 0.).unwrap())
        .collect(),
        vec![0., 0., 0., 0., 0.5, 0.5, 1., 1., 1., 1.],
    )
    .unwrap()
}

#[test]
fn a_joined_curve_requires_its_own_proof_instead_of_a_certified_raw_assembly() {
    let source = unequal_source();
    let before = source.clone();
    let angle = 1_f64.to_radians();
    let raw = pieces::construct(&source, 2e-6, angle, pieces::SplitClock::SourceLookup)
        .unwrap()
        .unwrap();
    assert_eq!(raw.breaks, [0., 0.25, 0.5, 1.]);
    assert_eq!(raw.curve.control_points().len(), 10);
    assert!(!crate::curve_fit::g1::within_deviation(&source, &raw.curve, 2e-6).unwrap());
    let joined = try_propose_joined_refit(&source, 2e-6, 0.)
        .unwrap()
        .unwrap();
    assert_eq!(joined.control_points().len(), 8);
    assert_eq!(
        joined.knots(),
        [0., 0., 0., 0., 0.25, 0.25, 0.5, 0.5, 1., 1., 1., 1.]
    );
    assert!(crate::curve_fit::g1::within_deviation(&source, &joined, 2e-6).unwrap());
    let expected_y = [
        0.,
        0.,
        2.0963558545769456e-7,
        3.059657518309612e-6,
        6.726319695810892e-6,
        1.4719611722290931e-5,
        2e-5,
        2e-5,
    ];
    for (point, y) in joined.control_points().iter().zip(expected_y) {
        assert!((point.point().y() - y).abs() < 1e-15);
    }
    assert_eq!(source, before);
}

#[test]
fn original_source_proposals_validate_before_constructing_output() {
    let source = unequal_source();
    let before = source.clone();
    for limit in [0., -1., Real::NAN, Real::INFINITY] {
        assert!(matches!(
            try_propose_joined_refit(&source, limit, 0.),
            Err(GeometryError::InvalidTolerance)
        ));
    }
    for angle in [-1., Real::NAN, Real::INFINITY, 2. * std::f64::consts::PI] {
        assert!(matches!(
            try_propose_joined_refit(&source, 2e-6, angle),
            Err(GeometryError::InvalidContinuityAngle)
        ));
    }
    let controls = (0..513)
        .map(|i| Point3::try_new(i as Real, 0., 0.).unwrap())
        .collect();
    let knots = [vec![0.], (0..513).map(|i| i as Real).collect(), vec![512.]].concat();
    let large = NurbsCurve::try_new(1, controls, knots).unwrap();
    assert!(matches!(
        try_propose_joined_refit(&large, 2e-6, 0.),
        Err(GeometryError::LoftResourceLimit { .. })
    ));
    assert_eq!(source, before);
}
