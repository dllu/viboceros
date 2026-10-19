use super::*;
use crate::WeightedPoint3;
fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
#[test]
fn simplification_is_idempotent_and_retains_tiny_translated_and_weight_gauged_lines() {
    for (origin, size) in [(0., 1e-12), (1e9, 10.)] {
        for gauge in [1., 7., -7., 1e-200, 1e200] {
            let source = Curve3::NurbsCurve(
                NurbsCurve::try_new_rational(
                    3,
                    [0., 0.2, 0.7, 1.]
                        .into_iter()
                        .map(|x| {
                            WeightedPoint3::try_new(point(origin + size * x, 0., 0.), gauge)
                                .unwrap()
                        })
                        .collect(),
                    vec![-2.; 4].into_iter().chain([8.; 4]).collect(),
                )
                .unwrap(),
            );
            let before = source.clone();
            let tolerance = Tolerance::try_new(size * 0.001, 1e-12, 1e-6).unwrap();
            let first = simplify_curve(&source, tolerance).unwrap();
            let Curve3::Line(line) = &first.curve else {
                panic!()
            };
            assert_eq!(line.domain(), -2. ..=8.);
            assert_eq!(line.start(), point(origin, 0., 0.));
            assert_eq!(line.end(), point(origin + size, 0., 0.));
            assert!(first.rebuilt_representation);
            let second = simplify_curve(&first.curve, tolerance).unwrap();
            assert_eq!(second.curve, first.curve);
            assert!(!second.rebuilt_representation);
            assert_eq!(source, before);
        }
    }
}
#[test]
fn backtracking_and_untouched_polyline_children_are_preserved_exactly() {
    let tolerance = Tolerance::DEFAULT;
    let polyline = Polyline3::try_new(
        vec![
            point(0., 0., 0.),
            point(3., 0., 0.),
            point(1., 0., 0.),
            point(5., 0., 0.),
        ],
        tolerance,
    )
    .unwrap();
    let source = Curve3::Polyline(polyline);
    let result = simplify_curve(&source, tolerance).unwrap();
    assert_eq!(result.curve, source);
    assert!(!result.rebuilt_representation);
    let line = LineSegment::try_new(point(-2., 0., 0.), point(0., 0., 0.), tolerance).unwrap();
    let polyline = Polyline3::try_with_parameters(
        vec![point(0., 0., 0.), point(0., 3., 0.), point(4., 3., 0.)],
        vec![-7., -2., 8.],
        tolerance,
    )
    .unwrap();
    let source = Curve3::PolyCurve(
        PolyCurve3::try_with_segment_domains(
            vec![CurveSegment3::Line(line), CurveSegment3::Polyline(polyline)],
            vec![3., 5., 20.],
        )
        .unwrap(),
    );
    let result = simplify_curve(&source, tolerance).unwrap();
    let Curve3::Polyline(c) = result.curve else {
        panic!()
    };
    assert_eq!(c.parameters(), &[3., 5., 10., 20.]);
    assert!(result.rebuilt_representation);
}
#[test]
fn injected_split_failure_does_not_modify_the_source() {
    let source = Curve3::NurbsCurve(
        NurbsCurve::try_new(
            3,
            vec![
                point(0., 0., 0.),
                point(1., 1., 0.),
                point(2., 1., 0.),
                point(3., 0., 0.),
                point(4., 3., 0.),
                point(5., -1., 0.),
                point(6., 0., 0.),
            ],
            vec![0., 0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1., 1.],
        )
        .unwrap(),
    );
    let before = source.clone();
    let mut calls = 0;
    let result = simplify_curve_with_split(&source, Tolerance::DEFAULT, &mut |_, _| {
        calls += 1;
        Err::<(NurbsCurve, NurbsCurve), GeometryError>(GeometryError::InvalidCurveParameterInterval)
    });
    assert!(result.is_err());
    assert_eq!(calls, 1);
    assert_eq!(source, before);
}

#[test]
fn long_degree_one_curves_use_the_native_polyline_batch_without_knot_splitting() {
    let n = 20_000;
    let points = (0..n)
        .map(|i| point(i as f64, (i / 7 % 3) as f64, 0.))
        .collect::<Vec<_>>();
    let knots = std::iter::once(0.)
        .chain((0..n).map(|i| i as f64))
        .chain(std::iter::once((n - 1) as f64))
        .collect();
    let source = Curve3::NurbsCurve(
        NurbsCurve::try_new_rational(
            1,
            points
                .into_iter()
                .enumerate()
                .map(|(i, p)| WeightedPoint3::try_new(p, 1. + (i % 3) as f64).unwrap())
                .collect(),
            knots,
        )
        .unwrap(),
    );
    let result = simplify_curve_with_split(&source, Tolerance::DEFAULT, &mut |_,
                                                                              _|
     -> Result<
        (NurbsCurve, NurbsCurve),
        GeometryError,
    > {
        panic!("degree-one curves do not need knot splitting")
    })
    .unwrap();
    let Curve3::Polyline(c) = result.curve else {
        panic!()
    };
    assert_eq!(c.domain(), source.as_ref().domain());
    assert_eq!(c.vertices().first(), Some(&point(0., 0., 0.)));
    assert_eq!(
        c.vertices().last(),
        Some(&point((n - 1) as f64, ((n - 1) / 7 % 3) as f64, 0.))
    );
}
