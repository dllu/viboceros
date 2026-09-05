use super::*;

fn mixed_patch(scale: f64) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        [
            (p(0.0, 0.0, 0.0), 1.0),
            (p(0.5, 0.0, 0.0), 2.0),
            (p(0.0, 1.0 / 3.0, 0.0), 3.0),
            (p(0.2, 0.2, 0.2), 5.0),
        ]
        .map(|(point, weight)| WeightedPoint3::try_new(point, weight * scale).unwrap())
        .to_vec(),
        vec![0.0, 0.0, 1.0, 1.0],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap()
}

#[test]
fn bilinear_rational_second_partials_are_not_assumed_zero() {
    for scale in [1.0, 2.0_f64.powi(-700), -2.0_f64.powi(700)] {
        let surface = mixed_patch(scale);
        // S=(u,v,uv)/(1+u+2v+uv), evaluated at u=1/4, v=1/2.
        let jet = surface.evaluate_with_second_derivatives(0.25, 0.5).unwrap();
        near(
            Vector3::try_from(jet.point.to_array()).unwrap(),
            [2.0 / 19.0, 4.0 / 19.0, 1.0 / 19.0],
        );
        near(
            jet.derivative_u,
            [128.0 / 361.0, -48.0 / 361.0, 64.0 / 361.0],
        );
        near(
            jet.derivative_v,
            [-36.0 / 361.0, 80.0 / 361.0, 20.0 / 361.0],
        );
        near(
            jet.derivative_uu,
            [-3072.0 / 6859.0, 1152.0 / 6859.0, -1536.0 / 6859.0],
        );
        near(
            jet.derivative_uv,
            [-2176.0 / 6859.0, -704.0 / 6859.0, 1344.0 / 6859.0],
        );
        near(
            jet.derivative_vv,
            [1296.0 / 6859.0, -2880.0 / 6859.0, -720.0 / 6859.0],
        );
        let first = surface.evaluate_with_derivatives(0.25, 0.5).unwrap();
        assert_eq!((jet.point, jet.derivative_u, jet.derivative_v), first);
        assert_eq!(surface.evaluate(0.25, 0.5).unwrap(), jet.point);
    }
}

#[test]
fn polynomial_tensor_patch_has_exact_pure_and_mixed_second_partials() {
    // S=(u,v,u²+uv+v²), in tensor-product quadratic Bernstein form.
    let controls = (0..3)
        .flat_map(|j| {
            (0..3).map(move |i| {
                let z = f64::from(i == 2) + f64::from(j == 2) + (i * j) as f64 / 4.0;
                WeightedPoint3::try_new(p(i as f64 / 2.0, j as f64 / 2.0, z), 1.0).unwrap()
            })
        })
        .collect();
    let surface = NurbsSurface::try_new_rational(
        2,
        2,
        3,
        3,
        controls,
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    for (u, v) in [(0.0, 0.0), (0.25, 0.7), (0.5, 0.5), (1.0, 1.0)] {
        let jet = surface.evaluate_with_second_derivatives(u, v).unwrap();
        assert!(
            jet.point
                .distance_to(p(u, v, u * u + u * v + v * v))
                .unwrap()
                < 2e-12
        );
        near(jet.derivative_u, [1.0, 0.0, 2.0 * u + v]);
        near(jet.derivative_v, [0.0, 1.0, u + 2.0 * v]);
        near(jet.derivative_uu, [0.0, 0.0, 2.0]);
        near(jet.derivative_uv, [0.0, 0.0, 1.0]);
        near(jet.derivative_vv, [0.0, 0.0, 2.0]);
    }
}

#[test]
fn surface_jets_obey_reversal_swap_and_native_domain_chain_rules() {
    let source = mixed_patch(1.0);
    let jet = source.evaluate_with_second_derivatives(0.25, 0.5).unwrap();
    let reversed = source
        .try_reversed_u()
        .unwrap()
        .evaluate_with_second_derivatives(-0.25, 0.5)
        .unwrap();
    near(
        reversed.derivative_u,
        jet.derivative_u.to_array().map(|x| -x),
    );
    near(reversed.derivative_v, jet.derivative_v.to_array());
    near(reversed.derivative_uu, jet.derivative_uu.to_array());
    near(
        reversed.derivative_uv,
        jet.derivative_uv.to_array().map(|x| -x),
    );
    near(reversed.derivative_vv, jet.derivative_vv.to_array());
    let swapped = source
        .try_swapped_uv()
        .unwrap()
        .evaluate_with_second_derivatives(0.5, 0.25)
        .unwrap();
    near(swapped.derivative_u, jet.derivative_v.to_array());
    near(swapped.derivative_v, jet.derivative_u.to_array());
    near(swapped.derivative_uu, jet.derivative_vv.to_array());
    near(swapped.derivative_uv, jet.derivative_uv.to_array());
    near(swapped.derivative_vv, jet.derivative_uu.to_array());
    let mapped = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        source.control_points().to_vec(),
        vec![-7.0, -7.0, 13.0, 13.0],
        vec![2.0, 2.0, 6.0, 6.0],
    )
    .unwrap()
    .evaluate_with_second_derivatives(-2.0, 4.0)
    .unwrap();
    near(
        mapped.derivative_u,
        jet.derivative_u.to_array().map(|x| x / 20.0),
    );
    near(
        mapped.derivative_v,
        jet.derivative_v.to_array().map(|x| x / 4.0),
    );
    near(
        mapped.derivative_uu,
        jet.derivative_uu.to_array().map(|x| x / 400.0),
    );
    near(
        mapped.derivative_uv,
        jet.derivative_uv.to_array().map(|x| x / 80.0),
    );
    near(
        mapped.derivative_vv,
        jet.derivative_vv.to_array().map(|x| x / 16.0),
    );
}

#[test]
fn all_surface_partials_are_translation_invariant_when_controls_translate_exactly() {
    let source = rational_plane();
    let shift = Vector3::try_new(1e12, -2e12, 3e12).unwrap();
    let translated = source
        .transformed(AffineTransform3::from_translation(shift))
        .unwrap();
    for (u, v) in [(0.0, 0.0), (0.25, 0.625), (1.0 / 3.0, 0.4), (1.0, 1.0)] {
        let mut expected = source.evaluate_with_second_derivatives(u, v).unwrap();
        expected.point = expected.point.translated(shift).unwrap();
        assert_eq!(
            translated.evaluate_with_second_derivatives(u, v).unwrap(),
            expected
        );
    }
}

#[test]
fn continuation_uses_exact_derivatives_and_rejects_signed_weight_poles() {
    let surface = rational_plane();
    assert!(surface.evaluate(-0.25, 1.5).is_err());
    let jet = surface
        .evaluate_extended_with_second_derivatives(-0.25, 1.5)
        .unwrap();
    near(jet.derivative_u, [8.0 / 0.75_f64.powi(2), 0.0, 0.0]);
    near(jet.derivative_v, [0.0, 3.0, 0.0]);
    near(jet.derivative_uu, [-16.0 / 0.75_f64.powi(3), 0.0, 0.0]);
    near(jet.derivative_uv, [0.0; 3]);
    near(jet.derivative_vv, [0.0; 3]);
    assert!(matches!(
        surface.evaluate_extended(-1.0, 0.5),
        Err(crate::GeometryError::ZeroWeightAtParameter)
    ));
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            surface
                .evaluate_extended_with_second_derivatives(bad, 0.5)
                .is_err()
        );
        assert!(surface.evaluate_with_second_derivatives(0.5, bad).is_err());
    }
}

#[test]
fn point_only_evaluation_survives_overflowing_offsets_and_unrequested_partials() {
    let make = |xs: [f64; 2], weights: [f64; 2]| {
        NurbsSurface::try_new_rational(
            1,
            1,
            2,
            2,
            [0.0, 1.0]
                .into_iter()
                .flat_map(|y| {
                    xs.into_iter()
                        .zip(weights)
                        .map(move |(x, w)| WeightedPoint3::try_new(p(x, y, 0.0), w).unwrap())
                })
                .collect(),
            vec![0.0, 0.0, 1.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap()
    };
    let wide = make([-1e308, 1e308], [1.0, 1.0]);
    assert_eq!(wide.evaluate(0.5, 0.25).unwrap(), p(0.0, 0.25, 0.0));
    assert!(wide.evaluate_with_derivatives(0.5, 0.25).is_err());
    let signed = make([1e308, 5e307], [1.0, -1.0]);
    let actual = signed.evaluate(0.58, 0.25).unwrap();
    assert!(((actual.x() + 8.125e307) / 8.125e307).abs() < 2e-14);
    assert!((actual.y() - 0.25).abs() < 2e-12);
    assert!(matches!(
        signed.evaluate(0.5, 0.25),
        Err(crate::GeometryError::ZeroWeightAtParameter)
    ));
}
