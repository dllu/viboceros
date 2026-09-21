use super::*;
use crate::{ObjectSnapCache, ObjectSnapKind, ObjectSnapModes};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{
    Circle3, CircularArc3, CurveSegment3, Ellipse3, LineSegment, PointCloudProjection, PolyCurve3,
    Tolerance, Vector3,
};

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn circle() -> Circle3 {
    Circle3::try_new(
        p(4., -4., 0.),
        2.,
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn arc() -> CircularArc3 {
    CircularArc3::try_from_three_points(
        p(2., -4., 0.),
        p(4., -2., 0.),
        p(6., -4., 0.),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn query(doc: &Document, target: Point3, modes: ObjectSnapModes) -> Option<crate::ObjectSnap> {
    ObjectSnapCache::default()
        .nearest_projected_with_modes(
            doc,
            [target.x(), target.y()],
            0.1,
            |point| Some([point.x(), point.y()]),
            modes,
        )
        .unwrap()
}

#[test]
fn center_is_captured_by_arc_circle_ellipse_and_polycurve_hover_not_empty_center() {
    let ellipse = Ellipse3::try_new(
        p(4., -4., 0.),
        3.,
        2.,
        Vector3::try_new(1., 0., 0.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Vector3::try_new(0., 1., 0.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let polycurve = PolyCurve3::try_new(vec![
        CurveSegment3::Arc(arc()),
        LineSegment::try_new(p(6., -4., 0.), p(9., -4., 0.), Tolerance::DEFAULT)
            .unwrap()
            .into(),
    ])
    .unwrap();
    for (geometry, aim) in [
        (Geometry::Circle(circle()), p(2.4, -2.8, 0.)),
        (Geometry::Arc(arc()), p(2.4, -2.8, 0.)),
        (Geometry::Ellipse(ellipse), p(1.6, -2.8, 0.)),
        (Geometry::PolyCurve(polycurve), p(2.4, -2.8, 0.)),
    ] {
        let mut doc = Document::default();
        let id = doc.add_geometry(geometry).unwrap();
        for modes in [
            ObjectSnapModes::LANDMARKS,
            ObjectSnapModes::only(ObjectSnapKind::Center),
        ] {
            let snap = query(&doc, aim, modes).unwrap();
            assert_eq!(snap.kind(), ObjectSnapKind::Center);
            assert_eq!(snap.object_id(), id);
            assert!(snap.point().distance_to(p(4., -4., 0.)).unwrap() < 1e-12);
            assert!(snap.distance() < 1e-8);
            assert!(query(&doc, p(4., -4., 0.), modes).is_none());
        }
        doc.set_objects_locked([id], true).unwrap();
        assert!(query(&doc, aim, ObjectSnapModes::LANDMARKS).is_some());
        doc.set_objects_visibility([id], false).unwrap();
        assert!(query(&doc, aim, ObjectSnapModes::LANDMARKS).is_none());
    }
}

#[test]
fn direct_features_win_over_center_hover_and_modes_exclude_them() {
    for (geometry, aim, kind) in [
        (
            Geometry::Arc(arc()),
            p(2.01, -3.99, 0.),
            ObjectSnapKind::End,
        ),
        (
            Geometry::Arc(arc()),
            p(4.01, -1.99, 0.),
            ObjectSnapKind::Mid,
        ),
        (
            Geometry::Circle(circle()),
            p(6.01, -3.99, 0.),
            ObjectSnapKind::Quad,
        ),
    ] {
        let mut doc = Document::default();
        doc.add_geometry(geometry).unwrap();
        assert_eq!(
            query(&doc, aim, ObjectSnapModes::LANDMARKS).unwrap().kind(),
            kind
        );
        assert_eq!(
            query(&doc, aim, ObjectSnapModes::only(ObjectSnapKind::Center))
                .unwrap()
                .kind(),
            ObjectSnapKind::Center
        );
        assert_eq!(query(&doc, aim, ObjectSnapModes::NONE), None);
        assert_eq!(
            query(
                &doc,
                aim,
                ObjectSnapModes::LANDMARKS.with(ObjectSnapKind::Center, false)
            )
            .unwrap()
            .kind(),
            kind
        );
    }
}

#[test]
fn center_competes_by_capture_distance_with_features_on_other_objects() {
    for reverse in [false, true] {
        for target in [p(2.6, -2.8, 0.), p(3., -2.8, 0.)] {
            let mut doc = Document::default();
            let mut geometries = vec![Geometry::Circle(circle()), Geometry::Point(target)];
            if reverse {
                geometries.reverse();
            }
            for geometry in geometries {
                doc.add_geometry(geometry).unwrap();
            }
            let mut cache = ObjectSnapCache::default();
            let hit = cache
                .nearest_projected(&doc, [2.4, -2.8], 0.7, |p| Some([p.x(), p.y()]))
                .unwrap()
                .unwrap();
            assert_eq!(hit.kind(), ObjectSnapKind::Center);
            assert_eq!(hit.point(), p(4., -4., 0.));
            let hit = cache
                .nearest_projected(&doc, [target.x(), target.y()], 0.7, |p| {
                    Some([p.x(), p.y()])
                })
                .unwrap()
                .unwrap();
            assert_eq!(hit.kind(), ObjectSnapKind::Point);
            assert_eq!(hit.point(), target);
        }
    }
}

#[test]
fn arc_capture_respects_sweep_not_its_supporting_circle() {
    let mut doc = Document::default();
    doc.add_geometry(Geometry::Arc(arc())).unwrap();
    assert!(
        query(
            &doc,
            p(4., -6., 0.),
            ObjectSnapModes::only(ObjectSnapKind::Center)
        )
        .is_none()
    );
    assert!(
        query(
            &doc,
            p(2.4, -2.8, 0.),
            ObjectSnapModes::only(ObjectSnapKind::Center)
        )
        .is_some()
    );
}

#[test]
fn center_modes_are_valid_bit_sets_and_disabled_queries_do_not_project() {
    let mut modes = ObjectSnapModes::NONE;
    for kind in [
        ObjectSnapKind::Point,
        ObjectSnapKind::End,
        ObjectSnapKind::Mid,
        ObjectSnapKind::Center,
        ObjectSnapKind::Quad,
    ] {
        assert!(!modes.contains(kind));
        modes = modes.with(kind, true);
        assert!(modes.contains(kind));
    }
    assert_eq!(modes, ObjectSnapModes::LANDMARKS);
    assert!(!modes.contains(ObjectSnapKind::Near));
    assert_eq!(modes.with(ObjectSnapKind::Near, true), ObjectSnapModes::ALL);
    let mut doc = Document::default();
    doc.add_geometry(Geometry::Circle(circle())).unwrap();
    assert!(
        ObjectSnapCache::default()
            .nearest_projected_with_modes(
                &doc,
                [0., 0.],
                1.,
                |_| panic!("disabled query must not project"),
                ObjectSnapModes::NONE
            )
            .unwrap()
            .is_none()
    );
}

#[test]
fn numerical_capture_matches_independent_circle_distance_and_handles_invisible_projections() {
    for angle in [0.001, 0.41, 1.7, 3.2, 5.9] {
        for radius in [0.2, 0.99, 1., 1.01, 2.] {
            let (sin, cos) = Real::sin_cos(angle);
            let cursor = [radius * cos, radius * sin];
            let actual = projected_distance(|t| {
                let (s, c) = (t * std::f64::consts::TAU).sin_cos();
                Some((c - cursor[0]).hypot(s - cursor[1]))
            })
            .unwrap();
            assert!((actual - Real::abs(radius - 1.)).abs() < 1e-9);
        }
    }
    assert!(projected_distance(|_| None).is_none());
    // Disconnected projected branches must not create a chord through the cursor.
    assert_eq!(
        projected_distance(|t| if (0.2..0.8).contains(&t) {
            None
        } else {
            Some(10.)
        }),
        Some(10.)
    );
}

#[test]
fn conic_hover_refines_to_subpixel_accuracy_at_high_zoom() {
    let angle: Real = 0.41;
    let (s, c) = angle.sin_cos();
    for zoom in [1e6, 1e9, 1e12] {
        let distance = projected_distance(|t| {
            let (y, x) = (t * std::f64::consts::TAU).sin_cos();
            Some(((x - c) * zoom).hypot((y - s) * zoom))
        })
        .unwrap();
        assert!(
            distance < 0.01,
            "{zoom}: {distance} pixels from a point on the circle"
        );
    }
}

#[test]
fn axis_aligned_and_projected_center_capture_agree_at_large_origins() {
    for shift in [0., 2_f64.powi(40)] {
        let center = p(shift + 4., shift - 4., shift);
        let circle = Circle3::try_new(
            center,
            2.,
            Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mut doc = Document::default();
        doc.add_geometry(Geometry::Circle(circle)).unwrap();
        let origin = p(shift, shift, shift);
        let mut cache = ObjectSnapCache::default();
        let modes = ObjectSnapModes::only(ObjectSnapKind::Center);
        let axis = cache
            .nearest_axis_aligned_with_modes(
                &doc,
                PointCloudProjection::Xy,
                origin,
                [2.4, -2.8],
                0.1,
                modes,
            )
            .unwrap()
            .unwrap();
        let projected = cache
            .nearest_projected_with_modes(
                &doc,
                [2.4, -2.8],
                0.1,
                |p| Some([p.x() - shift, p.y() - shift]),
                modes,
            )
            .unwrap()
            .unwrap();
        assert_eq!(axis, projected);
        assert_eq!(axis.point(), center);
    }
}

#[test]
fn far_conics_are_culled_before_refinement_and_partial_visibility_is_not_a_rejection() {
    use std::cell::Cell;
    let mut doc = Document::default();
    doc.add_geometry(Geometry::Circle(circle())).unwrap();
    let calls = Cell::new(0);
    let mut cache = ObjectSnapCache::default();
    let snap = cache
        .nearest_projected_with_modes(
            &doc,
            [1000., 1000.],
            0.1,
            |point| {
                calls.set(calls.get() + 1);
                Some([point.x(), point.y()])
            },
            ObjectSnapModes::only(ObjectSnapKind::Center),
        )
        .unwrap();
    assert!(snap.is_none());
    assert!(calls.get() <= 9);
    // Camera near-plane clipping hides some corners of the bound, but the
    // center and the hovered part of the conic remain projectable.
    let snap = cache
        .nearest_projected_with_modes(
            &doc,
            [2.4, -2.8],
            0.1,
            |point| (point.y() > -5.).then_some([point.x(), point.y()]),
            ObjectSnapModes::only(ObjectSnapKind::Center),
        )
        .unwrap()
        .unwrap();
    assert_eq!(snap.kind(), ObjectSnapKind::Center);
}

#[test]
#[ignore = "manual release-mode Center hover timing diagnostic"]
fn center_hover_scene_timing() {
    for count in [1, 100, 1000] {
        let mut doc = Document::default();
        for i in 0..count {
            doc.add_geometry(Geometry::Circle(
                Circle3::try_new(
                    p(4. + 20. * i as Real, -4., 0.),
                    2.,
                    Vector3::try_new(0., 0., 1.)
                        .unwrap()
                        .normalized_nonzero()
                        .unwrap(),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        }
        let mut cache = ObjectSnapCache::default();
        let mut run = || {
            cache
                .nearest_projected(&doc, [2.4, -2.8], 0.1, |p| Some([p.x(), p.y()]))
                .unwrap()
        };
        assert!(run().is_some());
        let start = std::time::Instant::now();
        for _ in 0..100 {
            std::hint::black_box(run());
        }
        eprintln!(
            "Center hover, {count} circles: {:?}/query",
            start.elapsed() / 100
        );
    }
}
