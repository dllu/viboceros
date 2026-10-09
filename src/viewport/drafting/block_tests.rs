use super::*;
use viboceros_document::{BlockContent, BlockMember, BlockReference, ObjectAttributes};
use viboceros_drafting::{ObjectSnapKind, ObjectSnapOptions};
use viboceros_geometry::{AffineTransform3, LineSegment};

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn block_member_endpoints_and_insertion_points_snap_in_all_four_viewports() {
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    let mut doc = Document::default();
    let line = Geometry::Line(
        LineSegment::try_new(p(2., 3., 4.), p(5., 6., 7.), doc.tolerance()).unwrap(),
    );
    let definition = doc
        .add_block_definition(
            "part",
            vec![BlockMember::new(
                BlockContent::Geometry(line.into()),
                ObjectAttributes::on_layer(doc.current_layer_id()),
            )],
        )
        .unwrap();
    let reference = BlockReference::try_new(
        definition,
        AffineTransform3::from_translation(Vector3::try_new(10., 20., 30.).unwrap()),
    )
    .unwrap();
    let root = doc.add_block_instance(reference).unwrap();
    let before = format!("{doc:?}");
    for kind in [
        ViewKind::Top,
        ViewKind::Front,
        ViewKind::Plan,
        ViewKind::Perspective,
    ] {
        let mut view = Viewport::new(kind);
        view.target = NaVector3::new(10., 20., 30.);
        for (mode, target) in [
            (ObjectSnapKind::Point, p(10., 20., 30.)),
            (ObjectSnapKind::End, p(12., 23., 34.)),
        ] {
            let cursor = view.project(target, area).unwrap();
            let snap = view
                .object_snap(cursor, area, &doc, ObjectSnapModes::only(mode))
                .unwrap();
            assert_eq!(snap.object_id(), root);
            assert_eq!(snap.kind(), mode);
            assert!(snap.point().distance_to(target).unwrap() < 1e-10);
        }
    }
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn block_mesh_wire_policy_reaches_drafting_cursor_and_reports_root_identity() {
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    let mut doc = Document::default();
    let mesh = Geometry::Mesh(
        TriangleMesh::try_new(
            vec![p(-5., 0., 0.), p(5., 0., 0.), p(-5., -4., 0.)],
            vec![[0, 1, 2]],
            doc.tolerance(),
        )
        .unwrap(),
    );
    let definition = doc
        .add_block_definition(
            "mesh",
            vec![BlockMember::new(
                BlockContent::Geometry(mesh.into()),
                ObjectAttributes::on_layer(doc.current_layer_id()),
            )],
        )
        .unwrap();
    let root = doc
        .add_block_instance(
            BlockReference::try_new(definition, AffineTransform3::identity()).unwrap(),
        )
        .unwrap();
    let view = Viewport::new(ViewKind::Top);
    let pixel = view.project(p(2., 0., 0.), area).unwrap();
    let options = ObjectSnapOptions {
        modes: ObjectSnapModes::only(ObjectSnapKind::Near),
        mesh_edges: false,
    };
    assert!(view.object_snap(pixel, area, &doc, options).is_none());
    let snapped = view
        .drafting_cursor(
            pixel,
            area,
            &doc,
            DraftingInput {
                active: true,
                osnap: options.modes,
                mesh_edges: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(snapped.object_snap.unwrap().object_id(), root);
    assert!(snapped.point.distance_to(p(2., 0., 0.)).unwrap() < 1e-5);
}
