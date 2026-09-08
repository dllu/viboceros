use super::*;

#[test]
fn tracking_keeps_a_capturable_axis_when_the_other_distance_overflows() {
    for sign in [-1.0, 1.0] {
        let anchor = point(-sign * Real::MAX, -sign * Real::MAX, 7.0);
        for (cursor, axis, expected) in [
            (
                point(sign * Real::MAX, anchor.y(), 0.0),
                TrackAxis::Horizontal,
                point(sign * Real::MAX, anchor.y(), 7.0),
            ),
            (
                point(anchor.x(), sign * Real::MAX, 0.0),
                TrackAxis::Vertical,
                point(anchor.x(), sign * Real::MAX, 7.0),
            ),
        ] {
            let track = orthogonal_track(cursor, anchor, 1.0).unwrap().unwrap();
            assert_eq!(track.axis(), axis);
            assert_eq!(track.point(), expected);
        }
        assert!(
            orthogonal_track(point(sign * Real::MAX, sign * Real::MAX, 0.0), anchor, 1.0)
                .unwrap()
                .is_none()
        );
    }
}
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{
    Brep, Circle3, CircularArc3, Ellipse3, Frame3, LineSegment, NurbsCurve, NurbsSurface,
    PointCloud3, Polyline3, Tolerance, UnitVector3, Vector3,
};

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn snaps_to_closest_line_features_and_respects_radius() {
    let mut document = Document::default();
    let line_id = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                point(0.0, 0.0, 3.0),
                point(10.0, 0.0, 3.0),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ))
        .unwrap();

    let midpoint = nearest_object_snap(&document, point(5.2, 0.1, 0.0), 0.5)
        .unwrap()
        .unwrap();
    assert_eq!(midpoint.object_id(), line_id);
    assert_eq!(midpoint.kind(), ObjectSnapKind::Mid);
    assert_eq!(midpoint.point(), point(5.0, 0.0, 3.0));
    assert!(
        nearest_object_snap(&document, point(5.2, 0.1, 0.0), 0.1)
            .unwrap()
            .is_none()
    );
}

#[test]
fn exact_ties_use_feature_priority_not_document_order() {
    let mut document = Document::default();
    document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ))
        .unwrap();
    let point_id = document
        .add_geometry(Geometry::Point(point(0.0, 0.0, 5.0)))
        .unwrap();

    let snap = nearest_object_snap(&document, point(0.0, 0.0, 0.0), 1.0)
        .unwrap()
        .unwrap();
    assert_eq!(snap.kind(), ObjectSnapKind::Point);
    assert_eq!(snap.object_id(), point_id);
    assert_eq!(snap.point().z(), 5.0);
}

#[test]
fn relative_xy_snaps_match_explicit_local_projection() {
    for translation in [0.0, 2.0_f64.powi(52), -2.0_f64.powi(52)] {
        let origin = point(translation, translation, 0.0);
        let local = |x, y| point(translation + x, translation + y, 3.0);
        let mut document = Document::default();
        document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(local(-2.0, 0.0), local(2.0, 0.0), Tolerance::DEFAULT)
                    .unwrap(),
            ))
            .unwrap();
        document
            .add_geometry(Geometry::PointCloud(
                PointCloud3::try_new(vec![local(0.0, 1.0), local(1.0, 0.0), local(0.0, -1.0)])
                    .unwrap(),
            ))
            .unwrap();
        let locked = document
            .add_geometry(Geometry::Point(local(-1.0, 0.0)))
            .unwrap();
        document.set_objects_locked([locked], true).unwrap();
        for x in -12..=12 {
            for y in -8..=8 {
                let offset = [Real::from(x) / 4.0, Real::from(y) / 4.0];
                for radius in [0.25, 0.5, 1.0] {
                    let expected = nearest_object_snap_projected(&document, offset, radius, |p| {
                        Some([p.x() - translation, p.y() - translation])
                    })
                    .unwrap();
                    assert_eq!(
                        nearest_object_snap_relative(&document, origin, offset, radius).unwrap(),
                        expected
                    );
                }
            }
        }
    }
}

#[test]
fn relative_xy_snap_validates_offsets_even_without_objects() {
    let document = Document::default();
    for invalid in [Real::NAN, Real::INFINITY, Real::NEG_INFINITY] {
        for offset in [[invalid, 0.0], [0.0, invalid]] {
            assert!(
                nearest_object_snap_relative(&document, point(0.0, 0.0, 0.0), offset, 1.0).is_err()
            );
        }
    }
}

#[test]
fn snaps_to_the_nearest_point_cloud_member_in_xy() {
    let mut document = Document::default();
    let cloud_id = document
        .add_geometry(Geometry::PointCloud(
            PointCloud3::try_new(vec![
                point(-4.0, 2.0, 9.0),
                point(3.0, 5.0, -7.0),
                point(8.0, 1.0, 2.0),
            ])
            .unwrap(),
        ))
        .unwrap();
    let snap = nearest_object_snap(&document, point(3.08, 5.02, 100.0), 0.1)
        .unwrap()
        .unwrap();
    assert_eq!(snap.object_id(), cloud_id);
    assert_eq!(snap.kind(), ObjectSnapKind::Point);
    assert_eq!(snap.point(), point(3.0, 5.0, -7.0));
}

#[test]
fn projected_snapping_uses_the_requested_view_plane() {
    let mut document = Document::default();
    document
        .add_geometry(Geometry::Point(point(0.0, 0.0, 0.0)))
        .unwrap();
    let cloud_id = document
        .add_geometry(Geometry::PointCloud(
            PointCloud3::try_new(vec![point(0.0, 4.0, 10.0), point(6.0, 0.0, 10.0)]).unwrap(),
        ))
        .unwrap();

    let snap = nearest_object_snap_projected(&document, [0.05, 10.02], 0.1, |candidate| {
        Some([candidate.x(), candidate.z()])
    })
    .unwrap()
    .unwrap();
    assert_eq!(snap.object_id(), cloud_id);
    assert_eq!(snap.point(), point(0.0, 4.0, 10.0));
    assert!(snap.distance() < 0.1);
}

#[test]
fn polycurve_segment_junctions_snap_to_the_composite_object() {
    let segments = vec![
        NurbsCurve::try_clamped_uniform(
            2,
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 2.0, 0.0),
                point(3.0, 0.0, 0.0),
            ],
        )
        .unwrap(),
        NurbsCurve::try_clamped_uniform(1, vec![point(3.0, 0.0, 0.0), point(4.0, 3.0, 0.0)])
            .unwrap(),
    ];
    let mut document = Document::default();
    let id = document
        .add_geometry(Geometry::PolyCurve(
            viboceros_geometry::PolyCurve3::try_new(segments).unwrap(),
        ))
        .unwrap();
    for target in [
        point(0.0, 0.0, 0.0),
        point(3.0, 0.0, 0.0),
        point(4.0, 3.0, 0.0),
    ] {
        let snap = nearest_object_snap(&document, target, 0.1)
            .unwrap()
            .unwrap();
        assert_eq!(snap.object_id(), id);
        assert_eq!(snap.kind(), ObjectSnapKind::End);
        assert_eq!(snap.point(), target);
    }
    document.set_objects_visibility([id], false).unwrap();
    assert!(
        nearest_object_snap(&document, point(3.0, 0.0, 0.0), 0.1)
            .unwrap()
            .is_none()
    );
}

#[test]
fn curve_endpoints_are_available_to_osnap() {
    let mut document = Document::default();
    document
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_clamped_uniform(
                2,
                vec![
                    point(1.0, 2.0, 0.0),
                    point(3.0, 5.0, 0.0),
                    point(7.0, 2.0, 0.0),
                ],
            )
            .unwrap(),
        ))
        .unwrap();

    let snap = nearest_object_snap(&document, point(7.05, 2.0, 0.0), 0.1)
        .unwrap()
        .unwrap();
    assert_eq!(snap.kind(), ObjectSnapKind::End);
    assert_eq!(snap.point(), point(7.0, 2.0, 0.0));
}

#[test]
fn surface_corners_and_center_are_available_to_osnap() {
    let mut document = Document::default();
    let id = document
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_bilinear([
                point(1.0, 2.0, 0.0),
                point(7.0, 2.0, 0.0),
                point(7.0, 6.0, 2.0),
                point(1.0, 6.0, 2.0),
            ])
            .unwrap(),
        ))
        .unwrap();

    let corner = nearest_object_snap(&document, point(7.02, 6.01, 0.0), 0.1)
        .unwrap()
        .unwrap();
    assert_eq!(corner.object_id(), id);
    assert_eq!(corner.kind(), ObjectSnapKind::End);
    assert_eq!(corner.point(), point(7.0, 6.0, 2.0));

    let center = nearest_object_snap(&document, point(4.01, 4.02, 0.0), 0.1)
        .unwrap()
        .unwrap();
    assert_eq!(center.kind(), ObjectSnapKind::Mid);
    assert_eq!(center.point(), point(4.0, 4.0, 1.0));
}

#[test]
fn brep_vertices_and_edge_midpoints_are_available_to_osnap() {
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        point(0.0, 0.0, 0.0),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let id = document
        .add_geometry(Geometry::Brep(
            Brep::try_box(
                frame,
                [[0.0, 4.0], [0.0, 6.0], [0.0, 2.0]],
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ))
        .unwrap();

    let vertex = nearest_object_snap(&document, point(4.02, 6.01, 0.0), 0.1)
        .unwrap()
        .unwrap();
    assert_eq!(vertex.object_id(), id);
    assert_eq!(vertex.kind(), ObjectSnapKind::End);
    assert_eq!(vertex.point(), point(4.0, 6.0, 0.0));

    let midpoint = nearest_object_snap(&document, point(2.01, 0.02, 0.0), 0.1)
        .unwrap()
        .unwrap();
    assert_eq!(midpoint.object_id(), id);
    assert_eq!(midpoint.kind(), ObjectSnapKind::Mid);
    assert_eq!(midpoint.point(), point(2.0, 0.0, 0.0));
}

#[test]
fn circle_and_arc_features_are_available_to_osnap() {
    let mut document = Document::default();
    let normal = UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap();
    let circle_id = document
        .add_geometry(Geometry::Circle(
            Circle3::try_new(point(0.0, 0.0, 3.0), 2.0, normal, Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    let arc_id = document
        .add_geometry(Geometry::Arc(
            CircularArc3::try_from_three_points(
                point(9.0, 0.0, 4.0),
                point(10.0, 1.0, 4.0),
                point(11.0, 0.0, 4.0),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ))
        .unwrap();

    let center = nearest_object_snap(&document, point(0.02, -0.01, 0.0), 0.1)
        .unwrap()
        .unwrap();
    assert_eq!(center.object_id(), circle_id);
    assert_eq!(center.kind(), ObjectSnapKind::Center);
    assert_eq!(center.point(), point(0.0, 0.0, 3.0));

    let quadrant = nearest_object_snap(&document, point(2.02, 0.01, 0.0), 0.1)
        .unwrap()
        .unwrap();
    assert_eq!(quadrant.kind(), ObjectSnapKind::Quad);
    assert_eq!(quadrant.point(), point(2.0, 0.0, 3.0));

    let midpoint = nearest_object_snap(&document, point(10.0, 1.02, 0.0), 0.1)
        .unwrap()
        .unwrap();
    assert_eq!(midpoint.object_id(), arc_id);
    assert_eq!(midpoint.kind(), ObjectSnapKind::Mid);
    assert!(
        midpoint
            .point()
            .is_near(point(10.0, 1.0, 4.0), Tolerance::DEFAULT)
    );
}

#[test]
fn ellipse_center_and_quadrants_are_available_to_osnap() {
    let mut document = Document::default();
    let ellipse = Ellipse3::try_from_three_points(
        point(2.0, 3.0, 5.0),
        point(8.0, 3.0, 5.0),
        point(4.0, -1.0, 5.0),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let id = document.add_geometry(Geometry::Ellipse(ellipse)).unwrap();

    let center = nearest_object_snap(&document, point(2.02, 3.01, 0.0), 0.1)
        .unwrap()
        .unwrap();
    assert_eq!(center.object_id(), id);
    assert_eq!(center.kind(), ObjectSnapKind::Center);
    assert_eq!(center.point(), point(2.0, 3.0, 5.0));

    let quadrant = nearest_object_snap(&document, point(8.01, 3.02, 0.0), 0.1)
        .unwrap()
        .unwrap();
    assert_eq!(quadrant.object_id(), id);
    assert_eq!(quadrant.kind(), ObjectSnapKind::Quad);
    assert_eq!(quadrant.point(), point(8.0, 3.0, 5.0));
}

#[test]
fn polyline_vertices_and_segment_midpoints_are_available_to_osnap() {
    let mut document = Document::default();
    let id = document
        .add_geometry(Geometry::Polyline(
            Polyline3::try_new(
                vec![
                    point(0.0, 0.0, 2.0),
                    point(4.0, 0.0, 2.0),
                    point(4.0, 6.0, 2.0),
                ],
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ))
        .unwrap();
    let vertex = nearest_object_snap(&document, point(4.02, 0.01, 0.0), 0.1)
        .unwrap()
        .unwrap();
    assert_eq!(vertex.object_id(), id);
    assert_eq!(vertex.kind(), ObjectSnapKind::End);
    assert_eq!(vertex.point(), point(4.0, 0.0, 2.0));

    let midpoint = nearest_object_snap(&document, point(4.02, 3.01, 0.0), 0.1)
        .unwrap()
        .unwrap();
    assert_eq!(midpoint.kind(), ObjectSnapKind::Mid);
    assert_eq!(midpoint.point(), point(4.0, 3.0, 2.0));
}

#[test]
fn locked_objects_remain_snap_targets_but_hidden_objects_do_not() {
    let mut document = Document::default();
    let default = document.current_layer_id();
    let reference = document
        .add_layer("Reference", viboceros_document::ColorRgb::new(1, 2, 3))
        .unwrap();
    document.set_current_layer(reference).unwrap();
    let point_id = document
        .add_geometry(Geometry::Point(point(5.0, 6.0, 0.0)))
        .unwrap();
    document.set_current_layer(default).unwrap();

    document.set_layer_locked(reference, true).unwrap();
    assert_eq!(
        nearest_object_snap(&document, point(5.0, 6.0, 0.0), 0.1)
            .unwrap()
            .unwrap()
            .object_id(),
        point_id
    );
    document.set_layer_locked(reference, false).unwrap();
    document.set_objects_locked([point_id], true).unwrap();
    assert_eq!(
        nearest_object_snap(&document, point(5.0, 6.0, 0.0), 0.1)
            .unwrap()
            .unwrap()
            .object_id(),
        point_id
    );
    document.set_objects_visibility([point_id], false).unwrap();
    assert!(
        nearest_object_snap(&document, point(5.0, 6.0, 0.0), 0.1)
            .unwrap()
            .is_none()
    );
    document.set_objects_visibility([point_id], true).unwrap();
    document.set_layer_visibility(reference, false).unwrap();
    assert!(
        nearest_object_snap(&document, point(5.0, 6.0, 0.0), 0.1)
            .unwrap()
            .is_none()
    );
}

#[test]
fn orthogonal_tracking_preserves_the_anchor_plane() {
    let anchor = point(2.0, 3.0, 7.0);
    let horizontal = orthogonal_track(point(8.0, 3.1, 0.0), anchor, 0.2)
        .unwrap()
        .unwrap();
    assert_eq!(horizontal.axis(), TrackAxis::Horizontal);
    assert_eq!(horizontal.point(), point(8.0, 3.0, 7.0));

    let vertical = orthogonal_track(point(2.1, -4.0, 0.0), anchor, 0.2)
        .unwrap()
        .unwrap();
    assert_eq!(vertical.axis(), TrackAxis::Vertical);
    assert_eq!(vertical.point(), point(2.0, -4.0, 7.0));

    let both = orthogonal_track(point(2.1, 2.9, 0.0), anchor, 0.2)
        .unwrap()
        .unwrap();
    assert_eq!(both.axis(), TrackAxis::Both);
    assert_eq!(both.point(), anchor);
}

#[test]
fn rejects_invalid_capture_radii() {
    let document = Document::default();
    for radius in [0.0, -1.0, Real::NAN, Real::INFINITY] {
        assert_eq!(
            nearest_object_snap(&document, point(0.0, 0.0, 0.0), radius),
            Err(DraftingError::InvalidCaptureRadius)
        );
        assert_eq!(
            orthogonal_track(point(0.0, 0.0, 0.0), point(1.0, 1.0, 0.0), radius),
            Err(DraftingError::InvalidCaptureRadius)
        );
    }
}
