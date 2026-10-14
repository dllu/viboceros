#![cfg(feature = "native")]
use viboceros_geometry::{
    CircularArc3, CurveRef, CurveSegment3 as S, LineSegment, NurbsCurve, Point3, PolyCurve3,
    Tolerance, WeightedPoint3,
};

fn p(x: f64, y: f64) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
fn nurbs(points: Vec<Point3>, gauge: f64, knots: Vec<f64>) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        points
            .into_iter()
            .map(|p| WeightedPoint3::try_new(p, gauge).unwrap())
            .collect(),
        knots,
    )
    .unwrap()
}
fn curve(points: Vec<Point3>, gauge: f64) -> NurbsCurve {
    nurbs(points, gauge, vec![0., 0., 0., 1., 1., 1.])
}
fn line(a: Point3, b: Point3) -> S {
    S::Line(LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap())
}
fn pair(kind: usize, gauge: f64, chart: bool, scale: f64) -> PolyCurve3 {
    let q = |x, y| p(x * scale, y * scale);
    let mut first = S::NurbsCurve(curve(vec![q(0., 0.), q(2., 0.), q(2., 2.)], gauge));
    let mut second = match kind {
        0 => line(q(2., 2.), q(6., 2.)),
        1 => S::Arc(
            CircularArc3::try_from_three_points(
                q(2., 2.),
                q(2. + 2_f64.sqrt(), 4. - 2_f64.sqrt()),
                q(4., 4.),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
        _ => S::NurbsCurve(curve(vec![q(2., 2.), q(4., 2.), q(4., 4.)], gauge)),
    };
    if chart {
        first = first.try_reparameterized(-4. ..=7.).unwrap();
        second = second.try_reparameterized(10. ..=16.).unwrap();
    }
    PolyCurve3::try_new(vec![first, second]).unwrap()
}
fn check(source: &PolyCurve3, radius: f64) -> PolyCurve3 {
    let original = source.clone();
    let actual = viboceros_smlib::fillet_corners(source, radius, Tolerance::DEFAULT)
        .unwrap_or_else(|error| panic!("{error:?}; radius {radius}; source {source:?}"));
    let expected = source
        .try_fillet_corners(radius, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(actual.is_closed().unwrap(), expected.is_closed().unwrap());
    assert_eq!(actual.segments().len(), expected.segments().len());
    for (a, b) in actual.segments().iter().zip(expected.segments()) {
        assert_eq!(std::mem::discriminant(a), std::mem::discriminant(b));
        if let (S::NurbsCurve(a), S::NurbsCurve(b)) = (a, b) {
            assert_eq!(a.degree(), b.degree());
            assert_eq!(a.knots().len(), b.knots().len());
            for (a, b) in a.knots().iter().zip(b.knots()) {
                // Later corners see a previously extracted native leaf;
                // contact parameters can consequently differ by an ULP.
                assert!((a - b).abs() <= 16. * f64::EPSILON * a.abs().max(b.abs()).max(1.));
            }
            assert_eq!(a.control_points().len(), b.control_points().len());
            for (a, b) in a.control_points().iter().zip(b.control_points()) {
                assert!(a.point().distance_to(b.point()).unwrap() < 1e-9);
                assert!((a.weight() / b.weight() - 1.).abs() < 1e-10);
            }
        }
        for i in 0..=32 {
            let t = a.parameter_at(i as f64 / 32.).unwrap();
            let u = b.parameter_at(i as f64 / 32.).unwrap();
            assert!(
                a.evaluate(t)
                    .unwrap()
                    .distance_to(b.evaluate(u).unwrap())
                    .unwrap()
                    < 1e-9
            );
        }
        if let S::Arc(arc) = a {
            // Original arcs may also be retained; the independently checked
            // joining tangents below cover both original and fillet arcs.
            if !source
                .segments()
                .iter()
                .any(|s| matches!(s,S::Arc(old) if (old.radius()-arc.radius()).abs()<1e-12))
            {
                assert!((arc.radius() - radius).abs() < 1e-10);
            }
        }
    }
    for i in 0..actual.segments().len() - usize::from(!actual.is_closed().unwrap()) {
        let a = &actual.segments()[i];
        let b = &actual.segments()[(i + 1) % actual.segments().len()];
        let a = a
            .as_ref()
            .evaluate_with_tangent(*a.domain().end())
            .unwrap()
            .tangent()
            .as_vector();
        let b = b
            .as_ref()
            .evaluate_with_tangent(*b.domain().start())
            .unwrap()
            .tangent()
            .as_vector();
        assert!(a.cross(b).unwrap().length().unwrap() < 1e-8);
        assert!(a.dot(b).unwrap() > 0.);
    }
    assert_eq!(source, &original);
    actual
}

#[test]
fn native_contact_intervals_keep_charts_gauges_and_tangent_analytic_leaves() {
    for kind in 0..3 {
        for gauge in [1., 1e-280, -1e280] {
            for chart in [false, true] {
                for reverse in [false, true] {
                    let source = pair(kind, gauge, chart, 1.);
                    let source = if reverse {
                        source.reversed().unwrap()
                    } else {
                        source
                    };
                    let result = check(&source, 0.5);
                    assert_eq!(result.segments().len(), 3);
                    assert!(matches!(result.segments()[1], S::Arc(_)));
                }
            }
        }
    }
    // The direct native filleting executive rejects these small corners.
    // Exact native extraction with shared contacts must retain support.
    for kind in 0..3 {
        check(&pair(kind, 1., true, 1e-6), 0.5e-6);
    }
}

#[test]
fn native_contacts_preserve_unselected_leaves_and_fillet_a_closed_seam() {
    for gauge in [1., 1e-280, -1e280] {
        for kind in [1, 2] {
            for reverse in [false, true] {
                let source = pair(kind, gauge, true, 1.);
                let mut segments = vec![line(p(-1., 0.), p(0., 0.))];
                segments.extend_from_slice(source.segments());
                segments.push(line(p(4., 4.), p(4., 5.)));
                let source = PolyCurve3::try_new(segments).unwrap();
                let source = if reverse {
                    source.reversed().unwrap()
                } else {
                    source
                };
                let result = check(&source, 0.5);
                assert_eq!(result.segments().first(), source.segments().first());
                assert_eq!(result.segments().last(), source.segments().last());
            }
        }
        let pair = pair(2, gauge, false, 1.);
        let closed = PolyCurve3::try_new(vec![
            pair.segments()[1].clone(),
            line(p(4., 4.), p(0., 4.)),
            line(p(0., 4.), p(0., 0.)),
            pair.segments()[0].clone(),
        ])
        .unwrap();
        let rounded = check(&closed, 0.5);
        assert!(rounded.is_closed().unwrap());
        assert_eq!(rounded.segments().len(), 8);
        assert!(matches!(rounded.segments()[0], S::Arc(_)));
    }
}

#[test]
fn native_sharp_knot_preparation_matches_separate_leaves_and_degree_eighteen_fallback() {
    for gauge in [1., 1e-280, -1e280] {
        for reverse in [false, true] {
            let leaf = nurbs(
                vec![p(0., 0.), p(2., 0.), p(2., 2.), p(4., 2.), p(4., 4.)],
                gauge,
                vec![0., 0., 0., 1., 1., 2., 2., 2.],
            );
            let source = PolyCurve3::try_new(vec![S::NurbsCurve(leaf)]).unwrap();
            let source = if reverse {
                source.reversed().unwrap()
            } else {
                source
            };
            let result = check(&source, 0.5);
            assert_eq!(result.segments().len(), 3);
            let separate = pair(2, gauge, false, 1.);
            let separate = if reverse {
                separate.reversed().unwrap()
            } else {
                separate
            };
            let expected =
                viboceros_smlib::fillet_corners(&separate, 0.5, Tolerance::DEFAULT).unwrap();
            for i in 0..=32 {
                let t = i as f64 / 32.;
                let a = CurveRef::PolyCurve(&result)
                    .evaluate(result.parameter_at(t).unwrap())
                    .unwrap();
                let b = CurveRef::PolyCurve(&expected)
                    .evaluate(expected.parameter_at(t).unwrap())
                    .unwrap();
                assert!(a.distance_to(b).unwrap() < 1e-9);
            }
        }
    }
    let source = pair(0, 1., false, 1.);
    let S::NurbsCurve(first) = &source.segments()[0] else {
        unreachable!()
    };
    let source = PolyCurve3::try_new(vec![
        S::NurbsCurve(first.try_change_degree(18, false).unwrap()),
        source.segments()[1].clone(),
    ])
    .unwrap();
    check(&source, 0.5);
}
