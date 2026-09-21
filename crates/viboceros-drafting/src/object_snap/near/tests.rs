use super::*;
use crate::{ObjectSnapKind, ObjectSnapModes};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{
    Brep, Circle3, CurveSegment3, Ellipse3, LineSegment, NurbsCurve, NurbsSurface, PolyCurve3,
    Polyline3, WeightedPoint3,
};

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn unit(x: Real, y: Real, z: Real) -> UnitVector3 {
    Vector3::try_new(x, y, z)
        .unwrap()
        .normalized_nonzero()
        .unwrap()
}
fn segment(a: Point3, b: Point3) -> LineSegment {
    LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap()
}
fn modes() -> ObjectSnapModes {
    ObjectSnapModes::only(ObjectSnapKind::Near)
}
fn query(
    doc: &Document,
    cursor: [Real; 2],
    radius: Real,
    modes: ObjectSnapModes,
) -> Option<super::super::ObjectSnap> {
    ObjectSnapCache::default()
        .nearest_projected_with_modes(doc, cursor, radius, |p| Some([p.x(), p.y()]), modes)
        .unwrap()
}
fn close(a: Point3, b: Point3) {
    assert!(a.distance_to(b).unwrap() < 1e-10, "{a:?} != {b:?}");
}

#[test]
fn perspective_line_recovers_world_fraction_not_screen_fraction() {
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Line(segment(p(0., 0., 1.), p(2., 0., 2.))))
        .unwrap();
    let hit = ObjectSnapCache::default()
        .nearest_projected_with_modes(
            &doc,
            [0.5, 0.1],
            0.2,
            |p| (p.z() > 0.).then_some([p.x() / p.z(), p.y() / p.z()]),
            modes(),
        )
        .unwrap()
        .unwrap();
    close(hit.point(), p(2. / 3., 0., 4. / 3.));
    assert_eq!(hit.object_id(), id);
    assert_eq!(hit.kind(), ObjectSnapKind::Near);
    assert!((hit.distance() - 0.1).abs() < 1e-14);
}

#[test]
fn perspective_line_depth_ratios_do_not_require_resolving_one_minus_a_tiny_fraction() {
    for depth in [2., 1e6, 1e12, 1e100] {
        for reverse in [false, true] {
            let a = p(0., 0., 1.);
            let b = p(1., 0., depth);
            let mut doc = Document::default();
            doc.add_geometry(Geometry::Line(if reverse {
                segment(b, a)
            } else {
                segment(a, b)
            }))
            .unwrap();
            let snap = ObjectSnapCache::default()
                .nearest_projected_with_modes(
                    &doc,
                    [0.5, 0.1],
                    0.2,
                    |p| Some([p.x() * depth / p.z(), p.y() / p.z()]),
                    modes(),
                )
                .unwrap()
                .unwrap();
            let t = 1. / (depth + 1.);
            assert!(
                snap.point()
                    .distance_to(p(t, 0., 1. + (depth - 1.) * t))
                    .unwrap()
                    < 2e-12,
                "depth {depth}, reverse {reverse}: {:?}",
                snap.point()
            );
        }
    }
}

#[test]
fn near_finds_a_visible_sliver_of_a_camera_crossing_line_in_both_orders() {
    for depth in [1e3, 1e12, 1e100] {
        for reverse in [false, true] {
            let a = p(0., 0., 1.);
            let b = p(1., 0., -depth);
            let (a, b) = if reverse { (b, a) } else { (a, b) };
            let project =
                |p: Point3| (p.z() >= 0.1).then_some([p.x() * depth / p.z(), p.y() / p.z()]);
            for geometry in [
                Geometry::Line(segment(a, b)),
                Geometry::Polyline(Polyline3::try_new(vec![a, b], Tolerance::DEFAULT).unwrap()),
                Geometry::NurbsCurve(
                    NurbsCurve::try_new_rational(
                        1,
                        vec![
                            WeightedPoint3::try_new(a, 0.5).unwrap(),
                            WeightedPoint3::try_new(b, 3.).unwrap(),
                        ],
                        vec![0., 0., 1., 1.],
                    )
                    .unwrap(),
                ),
            ] {
                let mut doc = Document::default();
                doc.add_geometry(geometry).unwrap();
                let hit = ObjectSnapCache::default()
                    .nearest_projected_with_modes(&doc, [0.5, 0.1], 0.2, project, modes())
                    .unwrap()
                    .unwrap_or_else(|| {
                        panic!("depth {depth}, reverse {reverse}: missed visible line")
                    });
                // Solve depth*t/(1-(depth+1)*t)=1/2 independently of native search.
                let t = 0.5 / (depth + 0.5 * (depth + 1.));
                close(hit.point(), p(t, 0., 1. - (depth + 1.) * t));
                assert!((project(hit.point()).unwrap()[0] - 0.5).abs() < 1e-12);
                assert!((hit.distance() - 0.1).abs() < 1e-12);
            }
        }
    }
}

#[test]
fn near_retains_small_offsets_from_either_end_of_a_long_visible_line() {
    for far in [1e12, 1e100] {
        for reverse in [false, true] {
            let a = p(-far, 0., 7.);
            let b = p(1., 0., 7.);
            let mut doc = Document::default();
            doc.add_geometry(Geometry::Line(if reverse {
                segment(b, a)
            } else {
                segment(a, b)
            }))
            .unwrap();
            close(
                query(&doc, [0.25, 0.1], 0.2, modes()).unwrap().point(),
                p(0.25, 0., 7.),
            );
        }
    }
}

#[test]
fn near_covers_analytic_curves_composites_surface_boundaries_and_brep_edges() {
    let c = NurbsCurve::try_new(
        2,
        vec![p(0., 0., 7.), p(1., 0., 7.), p(8., 0., 7.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let surface = NurbsSurface::try_new(
        2,
        1,
        3,
        2,
        vec![
            p(0., 0., 7.),
            p(1., 0., 7.),
            p(8., 0., 7.),
            p(0., 4., 7.),
            p(1., 4., 7.),
            p(8., 4., 7.),
        ],
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    for geometry in [
        Geometry::Line(segment(p(0., 0., 7.), p(8., 0., 7.))),
        Geometry::Polyline(
            Polyline3::try_new(
                vec![p(0., 0., 7.), p(8., 0., 7.), p(8., 4., 7.)],
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
        Geometry::NurbsCurve(c.clone()),
        Geometry::PolyCurve(
            PolyCurve3::try_new(vec![
                CurveSegment3::NurbsCurve(c),
                segment(p(8., 0., 7.), p(8., 4., 7.)).into(),
            ])
            .unwrap(),
        ),
        Geometry::NurbsSurface(surface.clone()),
        Geometry::Brep(Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap()),
    ] {
        let mut doc = Document::default();
        let id = doc.add_geometry(geometry).unwrap();
        let hit = query(&doc, [2.37, -0.1], 0.2, modes()).unwrap();
        close(hit.point(), p(2.37, 0., 7.));
        assert_eq!(hit.object_id(), id);
        doc.set_objects_locked([id], true).unwrap();
        assert!(query(&doc, [2.37, -0.1], 0.2, modes()).is_some());
        doc.set_objects_visibility([id], false).unwrap();
        assert!(query(&doc, [2.37, -0.1], 0.2, modes()).is_none());
    }
}

#[test]
fn circle_ellipse_and_rational_quarter_have_accurate_nonzero_distance_targets() {
    let circle = Circle3::try_new(p(0., 0., 7.), 2., unit(0., 0., 1.), Tolerance::DEFAULT).unwrap();
    let ellipse = Ellipse3::try_new(
        p(0., 0., 7.),
        2.,
        1.,
        unit(1., 0., 0.),
        unit(0., 1., 0.),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let quarter = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(p(2., 0., 7.), 1.).unwrap(),
            WeightedPoint3::try_new(p(2., 1., 7.), std::f64::consts::FRAC_1_SQRT_2).unwrap(),
            WeightedPoint3::try_new(p(0., 1., 7.), 1.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    // Ellipse station (.6,.8) in its angular unit circle. Move the cursor
    // along its normal, not its radius, to get an independently known closest point.
    for (geometry, cursor, target) in [
        (
            Geometry::Arc(
                viboceros_geometry::CircularArc3::try_from_three_points(
                    p(2., 0., 7.),
                    p(2_f64.sqrt(), 2_f64.sqrt(), 7.),
                    p(0., 2., 7.),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ),
            [1.8, 2.4],
            p(1.2, 1.6, 7.),
        ),
        (Geometry::Circle(circle), [1.8, 2.4], p(1.2, 1.6, 7.)),
        (Geometry::Ellipse(ellipse), [1.23, 0.88], p(1.2, 0.8, 7.)),
        (Geometry::NurbsCurve(quarter), [1.23, 0.88], p(1.2, 0.8, 7.)),
    ] {
        let mut doc = Document::default();
        doc.add_geometry(geometry).unwrap();
        close(query(&doc, cursor, 1.1, modes()).unwrap().point(), target);
    }
}

#[test]
fn stationary_endpoints_do_not_hide_a_nearby_interior_minimum_at_small_capture_scales() {
    for (controls, x, y) in [
        (
            vec![p(0., 0., 0.), p(0., 0., 0.), p(1., 0., 0.)],
            1e-6,
            1e-7,
        ),
        (
            vec![p(0., 0., 0.), p(1., 0., 0.), p(1., 0., 0.)],
            1. - 1e-6,
            1e-7,
        ),
        (
            vec![p(0., 0., 0.), p(0., 0., 0.), p(1., 0., 0.)],
            1e-20,
            1e-22,
        ),
    ] {
        let mut doc = Document::default();
        doc.add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_new(2, controls, vec![0., 0., 0., 1., 1., 1.]).unwrap(),
        ))
        .unwrap();
        let snap = query(&doc, [x, y], y * 1.1, modes()).unwrap();
        assert!(
            (snap.point().x() - x).abs() < x * 1e-12,
            "{x}: {:?}",
            snap.point()
        );
        assert!((snap.distance() - y).abs() < y * 1e-10);
    }
}

#[test]
fn discrete_landmarks_suppress_near_but_near_suppresses_center_on_same_object() {
    let mut doc = Document::default();
    doc.add_geometry(Geometry::Line(segment(p(0., 0., 0.), p(8., 0., 0.))))
        .unwrap();
    for (cursor, kind, target) in [
        ([0.2, 0.1], ObjectSnapKind::End, p(0., 0., 0.)),
        ([3.8, 0.1], ObjectSnapKind::Mid, p(4., 0., 0.)),
    ] {
        close(
            query(&doc, cursor, 0.3, modes()).unwrap().point(),
            p(cursor[0], 0., 0.),
        );
        let hit = query(&doc, cursor, 0.3, modes().with(kind, true)).unwrap();
        assert_eq!(hit.kind(), kind);
        close(hit.point(), target);
    }
    let mut doc = Document::default();
    doc.add_geometry(Geometry::Circle(
        Circle3::try_new(p(0., 0., 0.), 2., unit(0., 0., 1.), Tolerance::DEFAULT).unwrap(),
    ))
    .unwrap();
    let hit = query(
        &doc,
        [1.2, 1.6],
        0.1,
        modes().with(ObjectSnapKind::Center, true),
    )
    .unwrap();
    assert_eq!(hit.kind(), ObjectSnapKind::Near);
    close(hit.point(), p(1.2, 1.6, 0.));
    let hit = query(
        &doc,
        [1.98, 0.18],
        0.3,
        modes().with(ObjectSnapKind::Quad, true),
    )
    .unwrap();
    assert_eq!(hit.kind(), ObjectSnapKind::Quad);
    close(hit.point(), p(2., 0., 0.));
}

#[test]
fn extreme_knot_domains_and_discontinuous_spans_do_not_change_near_locus() {
    for (a, b) in [
        (0., 1.),
        (1e12, 1e12 + 1.),
        (0., 1e-170),
        (0., 1e170),
        (0., Real::from_bits(1)),
    ] {
        let mut doc = Document::default();
        doc.add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_new(
                2,
                vec![p(0., 0., 0.), p(1., 0., 0.), p(8., 0., 0.)],
                vec![a, a, a, b, b, b],
            )
            .unwrap(),
        ))
        .unwrap();
        close(
            query(&doc, [2.37, 0.1], 0.2, modes()).unwrap().point(),
            p(2.37, 0., 0.),
        );
    }
    let mut doc = Document::default();
    doc.add_geometry(Geometry::NurbsCurve(
        NurbsCurve::try_new(
            1,
            vec![p(-4., 0., 0.), p(-2., 0., 0.), p(2., 0., 0.), p(4., 0., 0.)],
            vec![0., 0., 1., 1., 2., 2.],
        )
        .unwrap(),
    ))
    .unwrap();
    assert!(query(&doc, [0., 0.], 0.2, modes()).is_none());
    close(
        query(&doc, [2.3, 0.1], 0.2, modes()).unwrap().point(),
        p(2.3, 0., 0.),
    );
}

#[test]
fn cached_near_tracks_edits_undo_deletion_and_conversion() {
    let make = |y| {
        Geometry::NurbsCurve(
            NurbsCurve::try_new(
                2,
                vec![p(0., y, 0.), p(1., y, 0.), p(8., y, 0.)],
                vec![0., 0., 0., 1., 1., 1.],
            )
            .unwrap(),
        )
    };
    let mut doc = Document::default();
    let id = doc.add_geometry(make(0.)).unwrap();
    let mut cache = ObjectSnapCache::default();
    let hit = |cache: &mut ObjectSnapCache, doc: &Document, y| {
        cache
            .nearest_projected_with_modes(doc, [2.37, y], 0.2, |p| Some([p.x(), p.y()]), modes())
            .unwrap()
    };
    for _ in 0..3 {
        close(hit(&mut cache, &doc, 0.).unwrap().point(), p(2.37, 0., 0.));
    }
    doc.replace_object_geometries([(id, make(3.))]).unwrap();
    assert!(hit(&mut cache, &doc, 0.).is_none());
    close(hit(&mut cache, &doc, 3.).unwrap().point(), p(2.37, 3., 0.));
    doc.undo().unwrap();
    assert!(hit(&mut cache, &doc, 0.).is_some());
    doc.replace_object_geometries([(id, Geometry::Point(p(2.37, 0., 0.)))])
        .unwrap();
    assert!(hit(&mut cache, &doc, 0.).is_none());
    doc.delete_object(id).unwrap();
    assert!(hit(&mut cache, &doc, 0.).is_none());
}

#[test]
fn clipped_line_and_degenerate_projected_tangent_never_bridge_invisible_parts() {
    let mut doc = Document::default();
    doc.add_geometry(Geometry::Line(segment(p(-2., 0., -1.), p(2., 0., 3.))))
        .unwrap();
    let mut cache = ObjectSnapCache::default();
    let hit = cache
        .nearest_projected_with_modes(
            &doc,
            [0.25, 0.02],
            0.1,
            |p| (p.z() > 0.1).then_some([p.x() / p.z(), p.y() / p.z()]),
            modes(),
        )
        .unwrap()
        .unwrap();
    close(hit.point(), p(1. / 3., 0., 4. / 3.));
    assert!(
        cache
            .nearest_projected_with_modes(&doc, [0., 0.], 0.1, |_| None, modes())
            .unwrap()
            .is_none()
    );
}

#[test]
fn mixed_weight_near_uses_the_real_locus_outside_the_control_hull() {
    let mut doc = Document::default();
    doc.add_geometry(Geometry::NurbsCurve(
        NurbsCurve::try_new_rational(
            1,
            vec![
                WeightedPoint3::try_new(p(0., 0., 0.), 1.).unwrap(),
                WeightedPoint3::try_new(p(1., 0., 0.), -0.5).unwrap(),
            ],
            vec![0., 0., 1., 1.],
        )
        .unwrap(),
    ))
    .unwrap();
    close(
        query(&doc, [2., 0.05], 0.1, modes()).unwrap().point(),
        p(2., 0., 0.),
    );
    // There is no rational-locus point between the two branches [1,+inf)
    // and (-inf,0]; a chord through the control polygon would be a false hit.
    assert!(query(&doc, [0.5, 0.], 0.1, modes()).is_none());
}

#[test]
fn relative_axis_queries_keep_small_offsets_at_large_origins() {
    for projection in [
        viboceros_geometry::PointCloudProjection::Xy,
        viboceros_geometry::PointCloudProjection::Xz,
        viboceros_geometry::PointCloudProjection::Yz,
    ] {
        let map = |x| match projection {
            viboceros_geometry::PointCloudProjection::Xy => p(1e12 + x, -1e12, 7.),
            viboceros_geometry::PointCloudProjection::Xz => p(1e12 + x, 7., -1e12),
            viboceros_geometry::PointCloudProjection::Yz => p(7., 1e12 + x, -1e12),
        };
        let mut doc = Document::default();
        doc.add_geometry(Geometry::Line(segment(map(0.), map(8.))))
            .unwrap();
        let hit = ObjectSnapCache::default()
            .nearest_axis_aligned_with_modes(
                &doc,
                projection,
                map(0.),
                [2.375, 0.01],
                0.05,
                modes(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(hit.point(), map(2.375));
        assert!((hit.distance() - 0.01).abs() < 1e-12);
    }
}

#[test]
#[ignore = "local timing diagnostic, not a frame-time assertion"]
fn near_scene_timing() {
    use std::{hint::black_box, time::Instant};
    for nurbs in [false, true] {
        for count in [1, 100, 1000] {
            let mut doc = Document::default();
            for i in 0..count {
                let y = i as Real * 3.;
                let shape = if nurbs {
                    Geometry::NurbsCurve(
                        NurbsCurve::try_new(
                            2,
                            vec![p(0., y, 0.), p(1., y, 0.), p(8., y, 0.)],
                            vec![0., 0., 0., 1., 1., 1.],
                        )
                        .unwrap(),
                    )
                } else {
                    Geometry::Line(segment(p(0., y, 0.), p(8., y, 0.)))
                };
                doc.add_geometry(shape).unwrap();
            }
            let mut cache = ObjectSnapCache::default();
            let mut query = || {
                cache
                    .nearest_projected_with_modes(
                        &doc,
                        [2.37, 0.1],
                        0.2,
                        |p| Some([p.x(), p.y()]),
                        modes(),
                    )
                    .unwrap()
                    .unwrap()
            };
            for _ in 0..10 {
                black_box(query());
            }
            let start = Instant::now();
            for _ in 0..100 {
                black_box(query());
            }
            eprintln!(
                "near_scene nurbs={nurbs} objects={count} us/query={:.3}",
                start.elapsed().as_secs_f64() * 1e4
            );
        }
    }
}
