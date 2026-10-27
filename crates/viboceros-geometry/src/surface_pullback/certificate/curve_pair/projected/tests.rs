use super::*;
use crate::{Point3, WeightedPoint3};

fn line(a: Real, b: Real) -> NurbsCurve {
    NurbsCurve::try_new(
        1,
        vec![
            Point3::try_new(a, 0., 0.).unwrap(),
            Point3::try_new(b, 0., 0.).unwrap(),
        ],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

#[test]
fn nonlinear_rational_and_backtracking_maps_cover_both_complete_images() {
    for (degree, coordinates) in [
        (3, vec![0., 0.0625, 0.5, 1.]),
        (5, vec![0., 1., 1., 0., 0., 1.]),
    ] {
        for gauge in [1., -8.] {
            let source = NurbsCurve::try_new_rational(
                degree,
                coordinates
                    .iter()
                    .enumerate()
                    .map(|(i, &x)| {
                        WeightedPoint3::try_new(
                            Point3::try_new(x, 0., 0.).unwrap(),
                            gauge * if i % 2 == 0 { 1. } else { 2. },
                        )
                        .unwrap()
                    })
                    .collect(),
                [vec![-3.; degree + 1], vec![7.; degree + 1]].concat(),
            )
            .unwrap();
            let target = line(0., 1.).try_reparameterized(2. ..=10.).unwrap();
            let before = (source.clone(), target.clone());
            assert_eq!(
                source.projected_curve_deviation_bound(&target, 0.).unwrap(),
                Some(0.)
            );
            assert_eq!(
                source
                    .monotone_projected_curve_deviation_bound(&target, 0.)
                    .unwrap(),
                (degree == 3).then_some(0.),
            );
            assert_eq!(
                source
                    .reversed()
                    .unwrap()
                    .projected_curve_deviation_bound(&target.reversed().unwrap(), 0.)
                    .unwrap(),
                Some(0.)
            );
            assert_eq!((source, target), before);
        }
    }
}

#[test]
fn source_jumps_cannot_hide_a_missing_part_of_the_target_image() {
    let source = NurbsCurve::try_new(
        1,
        [0., 0.125, 0.875, 1.]
            .map(|x| Point3::try_new(x, 0., 0.).unwrap())
            .to_vec(),
        vec![0., 0., 0.5, 0.5, 1., 1.],
    )
    .unwrap();
    // Each source point lies on the target, but the target's middle has no
    // source witness. An onto map cannot jump over that missing interval.
    assert_eq!(
        source
            .projected_curve_deviation_bound(&line(0., 1.), 0.)
            .unwrap(),
        None
    );
}

#[test]
fn an_isolated_zero_derivative_can_preserve_strict_parameter_order() {
    let source = NurbsCurve::try_new(
        3,
        [0., 1., 0., 1.]
            .map(|x| Point3::try_new(x, 0., 0.).unwrap())
            .to_vec(),
        [vec![0.; 4], vec![1.; 4]].concat(),
    )
    .unwrap();
    assert_eq!(source.derivative_at(0.5).unwrap().x(), 0.);
    assert_eq!(
        source
            .monotone_projected_curve_deviation_bound(&line(0., 1.), 0.)
            .unwrap(),
        Some(0.)
    );
}

#[test]
fn a_constant_parameter_interval_is_not_strictly_increasing() {
    let source = NurbsCurve::try_new(
        1,
        [0., 0., 0.5, 1.]
            .map(|x| Point3::try_new(x, 0., 0.).unwrap())
            .to_vec(),
        vec![0., 0., 0.25, 0.75, 1., 1.],
    )
    .unwrap();
    let target = line(0., 1.);
    assert_eq!(
        source.projected_curve_deviation_bound(&target, 0.).unwrap(),
        Some(0.)
    );
    assert_eq!(
        source
            .monotone_projected_curve_deviation_bound(&target, 0.)
            .unwrap(),
        None
    );
}

#[test]
fn endpoint_bounds_include_the_target_outside_the_source_subsegment() {
    let source = line(0.125, 0.875);
    let target = line(0., 1.);
    assert_eq!(
        source
            .projected_curve_deviation_bound(&target, 0.1)
            .unwrap(),
        None
    );
    assert_eq!(
        source
            .projected_curve_deviation_bound(&target, 0.125)
            .unwrap(),
        Some(0.125)
    );
}

#[test]
fn exact_proof_detects_a_polynomial_bump_between_five_matching_samples() {
    let source = NurbsCurve::try_new(
        5,
        [0., 6., -13., 13., -6., 0.]
            .into_iter()
            .enumerate()
            .map(|(i, y)| Point3::try_new(i as Real / 5., y, 0.).unwrap())
            .collect(),
        [vec![0.; 6], vec![1.; 6]].concat(),
    )
    .unwrap();
    for t in [0., 0.25, 0.5, 0.75, 1.] {
        assert!(source.evaluate(t).unwrap().y().abs() < 1e-14);
    }
    assert_eq!(
        source
            .projected_curve_deviation_bound(&line(0., 1.), 0.1)
            .unwrap(),
        None
    );
}

#[test]
fn coordinate_axes_scales_and_common_weight_gauges_retain_complete_bounds() {
    for axis in 0..3 {
        for scale in [2_f64.powi(-500), 1., 2_f64.powi(500)] {
            let point = |x| {
                Point3::try_from(std::array::from_fn(
                    |i| if i == axis { x * scale } else { 0. },
                ))
                .unwrap()
            };
            let source = NurbsCurve::try_new_rational(
                3,
                [0., 0.0625, 0.5, 1.]
                    .map(|x| WeightedPoint3::try_new(point(x), -2.).unwrap())
                    .to_vec(),
                [vec![0.; 4], vec![1.; 4]].concat(),
            )
            .unwrap();
            let target =
                NurbsCurve::try_new(1, vec![point(0.), point(1.)], vec![0., 0., 1., 1.]).unwrap();
            assert_eq!(
                source.projected_curve_deviation_bound(&target, 0.).unwrap(),
                Some(0.)
            );
        }
    }
}

#[test]
fn subnormal_offsets_do_not_become_zero_distance() {
    let tiny = Real::from_bits(1);
    let source = NurbsCurve::try_new_rational(
        3,
        [0., 0.0625, 0.5, 1.]
            .map(|x| WeightedPoint3::try_new(Point3::try_new(x, 0., tiny).unwrap(), -8.).unwrap())
            .to_vec(),
        [vec![0.; 4], vec![1.; 4]].concat(),
    )
    .unwrap();
    let target = line(0., 1.);
    assert_eq!(
        source.projected_curve_deviation_bound(&target, 0.).unwrap(),
        None
    );
    assert_eq!(
        source
            .projected_curve_deviation_bound(&target, tiny)
            .unwrap(),
        Some(tiny)
    );
}

#[test]
fn finite_coordinates_with_an_overflowing_endpoint_difference_are_supported() {
    let source = NurbsCurve::try_new(
        3,
        [-1e308, -5e307, 5e307, 1e308]
            .map(|x| Point3::try_new(x, 0., 0.).unwrap())
            .to_vec(),
        [vec![0.; 4], vec![1.; 4]].concat(),
    )
    .unwrap();
    let target = line(-1e308, 1e308);
    assert_eq!(
        source.projected_curve_deviation_bound(&target, 0.).unwrap(),
        Some(0.)
    );
}

#[test]
fn invalid_tolerances_are_errors_and_unsupported_proposals_are_inconclusive() {
    let target = line(0., 1.);
    for limit in [-1., Real::NAN, Real::INFINITY] {
        assert_eq!(
            target.projected_curve_deviation_bound(&target, limit),
            Err(GeometryError::InvalidTolerance)
        );
    }
    let constant = line(0., 0.);
    assert_eq!(
        target
            .projected_curve_deviation_bound(&constant, 1.)
            .unwrap(),
        None
    );
    assert_eq!(
        line(-1., 2.)
            .projected_curve_deviation_bound(&target, 1.)
            .unwrap(),
        None
    );
    let unclamped = NurbsCurve::try_new(
        2,
        [0., 0.5, 1.]
            .map(|x| Point3::try_new(x, 0., 0.).unwrap())
            .to_vec(),
        vec![-1., -1., 0., 1., 2., 2.],
    )
    .unwrap();
    assert_eq!(
        unclamped
            .projected_curve_deviation_bound(&target, 1.)
            .unwrap(),
        None
    );
    let mixed = NurbsCurve::try_new_rational(
        1,
        [(-1., 1.), (1., -1.)]
            .map(|(x, w)| WeightedPoint3::try_new(Point3::try_new(x, 0., 0.).unwrap(), w).unwrap())
            .to_vec(),
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert_eq!(
        mixed.projected_curve_deviation_bound(&target, 1.).unwrap(),
        None
    );
    let high = NurbsCurve::try_new(
        17,
        (0..18)
            .map(|i| Point3::try_new(i as Real / 17., 0., 0.).unwrap())
            .collect(),
        [vec![0.; 18], vec![1.; 18]].concat(),
    )
    .unwrap();
    assert_eq!(
        high.projected_curve_deviation_bound(&target, 1.).unwrap(),
        None
    );
}
