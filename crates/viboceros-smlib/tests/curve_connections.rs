#![cfg(feature = "native")]
use viboceros_geometry::{
    Curve3, CurveArcExtensionStyle as A, CurveChamferExtensionStyles as H,
    CurveFilletExtensionStyles as F, CurveOtherExtensionStyle as O, CurveRef, CurveSegment3 as S,
    LineSegment, NurbsCurve, Point3, PolyCurve3, Tolerance, WeightedPoint3,
};

fn p(x: f64, y: f64) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}

fn before(gauge: f64, degree: usize) -> Curve3 {
    let d = degree as f64;
    Curve3::NurbsCurve(
        NurbsCurve::try_new_rational(
            degree,
            (0..=degree)
                .map(|i| {
                    WeightedPoint3::try_new(
                        p(
                            2. * i as f64 / d,
                            (i * (i.saturating_sub(1))) as f64 / (d * (d - 1.)),
                        ),
                        gauge,
                    )
                    .unwrap()
                })
                .collect(),
            std::iter::repeat_n(0., degree + 1)
                .chain(std::iter::repeat_n(1., degree + 1))
                .collect(),
        )
        .unwrap(),
    )
}

fn after(gauge: f64, curved: bool) -> Curve3 {
    if !curved {
        return Curve3::Line(
            LineSegment::try_new(p(3., 3.), p(3., 4.), Tolerance::DEFAULT).unwrap(),
        );
    }
    Curve3::NurbsCurve(
        NurbsCurve::try_new_rational(
            2,
            [p(3., 3.), p(3., 4.), p(3.2, 5.)]
                .into_iter()
                .map(|q| WeightedPoint3::try_new(q, gauge).unwrap())
                .collect(),
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap(),
    )
}

fn same(a: &Curve3, b: &Curve3) {
    let a = a.to_polycurve().unwrap();
    let b = b.to_polycurve().unwrap();
    assert_eq!(a.segments().len(), b.segments().len());
    for (a, b) in a.segments().iter().zip(b.segments()) {
        assert_eq!(std::mem::discriminant(a), std::mem::discriminant(b));
        match (a, b) {
            (S::NurbsCurve(a), S::NurbsCurve(b)) => {
                assert_eq!(a.degree(), b.degree());
                assert_eq!(a.knots().len(), b.knots().len());
                assert_eq!(a.control_points().len(), b.control_points().len());
                for (x, y) in a.knots().iter().zip(b.knots()) {
                    assert!((x - y).abs() < 1e-9);
                }
                for (x, y) in a.control_points().iter().zip(b.control_points()) {
                    assert!(x.point().distance_to(y.point()).unwrap() < 1e-9);
                    assert!((x.weight() / y.weight() - 1.).abs() < 1e-10);
                }
            }
            (S::Arc(a), S::Arc(b)) => {
                assert!((a.radius() - b.radius()).abs() < 1e-10);
            }
            _ => {}
        }
        for i in 0..=32 {
            let fraction = i as f64 / 32.;
            let t = a.parameter_at(fraction).unwrap();
            let u = b.parameter_at(fraction).unwrap();
            assert!(
                a.evaluate(t)
                    .unwrap()
                    .distance_to(b.evaluate(u).unwrap())
                    .unwrap()
                    < 1e-9
            );
        }
    }
}

#[test]
fn native_connect_and_chamfer_preserve_selected_ends_composite_leaves_and_signed_gauges() {
    for curved in [false, true] {
        for gauge in [1., 1e-280, -1e280] {
            for chart in [false, true] {
                for composite in [false, true] {
                    let mut first = before(gauge, 2);
                    let mut second = after(gauge, curved);
                    if chart {
                        first = first.try_reparameterized(-4. ..=7.).unwrap();
                        second = second.try_reparameterized(-8. ..=-2.).unwrap();
                    }
                    if composite {
                        first = Curve3::PolyCurve(
                            PolyCurve3::try_new(vec![
                                S::Line(
                                    LineSegment::try_new(p(-1., 0.), p(0., 0.), Tolerance::DEFAULT)
                                        .unwrap(),
                                ),
                                first.to_polycurve().unwrap().segments()[0].clone(),
                            ])
                            .unwrap(),
                        );
                        let end = second.as_ref().end_point().unwrap();
                        second = Curve3::PolyCurve(
                            PolyCurve3::try_new(vec![
                                second.to_polycurve().unwrap().segments()[0].clone(),
                                S::Line(
                                    LineSegment::try_new(
                                        end,
                                        p(end.x() + 1., end.y()),
                                        Tolerance::DEFAULT,
                                    )
                                    .unwrap(),
                                ),
                            ])
                            .unwrap(),
                        );
                    }
                    let pick_first = first.as_ref().end_point().unwrap();
                    let pick_second = second.as_ref().start_point().unwrap();
                    for reverse_first in [false, true] {
                        for reverse_second in [false, true] {
                            let first = if reverse_first {
                                first.reversed(Tolerance::DEFAULT).unwrap()
                            } else {
                                first.clone()
                            };
                            let second = if reverse_second {
                                second.reversed(Tolerance::DEFAULT).unwrap()
                            } else {
                                second.clone()
                            };
                            let sources = [first.clone(), second.clone()];
                            let a = (&first, pick_first);
                            let b = (&second, pick_second);
                            let actual = viboceros_smlib::connect_curves_parts(
                                a,
                                b,
                                A::Arc,
                                O::Smooth,
                                Tolerance::DEFAULT,
                            )
                            .unwrap();
                            let expected =
                                viboceros_geometry::try_connect_curves_parts_with_styles(
                                    &first,
                                    pick_first,
                                    &second,
                                    pick_second,
                                    A::Arc,
                                    O::Smooth,
                                    Tolerance::DEFAULT,
                                )
                                .unwrap();
                            assert_eq!(actual.len(), 2);
                            for (a, b) in actual.iter().zip(&expected) {
                                same(a, b);
                            }
                            let joined = viboceros_smlib::connect_curves_joined(
                                a,
                                b,
                                A::Arc,
                                O::Smooth,
                                Tolerance::DEFAULT,
                            )
                            .unwrap();
                            let expected =
                                viboceros_geometry::try_connect_curves_joined_with_styles(
                                    &first,
                                    pick_first,
                                    &second,
                                    pick_second,
                                    A::Arc,
                                    O::Smooth,
                                    Tolerance::DEFAULT,
                                )
                                .unwrap();
                            same(
                                &Curve3::PolyCurve(joined.clone()),
                                &Curve3::PolyCurve(expected),
                            );
                            let meeting = if curved {
                                p(3.02533554003123, 2.28816378244402)
                            } else {
                                p(3., 2.25)
                            };
                            let outgoing = joined.segments()[if composite { 1 } else { 0 }]
                                .as_ref()
                                .end_point()
                                .unwrap();
                            assert!(outgoing.distance_to(meeting).unwrap() < 1e-9);
                            if composite {
                                assert_eq!(
                                    joined.segments().first().unwrap(),
                                    &S::Line(
                                        LineSegment::try_new(
                                            p(-1., 0.),
                                            p(0., 0.),
                                            Tolerance::DEFAULT
                                        )
                                        .unwrap()
                                    )
                                );
                                let second_segments = second.to_polycurve().unwrap();
                                let untouched = if reverse_second {
                                    second_segments.segments()[0].reversed().unwrap()
                                } else {
                                    second_segments.segments().last().unwrap().clone()
                                };
                                assert_eq!(joined.segments().last().unwrap(), &untouched);
                            }
                            let styles = H {
                                arc: A::Arc,
                                other: O::Smooth,
                            };
                            let distances = [0.15, 0.3];
                            let bevel = viboceros_smlib::chamfer_curves_parts(
                                a,
                                b,
                                distances,
                                true,
                                styles,
                                Tolerance::DEFAULT,
                            )
                            .unwrap();
                            let expected =
                                viboceros_geometry::try_chamfer_curves_parts_with_styles(
                                    a,
                                    b,
                                    distances,
                                    true,
                                    styles,
                                    Tolerance::DEFAULT,
                                )
                                .unwrap();
                            assert_eq!(bevel.len(), expected.len());
                            for (a, b) in bevel.iter().zip(&expected) {
                                same(a, b);
                            }
                            assert_eq!([first, second], sources);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn native_fillet_preparation_preserves_positive_radius_tangency_and_zero_radius_parts() {
    for gauge in [1., 1e-280, -1e280] {
        let first = before(gauge, 2);
        let second = after(gauge, false);
        let a = (&first, p(2., 1.));
        let b = (&second, p(3., 3.));
        let styles = F {
            arc: A::Arc,
            other: O::Smooth,
        };
        for radius in [0., 0.1] {
            let actual =
                viboceros_smlib::fillet_curves_joined(a, b, radius, styles, Tolerance::DEFAULT)
                    .unwrap();
            let expected = viboceros_geometry::try_fillet_curves_joined_with_styles(
                &first,
                a.1,
                &second,
                b.1,
                radius,
                styles,
                Tolerance::DEFAULT,
            )
            .unwrap();
            same(
                &Curve3::PolyCurve(actual.clone()),
                &Curve3::PolyCurve(expected),
            );
            if radius > 0. {
                let [before, S::Arc(arc), after] = actual.segments() else {
                    panic!("fillet must retain curve, native arc and line")
                };
                assert!((arc.radius() - radius).abs() < 1e-12);
                let incoming = before
                    .derivative_at(*before.domain().end())
                    .unwrap()
                    .normalized(Tolerance::DEFAULT)
                    .unwrap()
                    .as_vector();
                let arc_start = CurveRef::Arc(arc)
                    .evaluate_with_derivative(*arc.domain().start())
                    .unwrap()
                    .1
                    .normalized(Tolerance::DEFAULT)
                    .unwrap()
                    .as_vector();
                let outgoing = after
                    .derivative_at(*after.domain().start())
                    .unwrap()
                    .normalized(Tolerance::DEFAULT)
                    .unwrap()
                    .as_vector();
                let arc_end = CurveRef::Arc(arc)
                    .evaluate_with_derivative(*arc.domain().end())
                    .unwrap()
                    .1
                    .normalized(Tolerance::DEFAULT)
                    .unwrap()
                    .as_vector();
                assert!(incoming.cross(arc_start).unwrap().length().unwrap() < 1e-8);
                assert!(outgoing.cross(arc_end).unwrap().length().unwrap() < 1e-8);
            }
            let actual = viboceros_smlib::fillet_curves_parts(
                a,
                b,
                radius,
                true,
                styles,
                Tolerance::DEFAULT,
            )
            .unwrap();
            let expected = viboceros_geometry::try_fillet_curves_parts_with_styles(
                a,
                b,
                radius,
                true,
                styles,
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert_eq!(actual.len(), expected.len());
            for (a, b) in actual.iter().zip(&expected) {
                same(a, b);
            }
        }
    }
}

#[test]
fn native_connection_keeps_degree_eighteen_extrapolation_and_extraction_fallback() {
    let first = before(1., 18);
    let second = after(1., false);
    let a = (&first, p(2., 1.));
    let b = (&second, p(3., 3.));
    let actual =
        viboceros_smlib::connect_curves_parts(a, b, A::Arc, O::Smooth, Tolerance::DEFAULT).unwrap();
    let expected = viboceros_geometry::try_connect_curves_parts_with_styles(
        &first,
        a.1,
        &second,
        b.1,
        A::Arc,
        O::Smooth,
        Tolerance::DEFAULT,
    )
    .unwrap();
    for (a, b) in actual.iter().zip(&expected) {
        same(a, b);
    }
}
