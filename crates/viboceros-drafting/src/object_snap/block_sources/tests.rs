use super::*;
use viboceros_document::{
    BlockContent, BlockDefinition, BlockMember, BlockReference, ObjectAttributes,
};
use viboceros_geometry::{
    AffineTransform3, Circle3, LineSegment, NurbsCurve, TriangleMesh, UnitVector3,
};
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn member(doc: &Document, g: Geometry) -> BlockMember {
    BlockMember::new(
        BlockContent::Geometry(g.into()),
        ObjectAttributes::on_layer(doc.current_layer_id()),
    )
}
fn placement(id: viboceros_document::BlockDefinitionId, x: f64, y: f64, z: f64) -> BlockReference {
    BlockReference::try_new(
        id,
        AffineTransform3::from_translation(Vector3::try_new(x, y, z).unwrap()),
    )
    .unwrap()
}
fn query(
    cache: &mut ObjectSnapCache,
    doc: &Document,
    point: Point3,
    kind: ObjectSnapKind,
    mesh: bool,
) -> Option<ObjectSnap> {
    cache
        .nearest_axis_aligned_with_options(
            doc,
            PointCloudProjection::Xy,
            point,
            [0.; 2],
            0.1,
            ObjectSnapOptions {
                modes: ObjectSnapModes::only(kind),
                mesh_edges: mesh,
            },
        )
        .unwrap()
}

#[test]
fn root_nested_and_empty_definition_insertion_points_use_point_snap_with_root_identity() {
    let mut doc = Document::default();
    let layer = doc
        .add_layer("nested", viboceros_document::ColorRgb::BLACK)
        .unwrap();
    let child = BlockDefinition::new(
        "child",
        vec![member(&doc, Geometry::Point(p(30., 30., 3.)))],
    );
    let empty = BlockDefinition::new("empty", vec![]);
    let root = BlockDefinition::new(
        "root",
        vec![
            BlockMember::new(
                BlockContent::Reference(placement(child.id(), 1., 2., 3.)),
                ObjectAttributes::on_layer(layer),
            ),
            BlockMember::new(
                BlockContent::Reference(placement(empty.id(), 5., 6., 7.)),
                ObjectAttributes::on_layer(layer),
            ),
        ],
    );
    let id = root.id();
    doc.set_block_definitions(vec![root, child, empty]).unwrap();
    let transform = AffineTransform3::try_new(
        [[0., -1., 0.], [1., 0., 0.], [0., 0., 1.]],
        Vector3::try_new(10., 20., 30.).unwrap(),
    )
    .unwrap();
    let object = doc
        .add_block_instance(BlockReference::try_new(id, transform).unwrap())
        .unwrap();
    let before = format!("{doc:?}");
    let mut cache = ObjectSnapCache::default();
    for point in [p(10., 20., 30.), p(8., 21., 33.), p(4., 25., 37.)] {
        let snap = query(&mut cache, &doc, point, ObjectSnapKind::Point, false).unwrap();
        assert_eq!(snap.point(), point);
        assert_eq!(snap.object_id(), object);
    }
    assert_eq!(format!("{doc:?}"), before);
    doc.set_layer_locked(layer, true).unwrap();
    assert!(
        query(
            &mut cache,
            &doc,
            p(8., 21., 33.),
            ObjectSnapKind::Point,
            false
        )
        .is_some()
    );
    doc.set_layer_visibility(layer, false).unwrap();
    assert!(
        query(
            &mut cache,
            &doc,
            p(8., 21., 33.),
            ObjectSnapKind::Point,
            false
        )
        .is_none()
    );
    assert!(
        query(
            &mut cache,
            &doc,
            p(10., 20., 30.),
            ObjectSnapKind::Point,
            false
        )
        .is_some()
    );
}

#[test]
fn every_existing_mode_queries_placed_members_and_returns_the_root() {
    let mut doc = Document::default();
    let line = |a, b| Geometry::Line(LineSegment::try_new(a, b, doc.tolerance()).unwrap());
    let geometries = vec![
        Geometry::Point(p(20., 20., 0.)),
        line(p(10., -3., 0.), p(10., 3., 0.)),
        line(p(7., 0., 0.), p(13., 0., 0.)),
        Geometry::Circle(
            Circle3::try_new(
                p(30., 0., 0.),
                2.,
                UnitVector3::try_new(0., 0., 1., doc.tolerance()).unwrap(),
                doc.tolerance(),
            )
            .unwrap(),
        ),
        Geometry::Mesh(
            TriangleMesh::try_new(
                vec![p(40., 0., 0.), p(44., 0., 0.), p(40., 4., 0.)],
                vec![[0, 1, 2]],
                doc.tolerance(),
            )
            .unwrap(),
        ),
    ];
    let definition = doc
        .add_block_definition(
            "part",
            geometries.into_iter().map(|g| member(&doc, g)).collect(),
        )
        .unwrap();
    let root = doc
        .add_block_instance(placement(definition, 100., 200., 300.))
        .unwrap();
    let cases = [
        (
            ObjectSnapKind::Point,
            p(120., 220., 300.),
            p(120., 220., 300.),
            false,
        ),
        (
            ObjectSnapKind::End,
            p(110., 203., 300.),
            p(110., 203., 300.),
            false,
        ),
        (
            ObjectSnapKind::Mid,
            p(110., 202., 300.),
            p(110., 200., 300.),
            false,
        ),
        (
            ObjectSnapKind::Near,
            p(110., 201., 300.),
            p(110., 201., 300.),
            false,
        ),
        (
            ObjectSnapKind::Center,
            p(132., 200., 300.),
            p(130., 200., 300.),
            false,
        ),
        (
            ObjectSnapKind::Quad,
            p(132., 200., 300.),
            p(132., 200., 300.),
            false,
        ),
        (
            ObjectSnapKind::Vertex,
            p(140., 200., 300.),
            p(140., 200., 300.),
            false,
        ),
        (
            ObjectSnapKind::Intersection,
            p(110., 200., 300.),
            p(110., 200., 300.),
            false,
        ),
        (
            ObjectSnapKind::Near,
            p(142., 200., 300.),
            p(142., 200., 300.),
            true,
        ),
    ];
    let mut cache = ObjectSnapCache::default();
    for (kind, cursor, expected, mesh) in cases {
        let snap = query(&mut cache, &doc, cursor, kind, mesh)
            .unwrap_or_else(|| panic!("missing {kind:?}"));
        assert_eq!(snap.object_id(), root);
        assert_eq!(snap.kind(), kind);
        assert!(
            snap.point().distance_to(expected).unwrap() < 1e-8,
            "{kind:?}: {:?}",
            snap.point()
        );
    }
    assert!(
        query(
            &mut cache,
            &doc,
            p(142., 200., 300.),
            ObjectSnapKind::Near,
            false
        )
        .is_none()
    );
}

fn curve(width: f64) -> Geometry {
    Geometry::NurbsCurve(
        NurbsCurve::try_new(
            3,
            vec![
                p(0., 0., 0.),
                p(width / 3., 1., 0.),
                p(2. * width / 3., 1., 0.),
                p(width, 0., 0.),
            ],
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap(),
    )
}

#[test]
fn lazy_features_reuse_proxy_ids_and_refresh_after_definition_edits_undo_and_removal() {
    let mut doc = Document::default();
    let layer = doc
        .add_layer("curve", viboceros_document::ColorRgb::BLACK)
        .unwrap();
    let definition = doc
        .add_block_definition(
            "curve",
            vec![BlockMember::new(
                BlockContent::Geometry(curve(6.).into()),
                ObjectAttributes::on_layer(layer),
            )],
        )
        .unwrap();
    let root = doc
        .add_block_instance(placement(definition, 10., 0., 0.))
        .unwrap();
    let mut cache = ObjectSnapCache::default();
    let target = p(13., 0.75, 0.);
    for _ in 0..4 {
        assert_eq!(
            query(&mut cache, &doc, target, ObjectSnapKind::Mid, false)
                .unwrap()
                .object_id(),
            root
        );
    }
    assert_eq!(cache.blocks.builds, 1);
    assert_eq!(cache.curve_feature_builds(), 1);
    doc.set_layer_visibility(layer, false).unwrap();
    assert!(query(&mut cache, &doc, target, ObjectSnapKind::Mid, false).is_none());
    doc.set_layer_visibility(layer, true).unwrap();
    assert!(query(&mut cache, &doc, target, ObjectSnapKind::Mid, false).is_some());
    assert_eq!(cache.blocks.builds, 1);
    assert_eq!(cache.curve_feature_builds(), 1);
    doc.replace_block_definition_members(
        definition,
        vec![BlockMember::new(
            BlockContent::Geometry(curve(12.).into()),
            ObjectAttributes::on_layer(layer),
        )],
    )
    .unwrap();
    assert!(
        query(
            &mut cache,
            &doc,
            p(16., 0.75, 0.),
            ObjectSnapKind::Mid,
            false
        )
        .is_some()
    );
    assert_eq!(cache.blocks.builds, 2);
    assert_eq!(cache.curve_feature_builds(), 2);
    doc.undo().unwrap();
    assert!(query(&mut cache, &doc, target, ObjectSnapKind::Mid, false).is_some());
    assert_eq!(cache.blocks.builds, 3);
    assert_eq!(cache.curve_feature_builds(), 3);
    doc.delete_object(root).unwrap();
    assert!(query(&mut cache, &doc, target, ObjectSnapKind::Mid, false).is_none());
    assert!(cache.blocks.entries.is_empty());
    assert_eq!(cache.curve_feature_entries(), 0);
}

#[test]
fn intersections_between_mesh_and_line_members_keep_distinct_cache_source_identities() {
    let mut doc = Document::default();
    let mesh = Geometry::Mesh(
        TriangleMesh::try_new(
            vec![p(-3., 0., 0.), p(3., 0., 0.), p(-3., -3., 0.)],
            vec![[0, 1, 2]],
            doc.tolerance(),
        )
        .unwrap(),
    );
    let line = Geometry::Line(
        LineSegment::try_new(p(0., -1., 1.), p(0., 1., 1.), doc.tolerance()).unwrap(),
    );
    let definition = doc
        .add_block_definition("crossing", vec![member(&doc, mesh), member(&doc, line)])
        .unwrap();
    let root = doc
        .add_block_instance(placement(definition, 10., 10., 0.))
        .unwrap();
    let mut cache = ObjectSnapCache::default();
    let snap = query(
        &mut cache,
        &doc,
        p(10., 10., 0.),
        ObjectSnapKind::Intersection,
        true,
    )
    .unwrap();
    assert_eq!(snap.object_id(), root);
    assert!(
        [p(10., 10., 0.), p(10., 10., 1.)].iter().any(|point| snap
            .point()
            .distance_to(*point)
            .unwrap()
            < 1e-10)
    );
}

#[test]
fn repeated_instances_use_independent_ids_and_suspended_queries_stay_cold() {
    let mut doc = Document::default();
    let definition = doc
        .add_block_definition("curve", vec![member(&doc, curve(6.))])
        .unwrap();
    let a = doc
        .add_block_instance(placement(definition, 10., 0., 0.))
        .unwrap();
    let b = doc
        .add_block_instance(placement(definition, 20., 0., 0.))
        .unwrap();
    let mut cache = ObjectSnapCache::default();
    assert!(
        cache
            .nearest_axis_aligned_with_modes(
                &doc,
                PointCloudProjection::Xy,
                p(13., 0.75, 0.),
                [0.; 2],
                0.1,
                ObjectSnapModes::NONE
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(cache.blocks.builds, 0);
    for (root, target) in [(a, p(13., 0.75, 0.)), (b, p(23., 0.75, 0.))] {
        assert_eq!(
            query(&mut cache, &doc, target, ObjectSnapKind::Mid, false)
                .unwrap()
                .object_id(),
            root
        );
    }
    assert_eq!(cache.blocks.builds, 2);
    assert_eq!(cache.curve_feature_builds(), 2);
}
