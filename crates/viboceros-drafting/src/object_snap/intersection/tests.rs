use super::*;
use crate::object_snap::{ObjectSnapKind, ObjectSnapModes, ObjectSnapOptions};
use viboceros_document::Geometry;
use viboceros_geometry::{
    Circle3, CircularArc3, Ellipse3, LineSegment, MeshFace, NurbsCurve, NurbsSurface,
    PointCloudProjection, Polyline3, Tolerance, TriangleMesh, UnitVector3,
};

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn line(a: Point3, b: Point3) -> Geometry {
    Geometry::Line(LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap())
}

fn snap(doc: &Document, mesh_edges: bool) -> Option<super::super::ObjectSnap> {
    ObjectSnapCache::default()
        .nearest_axis_aligned_with_options(
            doc,
            PointCloudProjection::Xy,
            p(0., 0., 0.),
            [0.05, -0.05],
            0.2,
            ObjectSnapOptions {
                modes: ObjectSnapModes::only(ObjectSnapKind::Intersection),
                mesh_edges,
            },
        )
        .unwrap()
}

#[test]
fn crossings_include_apparent_depth_and_endpoint_contact_but_not_overlap() {
    let mut doc = Document::default();
    doc.add_geometry(line(p(-2., 0., 0.), p(2., 0., 0.)))
        .unwrap();
    assert!(snap(&doc, false).is_none());
    let second = doc
        .add_geometry(line(p(0., -2., 1.), p(0., 2., 1.)))
        .unwrap();
    let hit = snap(&doc, false).unwrap();
    assert_eq!(hit.kind(), ObjectSnapKind::Intersection);
    assert!(hit.point().distance_to(p(0., 0., 1.)).unwrap() < 1e-12);
    assert_eq!(hit.object_id(), second);
    doc.delete_object(second).unwrap();
    doc.add_geometry(line(p(0., 0., 0.), p(0., 2., 0.)))
        .unwrap();
    assert!(
        snap(&doc, false)
            .unwrap()
            .point()
            .distance_to(p(0., 0., 0.))
            .unwrap()
            < 1e-12
    );
    let mut overlap = Document::default();
    overlap
        .add_geometry(line(p(-2., 0., 0.), p(2., 0., 0.)))
        .unwrap();
    overlap
        .add_geometry(line(p(-1., 0., 0.), p(1., 0., 0.)))
        .unwrap();
    assert!(snap(&overlap, false).is_none());
}

#[test]
fn mesh_wire_intersection_obeys_source_switch() {
    let mesh = TriangleMesh::try_new_faces(
        vec![p(-2., 0., 0.), p(2., 0., 0.), p(2., 2., 0.), p(-2., 2., 0.)],
        vec![MeshFace::Quad([0, 1, 2, 3])],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut doc = Document::default();
    doc.add_geometry(Geometry::Mesh(mesh)).unwrap();
    let line_id = doc
        .add_geometry(line(p(0., -2., 0.), p(0., 2., 0.)))
        .unwrap();
    assert!(snap(&doc, false).is_none());
    let hit = snap(&doc, true).unwrap();
    assert!(hit.point().distance_to(p(0., 0., 0.)).unwrap() < 1e-12);
    assert_eq!(hit.object_id(), line_id);
}

#[test]
fn a_single_mesh_wire_corner_is_not_an_intersection() {
    let mut doc = Document::default();
    doc.add_geometry(Geometry::Mesh(
        TriangleMesh::try_new_faces(
            vec![p(-2., 0., 0.), p(0., 0., 0.), p(0., 2., 0.)],
            vec![MeshFace::Triangle([0, 1, 2])],
            Tolerance::DEFAULT,
        )
        .unwrap(),
    ))
    .unwrap();
    assert!(snap(&doc, false).is_none());
    assert!(snap(&doc, true).is_none());
}

#[test]
fn repeated_intersections_reuse_mesh_wire_hierarchy() {
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    for y in 0..32 {
        for x in 0..32 {
            let base = vertices.len() as u32;
            vertices.extend(
                [[2., -2.], [8., -2.], [8., -8.], [2., -8.]]
                    .map(|[a, b]| p(a + 10. * x as Real, b - 10. * y as Real, 0.)),
            );
            faces.push(MeshFace::Quad([base, base + 1, base + 2, base + 3]));
        }
    }
    let mut doc = Document::default();
    doc.add_geometry(Geometry::Mesh(
        TriangleMesh::try_new_faces(vertices, faces, Tolerance::DEFAULT).unwrap(),
    ))
    .unwrap();
    doc.add_geometry(line(p(5., -3., 0.), p(5., 0., 0.)))
        .unwrap();
    let mut cache = ObjectSnapCache::default();
    for _ in 0..50 {
        cache.meshes.intersection_visited = 0;
        let hit = cache
            .nearest_axis_aligned_with_options(
                &doc,
                PointCloudProjection::Xy,
                p(0., 0., 0.),
                [5.02, -1.98],
                0.2,
                ObjectSnapOptions {
                    modes: ObjectSnapModes::only(ObjectSnapKind::Intersection),
                    mesh_edges: true,
                },
            )
            .unwrap()
            .unwrap();
        assert!(hit.point().distance_to(p(5., -2., 0.)).unwrap() < 1e-10);
        assert!(cache.meshes.intersection_visited < 64);
    }
    assert_eq!(cache.meshes.builds, 1);
}

#[test]
fn polyline_and_degree_one_nurbs_segments_share_intersection_capture() {
    let mut doc = Document::default();
    doc.add_geometry(Geometry::Polyline(
        Polyline3::try_new(
            vec![p(-2., 0., 0.), p(2., 0., 0.), p(2., 2., 0.)],
            Tolerance::DEFAULT,
        )
        .unwrap(),
    ))
    .unwrap();
    doc.add_geometry(Geometry::NurbsCurve(
        NurbsCurve::try_new(1, vec![p(0., -2., 0.), p(0., 2., 0.)], vec![0., 0., 1., 1.]).unwrap(),
    ))
    .unwrap();
    assert!(
        snap(&doc, false)
            .unwrap()
            .point()
            .distance_to(p(0., 0., 0.))
            .unwrap()
            < 1e-12
    );
}

#[test]
fn a_single_polyline_snaps_at_its_corner_and_at_its_own_crossing() {
    for (vertices, expected) in [
        (
            vec![p(-2., 0., 0.), p(0., 0., 0.), p(0., 2., 0.)],
            p(0., 0., 0.),
        ),
        (
            vec![
                p(-2., -2., 0.),
                p(2., 2., 0.),
                p(-2., 2., 0.),
                p(2., -2., 0.),
            ],
            p(0., 0., 0.),
        ),
        (
            vec![
                p(-2., -2., 0.),
                p(2., 2., 0.),
                p(-2., 2., 1.),
                p(2., -2., 1.),
            ],
            p(0., 0., 1.),
        ),
    ] {
        let mut doc = Document::default();
        let id = doc
            .add_geometry(Geometry::Polyline(
                Polyline3::try_new(vertices, Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        let hit = snap(&doc, false).unwrap();
        assert_eq!(hit.kind(), ObjectSnapKind::Intersection);
        assert_eq!(hit.object_id(), id);
        assert!(hit.point().distance_to(expected).unwrap() < 1e-12);
    }
}

#[test]
fn straight_surface_boundary_intersects_a_line() {
    let surface = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![p(-2., 0., 0.), p(2., 0., 0.), p(-2., 2., 0.), p(2., 2., 0.)],
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let mut doc = Document::default();
    doc.add_geometry(Geometry::NurbsSurface(surface)).unwrap();
    doc.add_geometry(line(p(0., -2., 0.), p(0., 1., 0.)))
        .unwrap();
    assert!(
        snap(&doc, false)
            .unwrap()
            .point()
            .distance_to(p(0., 0., 0.))
            .unwrap()
            < 1e-12
    );
}

#[test]
fn circle_line_crossings_include_tangencies_and_reject_infinite_line_extension() {
    let circle = Geometry::Circle(
        Circle3::try_from_frame(
            p(0., 0., 0.),
            2.,
            UnitVector3::try_new(1., 0., 0., Tolerance::DEFAULT).unwrap(),
            UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
    );
    let pick = |wire: Geometry, cursor: [Real; 2]| {
        let mut doc = Document::default();
        doc.add_geometry(circle.clone()).unwrap();
        doc.add_geometry(wire).unwrap();
        ObjectSnapCache::default()
            .nearest_axis_aligned_with_options(
                &doc,
                PointCloudProjection::Xy,
                p(0., 0., 0.),
                cursor,
                0.2,
                ObjectSnapOptions {
                    modes: ObjectSnapModes::only(ObjectSnapKind::Intersection),
                    mesh_edges: false,
                },
            )
            .unwrap()
    };
    let root = 3.0_f64.sqrt();
    let transverse = pick(line(p(-3., 1., 0.), p(3., 1., 0.)), [root + 0.05, 1.05]).unwrap();
    assert!(transverse.point().distance_to(p(root, 1., 0.)).unwrap() < 1e-12);
    let tangent = pick(line(p(-3., 2., 0.), p(3., 2., 0.)), [0.05, 2.05]).unwrap();
    assert!(tangent.point().distance_to(p(0., 2., 0.)).unwrap() < 1e-12);
    assert!(pick(line(p(-3., 2.000_001, 0.), p(3., 2.000_001, 0.)), [0., 2.]).is_none());
    assert!(pick(line(p(-3., 1., 0.), p(-2.5, 1., 0.)), [-root, 1.]).is_none());
}

#[test]
fn arc_sweep_and_ellipse_tangent_bound_finite_intersections() {
    let arc = Geometry::Arc(
        CircularArc3::try_from_three_points(
            p(2., 0., 0.),
            p(0., 2., 0.),
            p(-2., 0., 0.),
            Tolerance::DEFAULT,
        )
        .unwrap(),
    );
    let ellipse = Geometry::Ellipse(
        Ellipse3::try_new(
            p(0., 0., 0.),
            3.,
            2.,
            UnitVector3::try_new(1., 0., 0., Tolerance::DEFAULT).unwrap(),
            UnitVector3::try_new(0., 1., 0., Tolerance::DEFAULT).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
    );
    let pick = |curve: Geometry, wire: Geometry, cursor: [Real; 2]| {
        let mut doc = Document::default();
        doc.add_geometry(curve).unwrap();
        doc.add_geometry(wire).unwrap();
        ObjectSnapCache::default()
            .nearest_axis_aligned_with_options(
                &doc,
                PointCloudProjection::Xy,
                p(0., 0., 0.),
                cursor,
                0.2,
                ObjectSnapOptions {
                    modes: ObjectSnapModes::only(ObjectSnapKind::Intersection),
                    mesh_edges: false,
                },
            )
            .unwrap()
    };
    let root = 3.0_f64.sqrt();
    assert!(
        pick(
            arc.clone(),
            line(p(-3., -1., 0.), p(3., -1., 0.)),
            [root, -1.]
        )
        .is_none()
    );
    assert!(
        pick(
            arc,
            line(p(-3., 1., 0.), p(3., 1., 0.)),
            [root + 0.05, 1.05]
        )
        .unwrap()
        .point()
        .distance_to(p(root, 1., 0.))
        .unwrap()
            < 1e-12
    );
    assert!(
        pick(ellipse, line(p(-4., 2., 0.), p(4., 2., 0.)), [0.05, 2.05])
            .unwrap()
            .point()
            .distance_to(p(0., 2., 0.))
            .unwrap()
            < 1e-12
    );
}
