use super::*;

#[test]
fn closest_curve_stationary_spans_cannot_exclude_the_active_line_image() {
    for gauge in [1., -1.] {
        let original = NurbsCurve::try_new_rational(
            2,
            [0., 0., 1., 1., 1.]
                .into_iter()
                .zip([1., 2., 1., 2., 1.])
                .map(|(x, w)| WeightedPoint3::try_new(p(x, 0., 0.), gauge * w).unwrap())
                .collect(),
            vec![-3., -3., -3., 2., 2., 7., 7., 7.],
        )
        .unwrap();
        for reversed in [false, true] {
            let curve = if reversed {
                original.reversed().unwrap()
            } else {
                original.clone()
            };
            for z in [0., 3., 1e100] {
                let target = p(0.99, 0., z);
                let t = curve.closest_parameter(target, Tolerance::DEFAULT).unwrap();
                let actual = curve.evaluate(t).unwrap();
                assert!(
                    (actual.x() - 0.99).abs() < 1e-9,
                    "gauge={gauge}, reversed={reversed}, z={z}, t={t}, x={}",
                    actual.x()
                );
                let q: Real = 0.99;
                let root = -3. + 5. * (q + (3. * q * q + q).sqrt()) / (1. + 2. * q);
                assert!((t - if reversed { -root } else { root }).abs() < 1e-8);
            }
            // A true tie along the constant span must keep its first parameter.
            let t = curve
                .closest_parameter(p(2., 0., 0.), Tolerance::DEFAULT)
                .unwrap();
            assert_eq!(t, if reversed { -7. } else { 2. });
        }
    }
}

#[test]
fn closest_curve_stationary_spans_cannot_exclude_an_active_rational_arc() {
    let w = 0.5_f64.sqrt();
    let curve = NurbsCurve::try_new_rational(
        2,
        [
            (1., 0., 1.),
            (1., 1., w),
            (0., 1., 1.),
            (0., 1., 2.),
            (0., 1., 1.),
        ]
        .into_iter()
        .map(|(x, y, w)| WeightedPoint3::try_new(p(x, y, 0.), w).unwrap())
        .collect(),
        vec![0., 0., 0., 0.5, 0.5, 1., 1., 1.],
    )
    .unwrap();
    // Independent Bernstein expression for the stored binary64 weight, not an
    // assertion that the rounded rational quarter arc is an exact circle.
    let s: Real = 0.99;
    let a = (1. - s).powi(2);
    let b = 2. * w * s * (1. - s);
    let c = s * s;
    let xy = [(a + b) / (a + b + c), (b + c) / (a + b + c)];
    for z in [0., 3., 1e100] {
        let t = curve
            .closest_parameter(p(xy[0], xy[1], z), Tolerance::DEFAULT)
            .unwrap();
        assert!(
            curve
                .evaluate(t)
                .unwrap()
                .distance_to(p(xy[0], xy[1], 0.))
                .unwrap()
                < 1e-8,
            "z={z}, t={t}"
        );
    }
}

#[test]
fn closest_curve_far_seed_on_a_long_segment_can_beat_many_near_decoy_spans() {
    let mut points = vec![p(0., 0., 0.), p(1000., 0., 0.)];
    points.extend((1..=32).map(|i| p(990., i as Real / 8., 0.)));
    let curve = NurbsCurve::try_clamped_uniform(1, points).unwrap();
    let target = p(990., 0., 0.);
    let t = curve.closest_parameter(target, Tolerance::DEFAULT).unwrap();
    let actual = curve.evaluate(t).unwrap();
    assert!(
        actual.distance_to(target).unwrap() < 1e-9,
        "t={t}, point={actual:?}"
    );
    assert_eq!(actual.y(), 0.);
}
