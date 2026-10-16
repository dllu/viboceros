use super::*;

fn comb(c: &NurbsCurve) -> CurvatureComb {
    curvature_comb_with_subdivision(c, 1, 10_000, &mut |c, interval| c.try_trimmed(interval))
        .unwrap()
}
fn outline(c: &NurbsCurve) -> Vec<Point3> {
    let curves = comb(c).envelope_polylines(1., Tolerance::DEFAULT).unwrap();
    assert_eq!(curves.len(), 1);
    curves[0].vertices().to_vec()
}
#[test]
fn curvature_chord_subdivision_matches_native_angle_boundary() {
    // Independent native captures have 71, 71 and 141 vertices respectively.
    for (height, vertices) in [(1.5, 71), (3_f64.sqrt(), 71), (1.75, 141)] {
        let c = NurbsCurve::try_new(
            2,
            vec![p(0., 0.), p(1., 0.), p(1., height)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        assert_eq!(outline(&c).len(), vertices);
    }
}
#[test]
fn curvature_chord_subdivision_preserves_rotation_reversal_and_weight_scale() {
    let c = NurbsCurve::try_new(
        2,
        vec![p(0., 0.), p(1., 0.), p(1., 2.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let baseline = outline(&c);
    let rotation = nalgebra::Rotation3::from_euler_angles(0.4, -0.7, 1.2);
    let transform = |p: Point3| {
        let q = rotation * nalgebra::Vector3::from(p.to_array());
        Point3::try_new(q.x + 1., q.y + 2., q.z - 3.).unwrap()
    };
    for reverse in [false, true] {
        for weight in [1., -3.] {
            let mut controls = c
                .control_points()
                .iter()
                .map(|v| crate::WeightedPoint3::try_new(transform(v.point()), weight).unwrap())
                .collect::<Vec<_>>();
            if reverse {
                controls.reverse();
            }
            let changed = NurbsCurve::try_new_rational(2, controls, c.knots().to_vec()).unwrap();
            let mut actual = outline(&changed);
            if reverse {
                actual.reverse();
            }
            assert_eq!(actual.len(), baseline.len());
            for (actual, expected) in actual.iter().zip(&baseline) {
                assert!(actual.distance_to(transform(*expected)).unwrap() < 1e-11);
            }
        }
    }
}
#[test]
fn curvature_chord_subdivision_uses_signed_rational_endpoint_directions() {
    let controls = [(p(0., 0.), 1.), (p(1., 1.), -0.1), (p(2., 0.), 1.)]
        .map(|(p, w)| crate::WeightedPoint3::try_new(p, w).unwrap())
        .to_vec();
    let c = NurbsCurve::try_new_rational(2, controls, vec![0., 0., 0., 1., 1., 1.]).unwrap();
    let sampled = comb(&c);
    assert!(sampled.strips.len() > 1);
    assert!(sampled.hairs.first().unwrap().tangent.x() < 0.);
    assert!(sampled.hairs.last().unwrap().tangent.x() < 0.);
    for hair in &sampled.hairs {
        assert!(
            hair.point
                .distance_to(c.evaluate(hair.parameter).unwrap())
                .unwrap()
                < 1e-12
        );
        let reference = CurveRef::NurbsCurve(&c)
            .curvature_vector(hair.parameter)
            .unwrap()
            .to_array();
        let actual = hair.curvature.to_array();
        assert!(
            (0..3)
                .map(|i| (actual[i] - reference[i]).powi(2))
                .sum::<Real>()
                .sqrt()
                < 1e-8
        );
    }
    let pole = NurbsCurve::try_new_rational(
        1,
        vec![
            crate::WeightedPoint3::try_new(p(0., 0.), 1.).unwrap(),
            crate::WeightedPoint3::try_new(p(1., 0.), -1.).unwrap(),
        ],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(
        curvature_comb_with_subdivision(&pole, 1, 1000, &mut |c, interval| c.try_trimmed(interval))
            .is_err()
    );
}
