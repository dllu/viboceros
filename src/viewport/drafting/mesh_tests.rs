use super::*;
use viboceros_drafting::{ObjectSnapKind, ObjectSnapOptions};
use viboceros_geometry::{MeshFace, TriangleMesh};

#[test]
fn mesh_policy_reaches_ordinary_and_edge_constrained_prompts_in_all_views() {
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    for kind in [
        ViewKind::Top,
        ViewKind::Front,
        ViewKind::Right,
        ViewKind::Perspective,
    ] {
        let p = |x, y| match kind {
            ViewKind::Front => Point3::try_new(x, 7., y).unwrap(),
            ViewKind::Right => Point3::try_new(7., x, y).unwrap(),
            _ => Point3::try_new(x, y, 7.).unwrap(),
        };
        let mut doc = Document::default();
        let id = doc
            .add_geometry(Geometry::Mesh(
                TriangleMesh::try_new_faces(
                    vec![p(-5., 0.), p(5., 0.), p(5., -4.), p(-5., -4.)],
                    vec![MeshFace::Quad([0, 1, 2, 3])],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let mut view = Viewport::new(kind);
        view.target = NaVector3::from(p(0., 0.).to_array());
        let pointer = view.project(p(-3.7, 0.), area).unwrap();
        let modes = ObjectSnapModes::only(ObjectSnapKind::Near);
        assert!(view.object_snap(pointer, area, &doc, modes).is_none());
        let cursor = view
            .drafting_cursor(
                pointer,
                area,
                &doc,
                DraftingInput {
                    active: true,
                    osnap: modes,
                    mesh_edges: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(cursor.object_snap.unwrap().object_id(), id);
        assert!(cursor.point.distance_to(p(-3.7, 0.)).unwrap() < 1e-5);
        let edge =
            NurbsCurve::try_new(1, vec![p(-6., -3.), p(6., -3.)], vec![0., 0., 12., 12.]).unwrap();
        let target = view
            .edge_point_cursor(
                &edge,
                None,
                pointer,
                area,
                &doc,
                ObjectSnapOptions {
                    modes,
                    mesh_edges: true,
                },
            )
            .unwrap();
        assert!((target.parameter - 2.3).abs() < 1e-5);
    }
}

#[test]
fn vertex_snap_reaches_all_four_viewports_with_mesh_wires_disabled() {
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    for kind in [
        ViewKind::Top,
        ViewKind::Front,
        ViewKind::Right,
        ViewKind::Perspective,
    ] {
        let p = |x, y| match kind {
            ViewKind::Front => Point3::try_new(x, 7., y).unwrap(),
            ViewKind::Right => Point3::try_new(7., x, y).unwrap(),
            _ => Point3::try_new(x, y, 7.).unwrap(),
        };
        let vertices = [p(-5., 0.), p(5., 0.), p(5., -4.), p(-5., -4.)];
        let mut doc = Document::default();
        let id = doc
            .add_geometry(Geometry::Mesh(
                TriangleMesh::try_new_faces(
                    vertices.to_vec(),
                    vec![MeshFace::Quad([0, 1, 2, 3])],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let mut view = Viewport::new(kind);
        view.target = NaVector3::from(p(0., 0.).to_array());
        let pointer = view.project(vertices[0], area).unwrap() + Vec2::new(2., 2.);
        for mode in [ObjectSnapKind::Point, ObjectSnapKind::End] {
            assert!(
                view.object_snap(
                    pointer,
                    area,
                    &doc,
                    ObjectSnapOptions {
                        modes: ObjectSnapModes::only(mode),
                        mesh_edges: true,
                    },
                )
                .is_none(),
                "{kind:?}: {mode:?} captured a mesh vertex"
            );
        }
        let cursor = view
            .drafting_cursor(
                pointer,
                area,
                &doc,
                DraftingInput {
                    active: true,
                    osnap: ObjectSnapModes::only(ObjectSnapKind::Vertex),
                    mesh_edges: false,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(cursor.object_snap.unwrap().object_id(), id);
        assert_eq!(cursor.object_snap.unwrap().kind(), ObjectSnapKind::Vertex);
        assert_eq!(cursor.point, vertices[0]);
    }
}
