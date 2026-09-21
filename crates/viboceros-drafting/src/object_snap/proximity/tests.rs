use super::*;

#[test]
fn analytic_and_common_sign_nurbs_hover_use_the_clipped_line_locus() {
    use super::super::ProjectedSnapMetric;
    use viboceros_geometry::{Tolerance, WeightedPoint3};
    let a = Point3::try_new(0., 0., 1.).unwrap();
    let b = Point3::try_new(1., 0., -1e12).unwrap();
    let metric = ProjectedSnapMetric {
        cursor: [0.5, 0.1],
        capture_radius: 0.2,
        project: |p: Point3| (p.z() >= 0.1).then_some([p.x() * 1e12 / p.z(), p.y() / p.z()]),
    };
    for (a, b) in [(a, b), (b, a)] {
        let line = LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap();
        assert!((line_distance(line, &metric).unwrap() - 0.1).abs() < 1e-12);
        for weights in [[1., 1.], [0.5, 3.], [-3., -0.5]] {
            let curve = NurbsCurve::try_new_rational(
                1,
                vec![
                    WeightedPoint3::try_new(a, weights[0]).unwrap(),
                    WeightedPoint3::try_new(b, weights[1]).unwrap(),
                ],
                vec![0., 0., 1., 1.],
            )
            .unwrap();
            assert!((nurbs_distance(&curve, true, &metric).unwrap() - 0.1).abs() < 1e-12);
        }
    }
}

#[test]
fn sided_span_proximity_does_not_bridge_discontinuous_curve_branches() {
    use viboceros_geometry::NurbsCurve;
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let curve = NurbsCurve::try_new(
        1,
        vec![p(-4., -1.), p(-4., 1.), p(4., -1.), p(4., 1.)],
        vec![0., 0., 0.5, 0.5, 1., 1.],
    )
    .unwrap();
    let sampler = curve.parameter_sampler().unwrap();
    let distances: Vec<_> = sampler
        .spans()
        .map(|span| {
            projected_distance(|t| {
                let point = span.evaluate(t).ok()?;
                Some(point.x().hypot(point.y()))
            })
            .unwrap()
        })
        .collect();
    assert_eq!(distances.len(), 2);
    for distance in distances {
        assert!((distance - 4.).abs() < 1e-12);
    }
}
