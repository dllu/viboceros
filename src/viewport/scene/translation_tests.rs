use super::*;
use viboceros_command::CommandRegistry;

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn rect() -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))
}

#[test]
fn translation_preview_shares_display_geometry_across_motion_and_views() {
    let mut document = Document::default();
    let commands = CommandRegistry::with_builtins();
    for input in [
        "Point 3,3,0",
        "Circle 3,3 1",
        "Curve 2,0 2,2 4,2 4,0 Degree=3",
        "SrfPt 2,-2 4,-2 4,-4,1 2,-4",
        "Box 2,-5 4,-3 2",
        "MeshBox 5,-5 7,-3 2",
    ] {
        commands.execute(&mut document, input).unwrap();
    }
    let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
    document
        .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let history = document.undo_label();
    let mut samples = Vec::new();
    for (i, view) in Viewport::standard_views().iter_mut().enumerate() {
        view.display_mode = DisplayMode::Shaded;
        for (j, destination) in [point(-6., 2., 0.), point(-5., 3., 1.), point(-6., 2., 0.)]
            .into_iter()
            .enumerate()
        {
            let preview = TranslationPreview {
                sources: &ids,
                base: point(0., 0., 0.),
                copy: true,
                reference: None,
                last_transform: None,
            }
            .resolve(Some(destination))
            .0;
            let scene = view.object_scene_with_transform(rect(), &document, None, &[], preview);
            let cached = view.cached_scene.borrow();
            let objects = &cached.as_ref().unwrap().key.objects;
            assert_eq!(objects.len(), ids.len() * 2);
            for (index, original) in objects[..ids.len()].iter().enumerate() {
                let target = &objects[ids.len() + index];
                assert!(Rc::ptr_eq(&original.geometry, &target.geometry));
                assert!(!original.draw_faces);
                assert!(target.draw_faces);
                assert!(!target.reversing);
                assert_eq!(target.color, SELECTED_COLOR);
                let geometry = &original.geometry;
                let sample = (
                    Rc::as_ptr(geometry),
                    geometry.wires().as_ptr(),
                    geometry.mesh().map(|m| m.vertices().as_ptr()),
                    geometry.normals().as_ptr(),
                );
                if i == 0 && j == 0 {
                    samples.push(sample);
                } else {
                    assert_eq!(sample, samples[index]);
                }
            }
            drop(cached);
            assert!(Arc::ptr_eq(
                &scene,
                &view.object_scene_with_transform(rect(), &document, None, &[], preview)
            ));
        }
    }
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(document.undo_label(), history);
}

#[test]
fn translation_preview_preserves_mesh_normals_colors_and_committed_copy_faces() {
    let mut document = Document::default();
    let mesh = TriangleMesh::try_new(
        vec![point(2., -1., 0.), point(4., -1., 2.), point(2., 1., 0.)],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap();
    let attrs = ObjectAttributes::on_layer(document.current_layer_id())
        .with_object_color(ColorRgb::new(200, 80, 60));
    let id = document
        .add_geometry_with_attributes(Geometry::Mesh(mesh.clone()), attrs.clone())
        .unwrap();
    document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let map = AffineTransform3::from_translation(Vector3::try_new(-6., 2., 4.).unwrap());
    let mut view = Viewport::new(ViewKind::Top);
    view.display_mode = DisplayMode::Shaded;
    let sources = [id];
    let preview = TranslationPreview {
        sources: &sources,
        base: point(0., 0., 0.),
        copy: false,
        reference: None,
        last_transform: Some(map),
    }
    .resolve(None)
    .0;
    view.refresh_clipping_with_transform(&document, rect(), preview)
        .unwrap();
    let mut builder = GpuSceneBuilder::new();
    view.add_gpu_mesh_faces(
        &mut builder,
        &mesh.transformed(map, document.tolerance()).unwrap(),
        Color32::from_rgb(200, 80, 60),
    );
    let expected = builder.finish(&view, rect(), false);
    let actual = view.object_scene_with_transform(rect(), &document, None, &[], preview);
    assert_eq!(actual.triangles.len(), 3);
    for vertex in &actual.triangles {
        let expected = expected
            .triangles
            .iter()
            .find(|v| v.position[..2] == vertex.position[..2])
            .unwrap();
        assert_eq!(vertex.normal, expected.normal);
        assert_eq!(vertex.color, expected.color);
    }
    assert!(
        actual.lines[..3]
            .iter()
            .all(|line| line.color == color_to_gpu(LOCKED_COLOR))
    );
    assert!(
        actual.lines[3..]
            .iter()
            .all(|line| line.color == color_to_gpu(SELECTED_COLOR))
    );
    document
        .add_geometry_with_attributes(
            Geometry::Mesh(
                mesh.transformed(
                    AffineTransform3::from_translation(Vector3::try_new(-2., 0., 0.).unwrap()),
                    document.tolerance(),
                )
                .unwrap(),
            ),
            attrs,
        )
        .unwrap();
    let actual = view.object_scene_with_transform(rect(), &document, None, &[], preview);
    assert_eq!(actual.triangles.len(), 6); // Prior copies are real shaded geometry.
}

#[test]
fn translation_preview_expands_clipping_and_restores_it_without_view_history() {
    let mut document = Document::default();
    let id = document
        .add_geometry(Geometry::Point(point(0., 0., 8.)))
        .unwrap();
    let mut view = Viewport::new(ViewKind::Top);
    view.refresh_clipping(&document, rect()).unwrap();
    let original = (view.frustum_near, view.frustum_far);
    let sources = [id];
    let preview = TranslationPreview {
        sources: &sources,
        base: point(0., 0., 0.),
        copy: false,
        reference: None,
        last_transform: None,
    }
    .resolve(Some(point(0., 0., -16.)))
    .0;
    view.refresh_clipping_with_transform(&document, rect(), preview)
        .unwrap();
    let scene = view.object_scene_with_transform(rect(), &document, None, &[], preview);
    assert_eq!(scene.points.len(), 2);
    for point in &scene.points {
        let depth = -point.position_size[2];
        assert!(depth >= scene.uniform.clip_depth[0] && depth <= scene.uniform.clip_depth[1]);
    }
    assert!(view.frustum_far > original.1);
    view.refresh_clipping(&document, rect()).unwrap();
    assert!((view.frustum_far - original.1).abs() < 1e-3);
    assert!(view.view_undo.is_empty());
}

#[test]
fn translation_preview_highlights_normal_reference_without_selecting_or_transforming_it() {
    let mut document = Document::default();
    let commands = CommandRegistry::with_builtins();
    commands.execute(&mut document, "Circle 0,0 3").unwrap();
    let reference = document.objects().next().unwrap().id();
    let id = document
        .add_geometry(Geometry::Point(point(3., 3., 0.)))
        .unwrap();
    document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let view = Viewport::new(ViewKind::Top);
    let sources = [id];
    let preview = TranslationPreview {
        sources: &sources,
        base: point(3., 0., 0.),
        copy: false,
        reference: Some(reference),
        last_transform: None,
    }
    .resolve(Some(point(-6., 0., 0.)))
    .0;
    view.object_scene_with_transform(rect(), &document, None, &[], preview);
    let cached = view.cached_scene.borrow();
    let objects = &cached.as_ref().unwrap().key.objects;
    assert_eq!(objects[0].color, SELECTED_COLOR);
    assert!(objects[0].transform.is_none());
    assert_eq!(objects[1].color, LOCKED_COLOR);
    assert!(objects[1].transform.is_none());
    assert_eq!(objects[2].color, SELECTED_COLOR);
    assert!(objects[2].transform.is_some());
    assert!(!document.is_selected(reference));
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
#[ignore = "requires a graphics adapter; run explicitly with --ignored --nocapture"]
fn gpu_translation_preview_renders_source_wires_target_points_and_material_faces() {
    use crate::viewport_gpu::readback::{OffscreenRenderer, SIZE};
    use eframe::wgpu;
    let mut document = Document::default();
    let attrs = ObjectAttributes::on_layer(document.current_layer_id())
        .with_object_color(ColorRgb::new(200, 80, 60));
    let id = document
        .add_geometry_with_attributes(Geometry::Point(point(3., 3., 0.)), attrs.clone())
        .unwrap();
    let mesh = TriangleMesh::try_new(
        vec![point(2., -1., 0.), point(4., -1., 0.), point(3., 1., 0.)],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap();
    let face = document
        .add_geometry_with_attributes(Geometry::Mesh(mesh), attrs)
        .unwrap();
    document
        .select_objects_direct([id, face], SelectionMode::Replace)
        .unwrap();
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(SIZE as f32));
    for format in [
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ] {
        let mut renderer = OffscreenRenderer::new(format);
        for mode in [
            DisplayMode::Wireframe,
            DisplayMode::Shaded,
            DisplayMode::Ghosted,
        ] {
            for copy in [false, true] {
                let mut view = Viewport::new(ViewKind::Top);
                view.pixels_per_unit = 20.;
                view.display_mode = mode;
                let sources = [id, face];
                let preview = TranslationPreview {
                    sources: &sources,
                    base: point(0., 0., 0.),
                    copy,
                    reference: None,
                    last_transform: None,
                }
                .resolve(Some(point(-6., 2., 0.)))
                .0;
                view.refresh_clipping_with_transform(&document, rect, preview)
                    .unwrap();
                let scene = view.object_scene_with_transform(rect, &document, None, &[], preview);
                let pixels = renderer.render_cached(&scene);
                let pixel_at = |point| {
                    let pixel = view.project(point, rect).unwrap();
                    pixels[pixel.y.round() as usize * SIZE as usize + pixel.x.round() as usize]
                };
                let selected = pixel_at(point(-3., 5., 0.));
                assert!(selected[0] > selected[1] && selected[1] > selected[2] + 20);
                let source = pixel_at(point(3., 3., 0.));
                if copy {
                    assert!(source[0] > source[1] + 20);
                } else {
                    assert_eq!(source[0], source[1]);
                    assert_eq!(source[1], source[2]);
                }
                assert_eq!(pixel_at(point(3., -1. / 3., 0.))[3], 0);
                let target = pixel_at(point(-3., 2. - 1. / 3., 0.));
                if mode == DisplayMode::Wireframe {
                    assert_eq!(target[3], 0);
                } else {
                    assert!(target[3] > 0);
                    assert!(target[0] > target[1]);
                }
                let uploads = renderer.preparations();
                renderer.render_cached(&view.object_scene_with_transform(
                    rect,
                    &document,
                    None,
                    &[],
                    preview,
                ));
                assert_eq!(renderer.preparations(), uploads);
            }
        }
    }
}
