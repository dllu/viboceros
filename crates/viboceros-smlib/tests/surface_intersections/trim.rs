use super::*;
use viboceros_geometry::{
    Brep, BrepBrepIntersectionEvent, BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim,
    BrepTrimType, BrepVertex, Circle3, NurbsCurve, NurbsCurve2, Point2,
    SurfaceBrepIntersectionEvent, SurfaceIso, UnitVector3, WeightedPoint2,
};

fn pair() -> [NurbsSurface; 2] {
    [
        NurbsSurface::try_new(
            2,
            3,
            3,
            4,
            (0..4)
                .flat_map(|v| {
                    (0..3).map(move |u| {
                        Point3::try_new(
                            u as f64 / 2.,
                            v as f64 / 3.,
                            f64::from(u == 2) + f64::from(v == 3),
                        )
                        .unwrap()
                    })
                })
                .collect(),
            vec![0., 0., 0., 1., 1., 1.],
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap(),
        patch([[0., 0., 1.], [1., 0., 1.], [0., 1., 1.], [1., 1., 0.]]),
    ]
}

fn rectangular_hole(surface: NurbsSurface, tolerance: Tolerance) -> Brep {
    let outer =
        Brep::try_rectangular_surface_face(surface.clone(), 0.25..=0.75, 0. ..=1., tolerance)
            .unwrap();
    let inner =
        Brep::try_rectangular_surface_face(surface.clone(), 0.4..=0.6, 0.55..=0.85, tolerance)
            .unwrap();
    let vo = outer.vertices().len();
    let eo = outer.edges().len();
    let vertices = outer
        .vertices()
        .iter()
        .chain(inner.vertices())
        .copied()
        .collect();
    let mut edges = outer.edges().to_vec();
    edges.extend(inner.edges().iter().map(|edge| {
        BrepEdge::try_new(
            edge.vertices().map(|v| v + vo),
            edge.curve().clone(),
            edge.tolerance(),
        )
        .unwrap()
    }));
    let trims = inner.faces()[0].loops()[0]
        .trims()
        .iter()
        .rev()
        .map(|trim| {
            BrepTrim::try_new(
                [trim.vertices()[1] + vo, trim.vertices()[0] + vo],
                trim.edge().map(|e| e + eo),
                !trim.is_reversed_3d(),
                trim.curve().reversed().unwrap(),
                trim.trim_type(),
                trim.iso(),
                trim.tolerance(),
            )
            .unwrap()
        })
        .collect();
    let face = BrepFace::try_new(
        surface,
        false,
        vec![
            outer.faces()[0].loops()[0].clone(),
            BrepLoop::try_new(BrepLoopType::Inner, trims).unwrap(),
        ],
    )
    .unwrap();
    Brep::try_new(vertices, edges, vec![face], tolerance).unwrap()
}

fn circular_hole(
    surface: NurbsSurface,
    center: [f64; 2],
    radius: f64,
    tolerance: Tolerance,
) -> Brep {
    let outer =
        Brep::try_rectangular_surface_face(surface.clone(), 0.25..=0.75, 0. ..=1., tolerance)
            .unwrap();
    let circle = Circle3::try_new(
        Point3::try_new(center[0], center[1], 0.).unwrap(),
        radius,
        UnitVector3::try_new(0., 0., -1., tolerance).unwrap(),
        tolerance,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let uv = NurbsCurve2::try_new_rational(
        circle.degree(),
        circle
            .control_points()
            .iter()
            .map(|p| {
                WeightedPoint2::try_new(
                    Point2::try_new(p.point().x(), p.point().y()).unwrap(),
                    p.weight(),
                )
                .unwrap()
            })
            .collect(),
        circle.knots().to_vec(),
    )
    .unwrap();
    let curve = surface.try_pushup_curve_certified(&uv, tolerance).unwrap();
    let v = outer.vertices().len();
    let e = outer.edges().len();
    let mut vertices = outer.vertices().to_vec();
    vertices
        .push(BrepVertex::try_new(curve.evaluate(*curve.domain().start()).unwrap(), 0.).unwrap());
    let mut edges = outer.edges().to_vec();
    edges.push(BrepEdge::try_new([v, v], curve, 0.).unwrap());
    let trim = BrepTrim::try_new(
        [v, v],
        Some(e),
        false,
        uv,
        BrepTrimType::Boundary,
        SurfaceIso::NotIso,
        [0., 0.],
    )
    .unwrap();
    let face = BrepFace::try_new(
        surface,
        false,
        vec![
            outer.faces()[0].loops()[0].clone(),
            BrepLoop::try_new(BrepLoopType::Inner, vec![trim]).unwrap(),
        ],
    )
    .unwrap();
    Brep::try_new(vertices, edges, vec![face], tolerance).unwrap()
}

fn points(curve: &NurbsCurve) -> impl Iterator<Item = Point3> + '_ {
    (0..=64).map(|i| {
        curve
            .evaluate(curve.parameter_at(i as f64 / 64.).unwrap())
            .unwrap()
    })
}

fn y_at(x: f64) -> f64 {
    let (mut lo, mut hi) = (0., 1.);
    for _ in 0..64 {
        let y = (lo + hi) * 0.5;
        if x * x + y * y * y + x * y < 1. {
            lo = y;
        } else {
            hi = y;
        }
    }
    (lo + hi) * 0.5
}

#[test]
fn general_uv_contacts_clip_rectangular_holes_and_both_original_faces() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let [first, second] = pair();
    let hole = rectangular_hole(second, tolerance);
    let source =
        Brep::try_rectangular_surface_face(first.clone(), 0.1..=0.8, 0.55..=0.95, tolerance)
            .unwrap();
    let before = (source.clone(), hole.clone());
    let events =
        viboceros_smlib::surface_brep_intersection_events(&first, &hole, tolerance).unwrap();
    assert_eq!(events.len(), 2, "{events:?}");
    let mut ranges = Vec::new();
    for event in events {
        let SurfaceBrepIntersectionEvent::Curve(curve) = event else {
            panic!("expected curve");
        };
        let mut x = Vec::new();
        for p in points(&curve) {
            assert!((p.x() * p.x() + p.y().powi(3) + p.x() * p.y() - 1.).abs() < 2e-6);
            assert!(p.x() <= 0.4 + 1e-6 || p.x() >= 0.6 - 1e-6);
            x.push(p.x());
        }
        ranges.push([
            x.iter().copied().fold(f64::INFINITY, f64::min),
            x.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        ]);
    }
    ranges.sort_by(|a, b| a[0].total_cmp(&b[0]));
    for (got, expected) in ranges.iter().zip([[0.25, 0.4], [0.6, 0.75]]) {
        assert!(
            (got[0] - expected[0]).abs() < 1e-6 && (got[1] - expected[1]).abs() < 1e-6,
            "{ranges:?}"
        );
    }
    let events = viboceros_smlib::brep_intersection_events(&source, &hole, tolerance).unwrap();
    assert_eq!(events.len(), 2, "{events:?}");
    for event in events {
        let BrepBrepIntersectionEvent::Curve(curve) = event else {
            panic!("expected curve");
        };
        for p in points(&curve) {
            assert!(
                p.x() >= 0.25 - 1e-6
                    && p.x() <= 0.75 + 1e-6
                    && p.y() >= 0.55 - 1e-6
                    && p.y() <= 0.95 + 1e-6
            );
            assert!(p.x() <= 0.4 + 1e-6 || p.x() >= 0.6 - 1e-6);
        }
    }
    assert_eq!((source, hole), before);
}

#[test]
fn general_uv_contacts_clip_a_rational_circular_hole() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let [first, second] = pair();
    let center = [0.5, y_at(0.5)];
    let radius = 0.09;
    let hole = circular_hole(second, center, radius, tolerance);
    let before = hole.clone();
    let events =
        viboceros_smlib::surface_brep_intersection_events(&first, &hole, tolerance).unwrap();
    assert_eq!(events.len(), 2, "{events:?}");
    let mut boundary_contacts = 0;
    for event in events {
        let SurfaceBrepIntersectionEvent::Curve(curve) = event else {
            panic!("expected curve");
        };
        for p in points(&curve) {
            let distance = (p.x() - center[0]).hypot(p.y() - center[1]);
            assert!(distance >= radius - 2e-6, "{p:?}");
            assert!((p.x() * p.x() + p.y().powi(3) + p.x() * p.y() - 1.).abs() < 2e-6);
        }
        for t in [*curve.domain().start(), *curve.domain().end()] {
            let p = curve.evaluate(t).unwrap();
            if ((p.x() - center[0]).hypot(p.y() - center[1]) - radius).abs() < 2e-6 {
                boundary_contacts += 1;
            }
        }
    }
    assert_eq!(boundary_contacts, 2);
    assert_eq!(hole, before);
}

#[test]
fn uv_trim_clipping_preserves_extreme_source_charts_and_signed_gauges() {
    use viboceros_geometry::WeightedPoint3;
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    for exponent in [-360, 0, 360] {
        for gauge in [1., -2_f64.powi(200)] {
            let [first, second] = pair();
            let width = 2_f64.powi(exponent);
            let first = first.try_reparameterized(0. ..=width, 0. ..=width).unwrap();
            let controls = second
                .control_points()
                .iter()
                .map(|p| WeightedPoint3::try_new(p.point(), p.weight() * gauge).unwrap())
                .collect();
            let second = NurbsSurface::try_new_rational(
                second.degree_u(),
                second.degree_v(),
                second.control_point_count_u(),
                second.control_point_count_v(),
                controls,
                second.knots_u().to_vec(),
                second.knots_v().to_vec(),
            )
            .unwrap()
            .try_reparameterized(0. ..=width, 0. ..=width)
            .unwrap();
            let face = Brep::try_rectangular_surface_face(
                second,
                0.25 * width..=0.75 * width,
                0. ..=width,
                tolerance,
            )
            .unwrap();
            let before = face.clone();
            let events =
                viboceros_smlib::surface_brep_intersection_events(&first, &face, tolerance)
                    .unwrap();
            assert_eq!(events.len(), 1, "exponent={exponent}, gauge={gauge}");
            let SurfaceBrepIntersectionEvent::Curve(curve) = &events[0] else {
                panic!("expected curve");
            };
            for p in points(curve) {
                assert!(p.x() >= 0.25 - 1e-6 && p.x() <= 0.75 + 1e-6);
            }
            let a = curve.evaluate(*curve.domain().start()).unwrap();
            let b = curve.evaluate(*curve.domain().end()).unwrap();
            assert!(
                (a.x().min(b.x()) - 0.25).abs() < 1e-6 && (a.x().max(b.x()) - 0.75).abs() < 1e-6
            );
            assert_eq!(face, before);
        }
    }
}

#[test]
fn fitted_general_intersection_retains_a_trim_corner_contact() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let [first, second] = pair();
    let y = y_at(0.5);
    let face = Brep::try_rectangular_surface_face(second, 0. ..=0.5, 0. ..=y, tolerance).unwrap();
    let events =
        viboceros_smlib::surface_brep_intersection_events(&first, &face, tolerance).unwrap();
    let [SurfaceBrepIntersectionEvent::Point(point)] = events.as_slice() else {
        panic!("{events:?}");
    };
    let expected = Point3::try_new(0.5, y, 1. - 0.5 * y).unwrap();
    assert!(point.distance_to(expected).unwrap() < 1e-6);
}

#[test]
fn fitted_general_intersections_keep_tangent_holes_from_fragmenting_the_curve() {
    for absolute in [1e-6, 1e-9] {
        let tolerance = Tolerance::try_new(absolute, 1e-12, 1e-10).unwrap();
        for x in [0.45, 0.5, 0.6] {
            for side in [-1., 1.] {
                let [first, second] = pair();
                let y = y_at(x);
                let radius = 0.06;
                let gradient = [2. * x + y, 3. * y * y + x];
                let length = gradient[0].hypot(gradient[1]);
                let center = [
                    x + side * radius * gradient[0] / length,
                    y + side * radius * gradient[1] / length,
                ];
                let hole = circular_hole(second, center, radius, tolerance);
                let before = hole.clone();
                let events =
                    viboceros_smlib::surface_brep_intersection_events(&first, &hole, tolerance)
                        .unwrap();
                assert_eq!(
                    events.len(),
                    1,
                    "absolute={absolute} x={x} side={side}, events={}",
                    events.len()
                );
                let SurfaceBrepIntersectionEvent::Curve(curve) = &events[0] else {
                    panic!("expected curve");
                };
                let ends = [
                    curve.evaluate(*curve.domain().start()).unwrap().x(),
                    curve.evaluate(*curve.domain().end()).unwrap().x(),
                ];
                assert!((ends[0].min(ends[1]) - 0.25).abs() < absolute * 2.);
                assert!((ends[0].max(ends[1]) - 0.75).abs() < absolute * 2.);
                for p in points(curve) {
                    assert!(
                        (p.x() * p.x() + p.y().powi(3) + p.x() * p.y() - 1.).abs() < absolute * 2.
                    );
                    assert!((p.x() - center[0]).hypot(p.y() - center[1]) >= radius - absolute * 2.);
                }
                assert_eq!(hole, before);
            }
        }
    }
}

#[test]
fn genuine_crossings_and_clearance_above_tolerance_are_preserved() {
    for absolute in [1e-6, 1e-9] {
        let tolerance = Tolerance::try_new(absolute, 1e-12, 1e-10).unwrap();
        let x = 0.5;
        let y = y_at(x);
        let radius = 0.06;
        let gradient = [2. * x + y, 3. * y * y + x];
        let length = gradient[0].hypot(gradient[1]);
        for (delta, count) in [(-64. * absolute, 2), (64. * absolute, 1)] {
            let [first, second] = pair();
            let center = [
                x + (radius + delta) * gradient[0] / length,
                y + (radius + delta) * gradient[1] / length,
            ];
            let hole = circular_hole(second, center, radius, tolerance);
            let events =
                viboceros_smlib::surface_brep_intersection_events(&first, &hole, tolerance)
                    .unwrap();
            assert_eq!(events.len(), count, "absolute={absolute}, delta={delta}");
            for event in events {
                let SurfaceBrepIntersectionEvent::Curve(curve) = event else {
                    panic!("expected curve");
                };
                for p in points(&curve) {
                    assert!((p.x() - center[0]).hypot(p.y() - center[1]) >= radius - absolute * 2.);
                }
            }
        }
    }
}
