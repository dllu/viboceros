use super::*;
use crate::{Point3, WeightedPoint3};
fn curves() -> [NurbsCurve; 2] {
    [
        [[0., 0., 0.], [3., 5., 0.], [7., -2., 0.], [10., 3., 0.]],
        [[0., 0., 2.], [3., 0., -3.], [7., 0., 4.], [10., 0., 1.]],
    ]
    .map(|points| {
        NurbsCurve::try_new(
            3,
            points.map(|p| Point3::try_from(p).unwrap()).to_vec(),
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap()
    })
}
fn directions() -> [Vector3; 2] {
    [[0., 0., 1.], [0., 1., 0.]].map(|p| Vector3::try_from(p).unwrap())
}
#[test]
fn aligned_profiles_reconstruct_the_complete_spatial_cubic_without_sampling() {
    let curves = curves();
    let output =
        curves_from_two_views([&curves[0], &curves[1]], directions(), Tolerance::DEFAULT).unwrap();
    assert_eq!(output.len(), 1);
    assert_eq!(output[0].control_points().len(), 4);
    for i in 0..=100 {
        let t = i as f64 / 100.;
        let a = curves[0].evaluate(t).unwrap();
        let b = curves[1].evaluate(t).unwrap();
        let p = output[0].evaluate(t).unwrap();
        assert!(
            p.distance_to(Point3::try_new(a.x(), a.y(), b.z()).unwrap())
                .unwrap()
                < 1e-12
        );
    }
}

#[test]
fn shared_rational_chart_survives_weight_gauges_reversal_and_independent_domains() {
    let curves = curves();
    let weights = [1., 0.5, 2., 1.];
    let curves: [NurbsCurve; 2] = std::array::from_fn(|i| {
        NurbsCurve::try_new_rational(
            3,
            curves[i]
                .control_points()
                .iter()
                .zip(weights)
                .map(|(p, w)| {
                    WeightedPoint3::try_new(p.point(), w * if i == 0 { -8. } else { 2. }).unwrap()
                })
                .collect(),
            curves[i].knots().to_vec(),
        )
        .unwrap()
    });
    let second = curves[1]
        .reversed()
        .unwrap()
        .try_reparameterized(100.0..=400.0)
        .unwrap();
    let first = curves[0].try_reparameterized(-20.0..=-2.0).unwrap();
    let result = aligned::reconstruct([&first, &second], directions())
        .unwrap()
        .unwrap();
    for i in 0..=100 {
        let t = i as f64 / 100.;
        let a = curves[0].evaluate(t).unwrap();
        let b = curves[1].evaluate(t).unwrap();
        assert!(
            result
                .evaluate(t)
                .unwrap()
                .distance_to(Point3::try_new(a.x(), a.y(), b.z()).unwrap())
                .unwrap()
                < 1e-12
        );
    }
}

#[test]
fn turning_profiles_with_identical_clocks_still_use_the_general_intersector() {
    let mut curves = curves();
    for curve in &mut curves {
        let controls = curve
            .control_points()
            .iter()
            .zip([0., -5., 15., 10.])
            .map(|(p, x)| {
                WeightedPoint3::try_new(
                    Point3::try_new(x, p.point().y(), p.point().z()).unwrap(),
                    p.weight(),
                )
                .unwrap()
            })
            .collect();
        *curve = NurbsCurve::try_new_rational(3, controls, curve.knots().to_vec()).unwrap();
    }
    let mut called = false;
    let result = curves_from_two_views_with_operation(
        [&curves[0], &curves[1]],
        directions(),
        Tolerance::DEFAULT,
        &mut |_, _, _| {
            called = true;
            Ok::<_, GeometryError>(Vec::new())
        },
    )
    .unwrap();
    assert!(called);
    assert!(result.is_empty());
}
#[test]
fn projector_hulls_cover_every_control_pair_in_oblique_directions() {
    let curves = curves();
    let d = [[0.2, 0., 1.], [0., 1., 0.3]].map(|p| Vector3::try_from(p).unwrap());
    let d = validate([&curves[0], &curves[1]], d, Tolerance::DEFAULT).unwrap();
    let projectors = projectors([&curves[0], &curves[1]], d, Tolerance::DEFAULT).unwrap();
    let n = d[0].cross(d[1]).unwrap();
    let dual = d[1]
        .cross(n)
        .unwrap()
        .scaled(1. / n.dot(n).unwrap())
        .unwrap();
    for a in curves[0].control_points() {
        for b in curves[1].control_points() {
            let alpha = dual.dot_point_difference(b.point(), a.point());
            let point = a.point().translated(d[0].scaled(alpha).unwrap()).unwrap();
            let bounds = projectors[0].control_point_bounds();
            for (x, [lo, hi]) in point.to_array().into_iter().zip([
                [bounds.min().x(), bounds.max().x()],
                [bounds.min().y(), bounds.max().y()],
                [bounds.min().z(), bounds.max().z()],
            ]) {
                assert!(x >= lo - 1e-12 && x <= hi + 1e-12);
            }
        }
    }
}
#[test]
fn parallel_directions_and_mixed_sign_weights_fail_before_backend() {
    let mut curves = curves();
    let mut called = false;
    let mut operation = |_: &NurbsSurface, _: &NurbsSurface, _: Tolerance| {
        called = true;
        Ok::<_, GeometryError>(Vec::new())
    };
    assert!(matches!(
        curves_from_two_views_with_operation(
            [&curves[0], &curves[1]],
            [directions()[0]; 2],
            Tolerance::DEFAULT,
            &mut operation
        ),
        Err(GeometryError::ParallelCurveViewDirections)
    ));
    let points = curves[0]
        .control_points()
        .iter()
        .enumerate()
        .map(|(i, p)| WeightedPoint3::try_new(p.point(), if i == 1 { -1. } else { 1. }).unwrap())
        .collect();
    curves[0] = NurbsCurve::try_new_rational(3, points, curves[0].knots().to_vec()).unwrap();
    assert!(
        curves_from_two_views_with_operation(
            [&curves[0], &curves[1]],
            directions(),
            Tolerance::DEFAULT,
            &mut operation
        )
        .is_err()
    );
    assert!(!called);
}
