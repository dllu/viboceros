//! Square admission is separate from Euclidean proximity and feature ranking.
use super::*;
use viboceros_geometry::{
    Circle3, LineSegment, MeshFace, NurbsCurve, Polyline3, Tolerance, TriangleMesh,
};

fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 1.).unwrap()
}

fn line(a: Point3, b: Point3) -> Geometry {
    Geometry::Line(LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap())
}

fn query(
    geometry: Geometry,
    cursor: [Real; 2],
    kind: ObjectSnapKind,
    axis: bool,
) -> Option<ObjectSnap> {
    let mut document = Document::default();
    document.add_geometry(geometry).unwrap();
    let mut cache = ObjectSnapCache::default();
    let options = ObjectSnapOptions {
        modes: ObjectSnapModes::only(kind),
        mesh_edges: true,
    };
    if axis {
        cache
            .nearest_axis_aligned_with_options(
                &document,
                PointCloudProjection::Xy,
                p(0., 0.),
                cursor,
                1.,
                options,
            )
            .unwrap()
    } else {
        cache
            .nearest_projected_with_options(
                &document,
                cursor,
                1.,
                |p| Some([p.x() / p.z(), p.y() / p.z()]),
                options,
            )
            .unwrap()
    }
}

#[test]
fn square_corners_admit_discrete_and_near_targets_with_euclidean_distances() {
    let corner = p(1., 1.);
    let mesh = Geometry::Mesh(
        TriangleMesh::try_new_faces(
            vec![p(-9., 1.), p(11., 1.), p(11., 21.), p(-9., 21.)],
            vec![MeshFace::Quad([0, 1, 2, 3])],
            Tolerance::DEFAULT,
        )
        .unwrap(),
    );
    for axis in [false, true] {
        for (geometry, kind) in [
            (Geometry::Point(corner), ObjectSnapKind::Point),
            (
                Geometry::PointCloud(
                    PointCloud3::try_new(vec![p(1.01, 0.), corner, p(-1., -1.)]).unwrap(),
                ),
                ObjectSnapKind::Point,
            ),
            (line(corner, p(20., 1.)), ObjectSnapKind::End),
            (line(corner, p(20., 1.)), ObjectSnapKind::Near),
            (mesh.clone(), ObjectSnapKind::Mid),
        ] {
            let hit = query(geometry, [0.; 2], kind, axis).unwrap();
            assert_eq!(hit.kind(), kind);
            assert_eq!(hit.point(), corner);
            assert_eq!(hit.distance(), 2.0_f64.sqrt());
        }
        assert!(
            query(
                Geometry::Point(p(1.0001, 0.)),
                [0.; 2],
                ObjectSnapKind::Point,
                axis
            )
            .is_none()
        );
    }
}

#[test]
fn corner_hover_can_capture_distant_mid_and_polygon_center_targets() {
    let a = p(0.75, 0.75);
    let b = p(20.75, 0.75);
    for axis in [false, true] {
        for geometry in [
            line(a, b),
            Geometry::Polyline(Polyline3::try_new(vec![a, b], Tolerance::DEFAULT).unwrap()),
            Geometry::NurbsCurve(NurbsCurve::try_new(1, vec![a, b], vec![0., 0., 1., 1.]).unwrap()),
            Geometry::NurbsCurve(
                NurbsCurve::try_new(2, vec![a, p(10.75, 0.75), b], vec![0., 0., 0., 1., 1., 1.])
                    .unwrap(),
            ),
        ] {
            let hit = query(geometry, [0.; 2], ObjectSnapKind::Mid, axis).unwrap();
            assert!(hit.point().distance_to(p(10.75, 0.75)).unwrap() < 1e-10);
            assert!((hit.distance() - 0.75_f64.hypot(0.75)).abs() < 1e-12);
        }
        let polygon = Geometry::Polyline(
            Polyline3::try_new(
                vec![a, b, p(20.75, 20.75), p(0.75, 20.75), a],
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
        let hit = query(polygon, [0.; 2], ObjectSnapKind::Center, axis).unwrap();
        assert!(hit.point().distance_to(p(10.75, 10.75)).unwrap() < 1e-10);
        assert!((hit.distance() - 0.75_f64.hypot(0.75)).abs() < 1e-12);
    }
}

#[test]
fn curved_center_and_near_use_corner_admission_after_euclidean_minimization() {
    let normal = Vector3::try_new(0., 0., 1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let circle = Circle3::try_new(p(3., 3.), 2.0_f64.sqrt(), normal, Tolerance::DEFAULT).unwrap();
    for axis in [false, true] {
        for kind in [ObjectSnapKind::Center, ObjectSnapKind::Near] {
            let hit = query(Geometry::Circle(circle), [1.25; 2], kind, axis).unwrap();
            let target = if kind == ObjectSnapKind::Center {
                p(3., 3.)
            } else {
                p(2., 2.)
            };
            assert!(hit.point().distance_to(target).unwrap() < 1e-7);
            assert!((hit.distance() - 0.75_f64.hypot(0.75)).abs() < 1e-10);
        }
    }
}

#[test]
fn a_wire_crossing_the_square_does_not_clamp_an_outside_nearest_point() {
    // The endpoint (0.9,-0.9) is in the square. The Euclidean closest point
    // is (1.08,-0.36), outside it; clipping to the box would falsely capture.
    for axis in [false, true] {
        for reverse in [false, true] {
            let (a, b) = if reverse {
                (p(1.2, 0.), p(0.9, -0.9))
            } else {
                (p(0.9, -0.9), p(1.2, 0.))
            };
            for geometry in [
                line(a, b),
                Geometry::NurbsCurve(
                    NurbsCurve::try_new(1, vec![a, b], vec![0., 0., 1., 1.]).unwrap(),
                ),
                Geometry::NurbsCurve(
                    NurbsCurve::try_new(
                        2,
                        vec![a, a.midpoint(b).unwrap(), b],
                        vec![0., 0., 0., 1., 1., 1.],
                    )
                    .unwrap(),
                ),
            ] {
                assert!(query(geometry, [0.; 2], ObjectSnapKind::Near, axis).is_none());
            }
        }
    }
}
