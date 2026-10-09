use super::*;
use viboceros_document::{
    BlockContent, BlockDefinitionId, BlockMember, BlockReference, LayerId, ObjectColorSource,
};
use viboceros_geometry::LineSegment;

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn rect() -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))
}

fn fixture() -> (Document, ObjectId, BlockDefinitionId, LayerId) {
    let mut document = Document::default();
    let layer = document
        .add_layer("point members", ColorRgb::new(50, 60, 70))
        .unwrap();
    let members = vec![
        BlockMember::new(
            BlockContent::Geometry(Geometry::Point(point(-2., 0., 0.)).into()),
            ObjectAttributes::on_layer(layer).with_color_source(ObjectColorSource::Parent),
        ),
        BlockMember::new(
            BlockContent::Geometry(
                Geometry::Line(
                    LineSegment::try_new(
                        point(-1., -1., 0.),
                        point(1., -1., 0.),
                        document.tolerance(),
                    )
                    .unwrap(),
                )
                .into(),
            ),
            ObjectAttributes::on_layer(document.current_layer_id())
                .with_object_color(ColorRgb::new(10, 40, 200)),
        ),
        BlockMember::new(
            BlockContent::Geometry(
                Geometry::Mesh(
                    TriangleMesh::try_new(
                        vec![point(0., 1., 0.), point(2., 1., 0.), point(1., 3., 0.)],
                        vec![[0, 1, 2]],
                        document.tolerance(),
                    )
                    .unwrap(),
                )
                .into(),
            ),
            ObjectAttributes::on_layer(document.current_layer_id())
                .with_object_color(ColorRgb::new(20, 180, 60)),
        ),
    ];
    let definition = document.add_block_definition("part", members).unwrap();
    let placement = BlockReference::try_new(
        definition,
        AffineTransform3::from_translation(Vector3::try_new(3., 2., 0.).unwrap()),
    )
    .unwrap();
    let object = document
        .add_block_instance_with_attributes(
            placement,
            ObjectAttributes::on_layer(document.current_layer_id())
                .with_object_color(ColorRgb::new(210, 30, 40)),
        )
        .unwrap();
    (document, object, definition, layer)
}

fn independent(document: &Document, object: ObjectId) -> Document {
    let root = document.object(object).unwrap();
    let Geometry::BlockInstance(instance) = root.geometry() else {
        panic!()
    };
    let mut flat = Document::default();
    for member in instance.members() {
        let display = document
            .block_member_display(root.attributes(), &member.path)
            .unwrap();
        let attributes = member
            .attributes
            .clone()
            .with_layer(flat.current_layer_id())
            .with_object_color(display.color);
        flat.add_geometry_with_attributes((*member.geometry).clone(), attributes)
            .unwrap();
    }
    flat
}

fn assert_scene_equal(left: &GpuViewportScene, right: &GpuViewportScene) {
    assert_eq!(
        bytemuck::cast_slice::<_, u8>(&left.lines),
        bytemuck::cast_slice::<_, u8>(&right.lines)
    );
    assert_eq!(
        bytemuck::cast_slice::<_, u8>(&left.points),
        bytemuck::cast_slice::<_, u8>(&right.points)
    );
    assert_eq!(
        bytemuck::cast_slice::<_, u8>(&left.triangles),
        bytemuck::cast_slice::<_, u8>(&right.triangles)
    );
    assert_eq!(left.overlay_line_start, right.overlay_line_start);
    assert_eq!(left.overlay_point_start, right.overlay_point_start);
}

#[test]
fn mixed_block_members_render_like_placed_geometry_in_every_view_and_display_mode() {
    let (mut document, object, _, _) = fixture();
    let mut flat = independent(&document, object);
    for selected in [false, true] {
        if selected {
            document
                .select_object(object, SelectionMode::Replace)
                .unwrap();
            flat.select_all();
        }
        for mode in [
            DisplayMode::Wireframe,
            DisplayMode::Shaded,
            DisplayMode::Ghosted,
        ] {
            for kind in [
                ViewKind::Top,
                ViewKind::Front,
                ViewKind::Plan,
                ViewKind::Perspective,
            ] {
                let mut view = Viewport::new(kind);
                view.display_mode = mode;
                let scene = view.object_scene(rect(), &document);
                let mut reference_view = Viewport::new(kind);
                reference_view.display_mode = mode;
                assert_scene_equal(&scene, &reference_view.object_scene(rect(), &flat));
                assert!(Arc::ptr_eq(&scene, &view.object_scene(rect(), &document)));
            }
        }
    }
    assert_eq!(document.objects().len(), 1);
}

#[test]
fn click_and_window_selection_return_root_identity_and_respect_member_layer_visibility() {
    let (mut document, object, _, layer) = fixture();
    let view = Viewport::new(ViewKind::Top);
    let pixel = view.project(point(1., 2., 0.), rect()).unwrap();
    assert_eq!(view.pick_object(pixel, rect(), &document), Some(object));
    let small = Rect::from_center_size(pixel, Vec2::splat(3.));
    assert_eq!(
        view.objects_in_selection(rect(), small, true, &document),
        [object]
    );
    assert!(
        view.objects_in_selection(rect(), small, false, &document)
            .is_empty()
    );
    assert_eq!(view.object_scene(rect(), &document).points.len(), 1);
    document.set_layer_visibility(layer, false).unwrap();
    assert!(view.pick_object(pixel, rect(), &document).is_none());
    assert!(
        view.objects_in_selection(rect(), small, true, &document)
            .is_empty()
    );
    assert!(view.object_scene(rect(), &document).points.is_empty());
    document.undo().unwrap();
    assert_eq!(view.pick_object(pixel, rect(), &document), Some(object));
}

#[test]
fn definition_and_parent_color_edits_invalidate_scene_and_undo_restores_geometry() {
    let (mut document, object, definition, _) = fixture();
    let view = Viewport::new(ViewKind::Top);
    let before = view.object_scene(rect(), &document);
    document
        .set_objects_color([object], Some(ColorRgb::new(30, 180, 240)))
        .unwrap();
    let recolored = view.object_scene(rect(), &document);
    assert_ne!(before.points[0].color, recolored.points[0].color);
    document.undo().unwrap();
    assert_scene_equal(&before, &view.object_scene(rect(), &document));
    let mut members = document
        .block_definition(definition)
        .unwrap()
        .members()
        .to_vec();
    let attributes = members[0].attributes().clone();
    members[0] = BlockMember::new(
        BlockContent::Geometry(Geometry::Point(point(-4., 0., 0.)).into()),
        attributes,
    );
    document
        .replace_block_definition_members(definition, members)
        .unwrap();
    let changed = view.object_scene(rect(), &document);
    assert_ne!(
        before.points[0].position_size,
        changed.points[0].position_size
    );
    document.undo().unwrap();
    assert_scene_equal(&before, &view.object_scene(rect(), &document));
    document.redo().unwrap();
    assert_scene_equal(&changed, &view.object_scene(rect(), &document));
}
