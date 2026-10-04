use super::*;
use viboceros_command::{CommandRegistry, mirror::MirrorPointPlane};

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn rect() -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))
}

#[test]
fn mirror_preview_reuses_samples_meshes_and_normals_across_mouse_moves_and_views() {
    let mut document = Document::default();
    let commands = CommandRegistry::with_builtins();
    for command in [
        "Point 3,3,0",
        "Line 2,0 4,0",
        "Circle 3,3 1",
        "Curve 2,0 2,2 4,2 4,0 Degree=3",
        "SrfPt 2,-2 4,-2 4,-4,1 2,-4",
        "Box 2,-5 4,-3 2",
        "MeshBox 5,-5 7,-3 2",
    ] {
        commands.execute(&mut document, command).unwrap();
    }
    let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
    document
        .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let history = document.undo_label();
    let mut views = Viewport::standard_views();
    let mut samples = Vec::new();
    for (i, view) in views.iter_mut().enumerate() {
        view.display_mode = DisplayMode::Shaded;
        for (j, cursor) in [point(0., 5., 0.), point(1., 4., 0.), point(-1., 3., 0.)]
            .into_iter()
            .enumerate()
        {
            let preview = MirrorPreview {
                grips: &[],
                sources: &ids,
                copy: true,
                plane: MirrorPointPlane::TwoPoint {
                    start: point(0., 0., 0.),
                },
                last_transform: None,
            }
            .resolve(Some(cursor), WorldPlane::Top.frame(), document.tolerance())
            .0;
            let current = view.object_scene_with_transform(rect(), &document, None, &[], preview);
            let cached = view.cached_scene.borrow();
            let objects = &cached.as_ref().unwrap().key.objects;
            assert_eq!(objects.len(), ids.len() * 2);
            for (index, original) in objects.iter().filter(|o| o.transform.is_none()).enumerate() {
                let reflected = &objects[ids.len() + index];
                assert!(Rc::ptr_eq(&original.geometry, &reflected.geometry));
                assert!(!original.draw_faces);
                assert!(reflected.draw_faces);
                assert_eq!(reflected.color, SELECTED_COLOR);
                let geometry = &original.geometry;
                let sample = (
                    Rc::as_ptr(geometry),
                    geometry.wires().as_ptr(),
                    geometry.mesh().map(|mesh| mesh.vertices().as_ptr()),
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
                &current,
                &view.object_scene_with_transform(rect(), &document, None, &[], preview)
            ));
        }
    }
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(document.selected_object_count(), ids.len());
    assert_eq!(document.undo_label(), history);
}

#[test]
fn mirror_preview_shaded_faces_keep_object_color_and_transform_normals() {
    let mut document = Document::default();
    let mesh = TriangleMesh::try_new(
        vec![point(2., -1., 0.), point(4., -1., 2.), point(2., 1., 0.)],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap();
    let color = ColorRgb::new(200, 80, 60);
    let attrs = ObjectAttributes::on_layer(document.current_layer_id()).with_object_color(color);
    let id = document
        .add_geometry_with_attributes(Geometry::Mesh(mesh.clone()), attrs)
        .unwrap();
    document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let mut view = Viewport::new(ViewKind::Top);
    view.display_mode = DisplayMode::Shaded;
    let map = MirrorPointPlane::TwoPoint {
        start: point(0., 0., 0.),
    }
    .reflection_at(
        WorldPlane::Top.frame(),
        point(0., 5., 0.),
        document.tolerance(),
    )
    .unwrap();
    let expected_mesh = mesh.transformed(map, document.tolerance()).unwrap();
    let mut builder = GpuSceneBuilder::new();
    view.add_gpu_mesh_faces(&mut builder, &expected_mesh, Color32::from_rgb(200, 80, 60));
    let expected = builder.finish(&view, rect(), false);
    let actual = view.object_scene_with_transform(
        rect(),
        &document,
        None,
        &[],
        Some(TransformedObjects::reflection(&[id], true, map)),
    );
    assert_eq!(actual.triangles.len(), 3); // Original faces are suppressed.
    for vertex in &actual.triangles {
        let expected = expected
            .triangles
            .iter()
            .find(|v| v.position[..2] == vertex.position[..2])
            .unwrap();
        assert_eq!(vertex.normal, expected.normal);
        assert_eq!(vertex.color, expected.color);
    }
    assert_eq!(actual.lines.len(), 6); // Gray source wires and selected reflected wires.
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
}

#[test]
fn mirror_preview_clipping_includes_reflected_bounds_and_restores_document_bounds() {
    let mut document = Document::default();
    let id = document
        .add_geometry(Geometry::Point(point(0., 0., 8.)))
        .unwrap();
    let mut view = Viewport::new(ViewKind::Top);
    let map = MirrorPointPlane::ThreePoint {
        origin: point(0., 0., 0.),
        x: point(1., 0., 0.),
    }
    .reflection_at(
        WorldPlane::Top.frame(),
        point(0., 1., 0.),
        document.tolerance(),
    )
    .unwrap();
    let sources = [id];
    let preview = Some(TransformedObjects::reflection(&sources, true, map));
    view.refresh_clipping(&document, rect()).unwrap();
    let original = (view.frustum_near, view.frustum_far);
    let before = view.object_scene_with_transform(rect(), &document, None, &[], preview);
    // Top looks down world Z: its encoded GPU depth is negative model Z.
    let reflected_depth = -before.points[1].position_size[2];
    assert!(reflected_depth > before.uniform.clip_depth[1]);
    view.refresh_clipping_with_transform(&document, rect(), preview)
        .unwrap();
    let after = view.object_scene_with_transform(rect(), &document, None, &[], preview);
    assert_eq!(after.points.len(), 2);
    for point in &after.points {
        let depth = -point.position_size[2];
        assert!(depth >= after.uniform.clip_depth[0] && depth <= after.uniform.clip_depth[1]);
    }
    assert!(view.frustum_far > original.1);
    view.refresh_clipping(&document, rect()).unwrap();
    assert!((view.frustum_far - original.1).abs() < 1e-3);
    assert!(view.view_undo.is_empty());
    assert_eq!(document.objects().len(), 1);
}

#[test]
#[ignore = "requires a graphics adapter; run explicitly with --ignored --nocapture"]
fn gpu_mirror_preview_renders_selected_reflections_source_wires_and_display_modes() {
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
                for three_point in [false, true] {
                    let mut view = Viewport::new(ViewKind::Top);
                    view.pixels_per_unit = 20.;
                    view.display_mode = mode;
                    let plane = if three_point {
                        MirrorPointPlane::ThreePoint {
                            origin: point(0., 0., 0.),
                            x: point(0., 0., 3.),
                        }
                    } else {
                        MirrorPointPlane::TwoPoint {
                            start: point(0., 0., 0.),
                        }
                    };
                    let input = MirrorPreview {
                        grips: &[],
                        plane,
                        copy,
                        sources: &[id, face],
                        last_transform: None,
                    };
                    let (preview, _) = input.resolve(
                        Some(point(0., 5., 0.)),
                        WorldPlane::Top.frame(),
                        document.tolerance(),
                    );
                    view.refresh_clipping_with_transform(&document, rect, preview)
                        .unwrap();
                    let scene =
                        view.object_scene_with_transform(rect, &document, None, &[], preview);
                    let pixels = renderer.render_cached(&scene);
                    let pixel_at = |point| {
                        let pixel = view.project(point, rect).unwrap();
                        pixels[pixel.y.round() as usize * SIZE as usize + pixel.x.round() as usize]
                    };
                    let selected = pixel_at(point(-3., 3., 0.));
                    assert!(selected[0] > selected[1] && selected[1] > selected[2] + 20);
                    let source = pixel_at(point(3., 3., 0.));
                    if !copy || three_point {
                        assert_eq!(source[0], source[1]);
                        assert_eq!(source[1], source[2]);
                    } else {
                        assert!(source[0] > source[1] + 20);
                    }
                    assert_eq!(pixel_at(point(3., -1. / 3., 0.))[3], 0); // Source faces are wire-only.
                    let reflected = pixel_at(point(-3., -1. / 3., 0.));
                    if mode == DisplayMode::Wireframe {
                        assert_eq!(reflected[3], 0);
                    } else {
                        assert!(reflected[3] > 0);
                        assert!(reflected[0] > reflected[1]);
                    }
                }
            }
        }
    }
}
