use super::*;
use crate::{Circle3, LoftStyle, Point3, Tolerance, UnitVector3, try_loft_nurbs_curves};

fn sections(scale: Real, offset: [Real; 3]) -> Vec<NurbsCurve> {
    [
        (
            [[0., 0., 0.], [1., 0., 0.], [1., 2., 0.]],
            [0., 0., 0.5, 1., 1.],
        ),
        (
            [[0., 0., 3.], [3., 0., 3.], [3., 0.5, 3.]],
            [7., 7., 8., 9., 9.],
        ),
    ]
    .into_iter()
    .map(|(points, knots)| {
        NurbsCurve::try_new(
            1,
            points
                .into_iter()
                .map(|p| {
                    Point3::try_from(std::array::from_fn(|i| p[i] * scale + offset[i])).unwrap()
                })
                .collect(),
            knots.to_vec(),
        )
        .unwrap()
    })
    .collect()
}

#[test]
fn matched_corners_follow_the_common_chart_without_moving_the_profiles() {
    let source = sections(1., [0.; 3]);
    let before = source.clone();
    let surface = try_loft_nurbs_curves(&source, LoftStyle::Straight, false).unwrap();
    let fraction = surface.knots_v()[2] / surface.domain_v().end();
    assert!((fraction - 0.6).abs() < 1e-14);
    for (i, curve) in source.iter().enumerate() {
        let u = surface.parameter_at_u(i as Real).unwrap();
        for (a, b) in [([0., 0.6], [0., 0.5]), ([0.6, 1.], [0.5, 1.])] {
            for j in 0..=8 {
                let t = j as Real / 8.;
                let point = surface
                    .evaluate(u, surface.parameter_at_v(a[0] + t * (a[1] - a[0])).unwrap())
                    .unwrap();
                let expected = curve
                    .evaluate(curve.parameter_at(b[0] + t * (b[1] - b[0])).unwrap())
                    .unwrap();
                assert!(point.distance_to(expected).unwrap() < 1e-12);
            }
        }
    }
    assert_eq!(source, before);
}

#[test]
fn common_chart_and_geometry_are_independent_of_extreme_model_units() {
    let expected =
        try_loft_nurbs_curves(&sections(1., [0.; 3]), LoftStyle::Straight, false).unwrap();
    for scale in [1e-200, 1e-100, 1e100, 1e200] {
        let source = sections(scale, [0.; 3]);
        let before = source.clone();
        let actual = try_loft_nurbs_curves(&source, LoftStyle::Straight, false).unwrap();
        for i in 0..=8 {
            for j in 0..=8 {
                let u = i as Real / 8.;
                let v = j as Real / 8.;
                let a = actual
                    .evaluate(
                        actual.parameter_at_u(u).unwrap(),
                        actual.parameter_at_v(v).unwrap(),
                    )
                    .unwrap();
                let b = expected
                    .evaluate(
                        expected.parameter_at_u(u).unwrap(),
                        expected.parameter_at_v(v).unwrap(),
                    )
                    .unwrap();
                for (x, y) in a.to_array().into_iter().zip(b.to_array()) {
                    assert!(
                        (x / scale - y).abs() < 2e-12,
                        "scale {scale}: {x} / {scale} != {y}"
                    );
                }
            }
        }
        assert_eq!(source, before);
    }
}

#[test]
fn invalid_backend_clamping_preserves_sources_and_stops_before_correspondence() {
    let source = sections(1., [0.; 3]);
    let before = source.clone();
    let error = prepare(&source, &mut |c: &NurbsCurve| {
        c.try_reparameterized(10. ..=20.)
    })
    .unwrap_err();
    assert!(matches!(error, GeometryError::InvalidControlNet { .. }));
    assert_eq!(source, before);
}

#[test]
fn length_budget_applies_across_spans() {
    let curve = Circle3::try_new(
        Point3::try_new(0., 0., 0.).unwrap(),
        1.,
        UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let before = curve.clone();
    let mut remaining = 4;
    assert!(matches!(
        coarse_length(&curve, 1., &mut remaining),
        Err(GeometryError::LoftResourceLimit {
            context: "length integration steps",
            ..
        })
    ));
    assert_eq!(remaining, 0);
    assert_eq!(curve, before);
}

#[test]
fn unrepresentable_common_intervals_are_rejected_without_deforming_profiles() {
    let points = [
        [0., 0.],
        [0., 5e11],
        [0., 1e12],
        [0.1, 1e12],
        [0.3, 1e12],
        [0.7, 1e12],
        [1., 1e12],
    ];
    let knots = [
        0.,
        0.,
        0.,
        0.5,
        0.5,
        Real::from_bits(0.5_f64.to_bits() + 1),
        0.75,
        1.,
        1.,
        1.,
    ];
    let source = (0..2)
        .map(|i| {
            NurbsCurve::try_new(
                2,
                points
                    .iter()
                    .map(|p| Point3::try_new(p[0], p[1], i as Real).unwrap())
                    .collect(),
                knots.iter().map(|t| t * (i + 1) as Real).collect(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let before = source.clone();
    assert!(matches!(
        try_loft_nurbs_curves(&source, LoftStyle::Straight, false),
        Err(GeometryError::InvalidControlNet {
            context: "Loft parameter mapping collapsed an interval"
        })
    ));
    assert_eq!(source, before);
}
