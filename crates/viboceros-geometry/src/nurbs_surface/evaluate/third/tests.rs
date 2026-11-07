use super::*;
use crate::WeightedPoint3;
fn close(a: Vector3, b: [Real; 3]) {
    for (a, b) in a.to_array().into_iter().zip(b) {
        assert!(
            (a - b).abs() <= 1e-11 * a.abs().max(b.abs()).max(1.),
            "{a:e} != {b:e}"
        );
    }
}
fn quotient(signed: bool) -> NurbsSurface {
    let (weights, end) = if signed {
        ([-1., 1.], 1.)
    } else {
        ([1., 2.], 0.5)
    };
    NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        (0..2)
            .flat_map(|v| {
                (0..2).map(move |u| {
                    WeightedPoint3::try_new(
                        Point3::try_new(u as Real * end, v as Real, u as Real * v as Real * end)
                            .unwrap(),
                        weights[u],
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}
#[test]
fn rational_degree_one_retains_nonzero_third_and_mixed_partials() {
    for signed in [false, true] {
        let s = quotient(signed);
        let u = 0.25;
        let v = 0.75;
        let jet = s.evaluate_with_third_derivatives(u, v).unwrap();
        let (w, second, third) = if signed {
            let w = 2. * u - 1.;
            (w, 4. / Real::powi(w, 3), -24. / Real::powi(w, 4))
        } else {
            let w = 1. + u;
            (w, -2. / Real::powi(w, 3), 6. / Real::powi(w, 4))
        };
        close(jet.derivative_uuu, [third, 0., v * third]);
        close(jet.derivative_uuv, [0., 0., second]);
        close(jet.derivative_uvv, [0.; 3]);
        close(jet.derivative_vvv, [0.; 3]);
        close(jet.second.derivative_uu, [second, 0., v * second]);
        assert!(w != 0.);
        for (a, b) in jet
            .partials()
            .into_iter()
            .zip(s.exact_third_jet([u, v], [1, 1]).unwrap().partials())
        {
            close(a, b.to_array());
        }
    }
    assert!(matches!(
        quotient(true).evaluate_with_third_derivatives(0.5, 0.25),
        Err(GeometryError::ZeroWeightAtParameter)
    ));
}
#[test]
fn rational_continuation_evaluates_the_same_analytic_third_derivative() {
    let jet = quotient(false)
        .evaluate_extended_with_third_derivatives(2., -0.5)
        .unwrap();
    close(jet.derivative_uuu, [6. / 81., 0., -3. / 81.]);
    close(jet.derivative_uuv, [0., 0., -2. / 27.]);
    assert!(
        quotient(false)
            .evaluate_with_third_derivatives(2., -0.5)
            .is_err()
    );
}
#[test]
fn third_partials_choose_exact_one_sided_limits_at_a_multiple_span() {
    let s = NurbsSurface::try_new_rational(
        3,
        1,
        5,
        2,
        (0..2)
            .flat_map(|v| {
                [0., 1. / 6., 0.5, 5. / 6., 1.]
                    .into_iter()
                    .enumerate()
                    .map(move |(i, u)| {
                        WeightedPoint3::try_new(
                            Point3::try_new(u, v as Real, if i == 4 { 1. } else { 0. }).unwrap(),
                            1.,
                        )
                        .unwrap()
                    })
            })
            .collect(),
        vec![0., 0., 0., 0., 0.5, 1., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    close(
        s.evaluate_with_third_derivatives_on_sides(
            0.5,
            0.4,
            ParameterSide::Left,
            ParameterSide::Right,
        )
        .unwrap()
        .derivative_uuu,
        [0.; 3],
    );
    close(
        s.evaluate_with_third_derivatives(0.5, 0.4)
            .unwrap()
            .derivative_uuu,
        [0., 0., 48.],
    );
}
#[test]
fn exact_recovery_cancels_overflowing_derivative_control_nets_before_rounding() {
    let s = NurbsSurface::try_new_rational(
        4,
        1,
        5,
        2,
        (0..2)
            .flat_map(|v| {
                (0..5).map(move |u| {
                    WeightedPoint3::try_new(
                        Point3::try_new(if u % 2 == 0 { 1e307 } else { -1e307 }, v as Real, 0.)
                            .unwrap(),
                        1.,
                    )
                    .unwrap()
                })
            })
            .collect(),
        [vec![0.; 5], vec![1.; 5]].concat(),
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let j = s.evaluate_with_third_derivatives(0.5, 0.25).unwrap();
    assert_eq!(j.second.point, Point3::try_new(0., 0.25, 0.).unwrap());
    close(j.derivative_uuu, [0.; 3]);
    close(j.second.derivative_uu, [0.; 3]);
    close(j.second.derivative_u, [0.; 3]);
}
