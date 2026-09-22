use super::*;

fn plane(size: f64, end: f64, weight: f64) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        [
            p(0., 0., 0.),
            p(size, 0., 0.),
            p(0., size, 0.),
            p(size, size, 0.),
        ]
        .map(|p| WeightedPoint3::try_new(p, weight).unwrap())
        .to_vec(),
        vec![0., 0., end, end],
        vec![0., 0., end, end],
    )
    .unwrap()
}

#[test]
fn regular_surface_normal_is_independent_of_model_and_parameter_scale() {
    for size in [1e-200, 1., 1e200] {
        for end in [1e-200, 1., 1e200] {
            let surface = plane(size, end, 1.);
            assert_eq!(
                surface
                    .normal_at(end * 0.5, end * 0.5)
                    .unwrap()
                    .as_vector()
                    .to_array(),
                [0., 0., 1.],
                "size={size}, domain={end}"
            );
        }
    }
}

#[test]
fn regular_surface_normal_does_not_cross_already_rounded_partials() {
    let a = 2_f64.powi(53);
    let surface = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        [
            p(-1., 0., 0.),
            p(a, a, 0.),
            p(a - 1., a, 0.),
            p(2. * a, 2. * a, 0.),
        ]
        .map(|p| WeightedPoint3::try_new(p, 1.).unwrap())
        .to_vec(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    // At (0,0), exact Su=(a+1,a,0) and Sv=(a,a,0), so Su×Sv=(0,0,a).
    // Both representable first derivatives round to the same (a,a,0).
    let (_, du, dv) = surface.evaluate_with_derivatives(0., 0.).unwrap();
    assert_eq!(du, dv);
    assert_eq!(
        surface.normal_at(0., 0.).unwrap().as_vector().to_array(),
        [0., 0., 1.]
    );
    assert_eq!(
        surface
            .try_swapped_uv()
            .unwrap()
            .normal_at(0., 0.)
            .unwrap()
            .as_vector()
            .to_array(),
        [0., 0., -1.]
    );
}

#[test]
fn regular_surface_normal_is_independent_of_homogeneous_gauge() {
    for weight in [
        -f64::MAX,
        -1.,
        -f64::MIN_POSITIVE,
        f64::MIN_POSITIVE,
        1.,
        f64::MAX,
    ] {
        let surface = plane(1., 1., weight);
        assert_eq!(
            surface
                .normal_at(0.25, 0.75)
                .unwrap()
                .as_vector()
                .to_array(),
            [0., 0., 1.]
        );
    }
}

#[test]
fn a_regular_normal_does_not_require_a_representable_point_or_speed() {
    let surface = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        [0., 1.]
            .into_iter()
            .flat_map(|y| {
                [(f64::MAX, 1.), (-f64::MAX, -1.)]
                    .into_iter()
                    .map(move |(x, w)| WeightedPoint3::try_new(p(x, y, 0.), w).unwrap())
            })
            .collect(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    // S=(MAX/(1-2u),v,0). At either station the point and positive Su speed
    // exceed binary64, but the regular natural normal is exactly +Z.
    for u in [0.25, 0.75] {
        assert!(matches!(
            surface.evaluate(u, 0.5),
            Err(crate::GeometryError::NonFinite { .. })
        ));
        assert!(matches!(
            surface.evaluate_with_derivatives(u, 0.5),
            Err(crate::GeometryError::NonFinite { .. })
        ));
        assert_eq!(
            surface.normal_at(u, 0.5).unwrap().as_vector().to_array(),
            [0., 0., 1.]
        );
    }
    assert_eq!(
        surface.normal_at(0.5, 0.5),
        Err(crate::GeometryError::ZeroWeightAtParameter)
    );
}

#[test]
fn normals_reproduce_a_polynomial_on_unclamped_spans_and_endpoints() {
    // The Greville coefficients reproduce u and v exactly, so this tensor net
    // is S=(u,v,uv), with analytic normal proportional to (-v,-u,1).
    let us = [-0.5, 0.5, 1.5, 2.5];
    let vs = [-1., 0., 1., 2., 3., 4.];
    let surface = NurbsSurface::try_new_rational(
        2,
        3,
        4,
        6,
        vs.into_iter()
            .flat_map(|v| {
                us.into_iter()
                    .map(move |u| WeightedPoint3::try_new(p(u, v, u * v), 1.).unwrap())
            })
            .collect(),
        vec![-2., -1., 0., 1., 2., 3., 4.],
        vec![-3., -2., -1., 0., 1., 2., 3., 4., 5., 6.],
    )
    .unwrap();
    for u in [0., 0.5, 1., 1.75, 2.] {
        for v in [0., 0.25, 1., 1.75, 2., 3.] {
            let expected = Vector3::try_new(-v, -u, 1.)
                .unwrap()
                .normalized_nonzero()
                .unwrap();
            near(
                surface.normal_at(u, v).unwrap().as_vector(),
                expected.as_vector().to_array(),
            );
        }
    }
}

#[test]
fn normal_uses_right_span_at_a_crease_and_rejects_invalid_parameters() {
    let surface = NurbsSurface::try_new_rational(
        1,
        1,
        4,
        2,
        [0., 1.]
            .into_iter()
            .flat_map(|y| {
                [(0., 0.), (1., 0.), (1., 0.), (2., 1.)]
                    .into_iter()
                    .map(move |(x, z)| WeightedPoint3::try_new(p(x, y, z), 1.).unwrap())
            })
            .collect(),
        vec![0., 0., 1., 1., 2., 2.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert_eq!(
        surface
            .normal_at(1.0_f64.next_down(), 0.5)
            .unwrap()
            .as_vector()
            .to_array(),
        [0., 0., 1.]
    );
    for u in [1., 2.] {
        near(
            surface.normal_at(u, 0.5).unwrap().as_vector(),
            [-0.5_f64.sqrt(), 0., 0.5_f64.sqrt()],
        );
    }
    for uv in [
        [f64::NAN, 0.5],
        [f64::INFINITY, 0.5],
        [-f64::from_bits(1), 0.5],
        [2.0_f64.next_up(), 0.5],
        [0.5, -1.],
        [0.5, 1.0_f64.next_up()],
    ] {
        assert!(surface.normal_at(uv[0], uv[1]).is_err(), "{uv:?}");
    }
}
