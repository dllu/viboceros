use super::*;

fn bilinear(controls: [(Point3, f64); 4]) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        controls
            .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
            .to_vec(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

#[test]
fn range_loss_does_not_turn_a_finite_surface_edge_into_a_pole() {
    let surface = bilinear([
        (p(0., 0., 0.), f64::MIN_POSITIVE),
        (p(4., 0., 0.), f64::MIN_POSITIVE),
        (p(0., 3., 0.), f64::MAX),
        (p(4., 3., 0.), f64::MAX),
    ]);
    assert_eq!(surface.evaluate(0.375, 0.).unwrap(), p(1.5, 0., 0.));
}

#[test]
fn range_loss_recovers_a_finite_point_not_just_reported_errors() {
    let small = 2_f64.powi(-700);
    let large = 2_f64.powi(700);
    let surface = bilinear([
        (p(f64::MAX, 0., 0.), small),
        (p(0., 0., 0.), 1.),
        (p(0., 1., 0.), large),
        (p(1., 1., 0.), large),
    ]);
    let expected = f64::MAX * small;
    assert_eq!(surface.evaluate(0.5, 0.).unwrap().x(), expected);
}

#[test]
fn range_loss_retains_weighted_coordinates_and_finite_differential_jets() {
    let small = 1e-200;
    let surface = bilinear([
        (p(0., 0., 0.), 1.),
        (p(small, 0., 0.), small),
        (p(0., 1., 0.), 1.),
        (p(small, 1., 0.), small),
    ]);
    // S=(a²u/(1-u+au), v, 0), a=1e-200. At u=1: x=a,
    // x_u=1, x_uu=2(1-a)/a. Computing a² in binary64 erases all three.
    let jet = surface.evaluate_with_second_derivatives(1., 0.25).unwrap();
    assert_eq!(jet.point, p(small, 0.25, 0.));
    assert_eq!(jet.derivative_u.to_array(), [1., 0., 0.]);
    assert_eq!(jet.derivative_v.to_array(), [0., 1., 0.]);
    assert_eq!(jet.derivative_uu.x(), 2. / small);
    assert_eq!(jet.derivative_uv.to_array(), [0.; 3]);
    assert_eq!(jet.derivative_vv.to_array(), [0.; 3]);
}

#[test]
fn range_loss_retains_constant_jets_despite_large_signed_cancellation() {
    let point = p(1., 2., 3.);
    let surface = bilinear([
        (point, f64::MIN_POSITIVE),
        (point, f64::MIN_POSITIVE),
        (point, f64::MAX),
        (point, -f64::MAX),
    ]);
    let jet = surface.evaluate_with_second_derivatives(0.5, 0.25).unwrap();
    assert_eq!(jet.point, point);
    for derivative in [
        jet.derivative_u,
        jet.derivative_v,
        jet.derivative_uu,
        jet.derivative_uv,
        jet.derivative_vv,
    ] {
        assert_eq!(derivative.to_array(), [0.; 3]);
    }
    assert!(matches!(
        surface.evaluate(0.5, 1.),
        Err(crate::GeometryError::ZeroWeightAtParameter)
    ));
    assert!(matches!(
        surface.evaluate_with_second_derivatives(0.5, 1.),
        Err(crate::GeometryError::ZeroWeightAtParameter)
    ));
}

#[test]
fn range_loss_does_not_hide_genuine_point_overflow_or_a_pole() {
    let tiny = f64::from_bits(1);
    let surface = bilinear([
        (p(f64::MAX, 0., 0.), 1.),
        (p(-f64::MAX, 0., 0.), -1.),
        (p(0., 1., 0.), tiny),
        (p(0., 1., 0.), tiny),
    ]);
    // On v=0: x=MAX/(1-2u). The exact finite rational at u=1/4
    // overflows binary64; u=1/2 is a genuine zero denominator.
    assert!(matches!(
        surface.evaluate(0.25, 0.),
        Err(crate::GeometryError::NonFinite { .. })
    ));
    assert_eq!(
        surface.evaluate(0.5, 0.),
        Err(crate::GeometryError::ZeroWeightAtParameter)
    );
    assert_eq!(surface.evaluate(0., 0.).unwrap(), p(f64::MAX, 0., 0.));
    assert!(matches!(
        surface.evaluate_with_derivatives(0., 0.),
        Err(crate::GeometryError::NonFinite { .. })
    ));
}

fn constant_jet(jet: crate::SurfaceJet2, point: Point3) {
    assert_eq!(jet.point, point);
    for derivative in [
        jet.derivative_u,
        jet.derivative_v,
        jet.derivative_uu,
        jet.derivative_uv,
        jet.derivative_vv,
    ] {
        assert_eq!(derivative.to_array(), [0.; 3]);
    }
}

#[test]
fn range_loss_respects_four_one_sided_limits_and_grid_default_sides() {
    use crate::ParameterSide::{Left, Right};
    let surface = NurbsSurface::try_new_rational(
        1,
        1,
        4,
        4,
        (0..4)
            .flat_map(|j| {
                (0..4).map(move |i| {
                    let point = p((i / 2) as f64, (j / 2) as f64, 3.);
                    let weight = if (i + j) % 2 == 0 {
                        f64::MIN_POSITIVE
                    } else {
                        f64::MAX
                    };
                    WeightedPoint3::try_new(point, weight).unwrap()
                })
            })
            .collect(),
        vec![0., 0., 1., 1., 2., 2.],
        vec![0., 0., 1., 1., 2., 2.],
    )
    .unwrap();
    let original = surface.clone();
    for (side_u, x) in [(Left, 0.), (Right, 1.)] {
        for (side_v, y) in [(Left, 0.), (Right, 1.)] {
            constant_jet(
                surface
                    .evaluate_with_second_derivatives_on_sides(1., 1., side_u, side_v)
                    .unwrap(),
                p(x, y, 3.),
            );
        }
    }
    let samples = [0., 0.25, 1., 1.75, 2.];
    let mut visited = 0;
    surface.for_each_grid_point(&samples, &samples, |i, j, result| {
        assert_eq!(
            result.unwrap(),
            p(f64::from(samples[i] >= 1.), f64::from(samples[j] >= 1.), 3.)
        );
        visited += 1;
    });
    assert_eq!(visited, 25);
    assert_eq!(surface, original);
}

#[test]
fn range_loss_on_unclamped_spans_preserves_independent_curve_partials() {
    let row = [
        (p(0., 0., 0.), 1.),
        (p(3., 0., 2.), 2.),
        (p(0., 0., 3.), 3.),
        (p(-1., 0., 1.), 2.),
        (p(2., 0., 0.), 1.),
    ];
    let knots = vec![-2., -1., 0., 1., 2., 3., 4., 5.];
    let reference = NurbsCurve::try_new_rational(
        2,
        row.map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
            .to_vec(),
        knots.clone(),
    )
    .unwrap();
    let surface = NurbsSurface::try_new_rational(
        2,
        1,
        5,
        2,
        [(0., 2_f64.powi(-700)), (3., 2_f64.powi(700))]
            .into_iter()
            .flat_map(|(y, scale)| {
                row.map(move |(point, weight)| {
                    WeightedPoint3::try_new(p(point.x(), y, point.z()), weight * scale).unwrap()
                })
            })
            .collect(),
        knots,
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    for u in [0., 0.25, 1., 1.75, 2., 2.25, 3.] {
        for side in [crate::ParameterSide::Left, crate::ParameterSide::Right] {
            let expected = reference
                .evaluate_with_second_derivative_on_side(u, side)
                .unwrap();
            let actual = surface
                .evaluate_with_second_derivatives_on_sides(u, 0.5, side, side)
                .unwrap();
            assert!(
                actual
                    .point
                    .distance_to(p(expected.0.x(), 3., expected.0.z()))
                    .unwrap()
                    < 2e-12
            );
            near(actual.derivative_u, expected.1.to_array());
            near(actual.derivative_uu, expected.2.to_array());
            for derivative in [
                actual.derivative_v,
                actual.derivative_uv,
                actual.derivative_vv,
            ] {
                assert_eq!(derivative.to_array(), [0.; 3]);
            }
        }
    }
}

#[test]
fn range_loss_constant_continuation_survives_extreme_domain_derivative_factors() {
    let point = p(f64::MAX, -f64::MAX, f64::MIN_POSITIVE);
    let tiny = f64::from_bits(1);
    let surface = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        [tiny, f64::MAX, tiny, f64::MAX]
            .map(|w| WeightedPoint3::try_new(point, w).unwrap())
            .to_vec(),
        vec![0., 0., tiny, tiny],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    for u in [0., tiny] {
        constant_jet(
            surface.evaluate_with_second_derivatives(u, 0.25).unwrap(),
            point,
        );
    }
    for u in [-tiny, 2. * tiny] {
        constant_jet(
            surface
                .evaluate_extended_with_second_derivatives(u, -0.5)
                .unwrap(),
            point,
        );
    }
    assert!(surface.evaluate(-tiny, 0.25).is_err());
    assert!(surface.evaluate_extended(f64::INFINITY, 0.25).is_err());
    assert!(surface.evaluate_extended(0., f64::NAN).is_err());
}
