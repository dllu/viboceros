use super::*;
use crate::{Point3, WeightedPoint3};

fn point(p: [f64; 3]) -> Point3 {
    Point3::try_from(p).unwrap()
}
fn bezier(points: &[[f64; 3]], weights: &[f64]) -> NurbsCurve {
    let degree = points.len() - 1;
    let controls = points
        .iter()
        .zip(weights)
        .map(|(p, w)| WeightedPoint3::try_new(point(*p), *w).unwrap())
        .collect();
    let knots = std::iter::repeat_n(0., degree + 1)
        .chain(std::iter::repeat_n(1., degree + 1))
        .collect();
    NurbsCurve::try_new_rational(degree, controls, knots).unwrap()
}
fn accuracy() -> Tolerance {
    Tolerance::try_new(1e-11, 1e-14, 1e-10).unwrap()
}
fn near(actual: Point3, expected: [f64; 3]) {
    assert!(
        actual.distance_to(point(expected)).unwrap() < 1e-10,
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn polynomial_extrema_are_tight_instead_of_control_point_bounds() {
    let c = bezier(&[[1., 2., 3.], [8., 16., 9.], [10., 4., 5.]], &[1.; 3]);
    let b = c.tight_bounds(accuracy()).unwrap();
    near(b.min(), [1., 2., 3.]);
    near(b.max(), [10., 124. / 13., 6.6]);
    assert_eq!(c.control_point_bounds().max().y(), 16.);
}

#[test]
fn rational_hulls_cover_extrema_under_positive_negative_and_mixed_weight_gauges() {
    for gauge in [1., 1e-200, 1e200, -1.] {
        let c = bezier(
            &[[1., 0., 0.], [1., 1., 0.], [0., 1., 0.]],
            &[gauge, gauge * std::f64::consts::FRAC_1_SQRT_2, gauge],
        );
        let b = c.tight_bounds(accuracy()).unwrap();
        near(b.min(), [0.; 3]);
        near(b.max(), [1., 1., 0.]);
    }
    let c = bezier(
        &[[0., 0., 0.], [1., 10., 0.], [2., 0., 0.]],
        &[1., -0.1, 1.],
    );
    let b = c.tight_bounds(accuracy()).unwrap();
    assert!((b.min().y() + 10. / 9.).abs() < 1e-10);
    for i in 0..=10000 {
        for (axis, coordinate) in c
            .evaluate(f64::from(i) / 10000.)
            .unwrap()
            .to_array()
            .into_iter()
            .enumerate()
        {
            assert!(coordinate >= b.min().to_array()[axis] - 1e-11);
            assert!(coordinate <= b.max().to_array()[axis] + 1e-11);
        }
    }
}

#[test]
fn bounds_clamp_unused_controls_and_include_both_full_order_limits() {
    let c = NurbsCurve::try_new(
        2,
        [
            [100., 10., 0.],
            [0., 1., 0.],
            [0., -1., 0.],
            [100., -10., 0.],
        ]
        .map(point)
        .to_vec(),
        (0..7).map(f64::from).collect(),
    )
    .unwrap();
    let b = c.tight_bounds(accuracy()).unwrap();
    near(b.min(), [0., -5.5, 0.]);
    near(b.max(), [50., 5.5, 0.]);
    let c = NurbsCurve::try_new(
        1,
        [[0., 0., 0.], [1., 2., 3.], [10., 0., 0.], [11., -2., 5.]]
            .map(point)
            .to_vec(),
        vec![0., 0., 1., 1., 2., 2.],
    )
    .unwrap();
    let b = c.tight_bounds(accuracy()).unwrap();
    near(b.min(), [0., -2., 0.]);
    near(b.max(), [11., 2., 5.]);
}

#[test]
fn curves_with_unresolved_poles_fail_instead_of_returning_control_hulls() {
    let c = bezier(&[[0., 0., 0.], [1., 10., 0.], [2., 0., 0.]], &[1., -1., 1.]);
    assert!(c.tight_bounds(accuracy()).is_err());
}

#[test]
fn regular_curve_with_a_projective_midpoint_control_uses_an_alternative_cut() {
    let c = bezier(&[[0., 0., 0.], [1., 10., 0.], [2., 0., 0.]], &[1., -1., 2.]);
    assert!(c.try_split(0.5).is_err());
    let b = c.tight_bounds(accuracy()).unwrap();
    assert!(
        (b.min().y() + 10. * (1. + 2f64.sqrt())).abs() < 1e-10,
        "{b:?}"
    );
    for i in 0..=10000 {
        let sample = c.evaluate(f64::from(i) / 10000.).unwrap().to_array();
        for (axis, coordinate) in sample.into_iter().enumerate() {
            assert!(coordinate >= b.min().to_array()[axis] - 1e-11);
            assert!(coordinate <= b.max().to_array()[axis] + 1e-11);
        }
    }
}

#[test]
fn closed_rational_curves_cover_all_spans_without_cyclic_seam_relocation() {
    let normal = crate::Vector3::try_new(2., 3., 4.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let circle = crate::Circle3::try_new(point([1., 2., 3.]), 5., normal, accuracy()).unwrap();
    let bounds = circle.to_nurbs().unwrap().tight_bounds(accuracy()).unwrap();
    near(bounds.min(), circle.bounds().min().to_array());
    near(bounds.max(), circle.bounds().max().to_array());
}
