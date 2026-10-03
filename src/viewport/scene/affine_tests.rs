use super::*;
use viboceros_command::{CommandRegistry, point_transform::PointTransform};

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn rect() -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))
}

#[test]
fn affine_preview_reuses_display_geometry_across_deformation_and_four_views() {
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
    let sources = document.objects().map(|o| o.id()).collect::<Vec<_>>();
    document
        .select_objects_direct(sources.iter().copied(), SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let history = document.undo_label();
    let mut samples = Vec::new();
    for (i, view) in Viewport::standard_views().iter_mut().enumerate() {
        view.display_mode = DisplayMode::Shaded;
        for (j, map) in [
            AffineTransform3::try_nonuniform_scale(point(0., 0., 0.), [2., 0.5, 3.]).unwrap(),
            AffineTransform3::try_new(
                [[1., 3., 0.], [0., 1., 0.], [0., 0., 1.]],
                Vector3::try_new(0., 0., 0.).unwrap(),
            )
            .unwrap(),
            AffineTransform3::try_nonuniform_scale(point(0., 0., 0.), [-2., 1., 1.]).unwrap(),
        ]
        .into_iter()
        .enumerate()
        {
            let preview = Some(TransformedObjects {
                sources: &sources,
                reference_sources: false,
                draw_source_faces: false,
                reversing: map.orientation_reversing().unwrap(),
                reference: None,
                transform: map,
            });
            let scene = view.object_scene_with_transform(rect(), &document, None, &[], preview);
            let cached = view.cached_scene.borrow();
            let objects = &cached.as_ref().unwrap().key.objects;
            assert_eq!(objects.len(), sources.len() * 2);
            for (k, original) in objects[..sources.len()].iter().enumerate() {
                let target = &objects[sources.len() + k];
                assert!(Rc::ptr_eq(&original.geometry, &target.geometry));
                assert!(!original.draw_faces);
                assert!(target.draw_faces);
                assert_eq!(target.color, SELECTED_COLOR);
                let g = &original.geometry;
                let sample = (
                    Rc::as_ptr(g),
                    g.wires().as_ptr(),
                    g.mesh().map(|m| m.vertices().as_ptr()),
                    g.normals().as_ptr(),
                );
                if i == 0 && j == 0 {
                    samples.push(sample);
                } else {
                    assert_eq!(sample, samples[k]);
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
fn affine_preview_face_normals_match_independently_deformed_meshes() {
    let mut document = Document::default();
    let mesh = TriangleMesh::try_new(
        vec![point(2., -1., 0.), point(4., -1., 2.), point(2., 1., 0.)],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap();
    let id = document
        .add_geometry_with_attributes(
            Geometry::Mesh(mesh.clone()),
            ObjectAttributes::on_layer(document.current_layer_id())
                .with_object_color(ColorRgb::new(200, 80, 60)),
        )
        .unwrap();
    document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let mut view = Viewport::new(ViewKind::Top);
    view.display_mode = DisplayMode::Shaded;
    for rows in [
        [[2., 0., 0.], [0., 0.5, 0.], [0., 0., 3.]],
        [[1., 3., 0.], [0., 1., 0.], [0., 0., 1.]],
        [[-2., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
    ] {
        let map = AffineTransform3::try_new(rows, Vector3::try_new(0., 0., 0.).unwrap()).unwrap();
        let preview = Some(TransformedObjects {
            sources: &[id],
            reference_sources: true,
            draw_source_faces: false,
            reversing: map.orientation_reversing().unwrap(),
            reference: None,
            transform: map,
        });
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
        for (a, b) in actual.triangles.iter().zip(&expected.triangles) {
            assert_eq!(a.position, b.position);
            assert_eq!(a.color, b.color);
            for (a, b) in a.normal.iter().zip(b.normal) {
                assert!((a - b).abs() < 2e-7, "{a} != {b}");
            }
        }
        assert!(
            actual.lines[..3]
                .iter()
                .all(|l| l.color == color_to_gpu(LOCKED_COLOR))
        );
        assert!(
            actual.lines[3..]
                .iter()
                .all(|l| l.color == color_to_gpu(SELECTED_COLOR))
        );
    }
}

#[test]
#[ignore = "requires a graphics adapter; run explicitly with --ignored --nocapture"]
fn gpu_affine_preview_renders_deformed_faces_points_and_source_styles() {
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
                for (definition, target) in [
                    (
                        PointTransform::Scale1DDirection {
                            center: point(0., 0., 0.),
                            factor: -1.,
                        },
                        point(4., 0., 0.),
                    ),
                    (
                        PointTransform::Scale2D {
                            center: point(15., 0., 0.),
                            reference: point(17., 0., 0.),
                        },
                        point(18., 0., 0.),
                    ),
                ] {
                    let mut view = Viewport::new(ViewKind::Top);
                    view.pixels_per_unit = 20.;
                    view.display_mode = mode;
                    let sources = [id, face];
                    let preview = AffinePreview {
                        sources: &sources,
                        definition,
                        frame: None,
                        copy,
                        last_transform: None,
                    }
                    .resolve(
                        Some(target),
                        view.construction_plane(),
                        document.tolerance(),
                    )
                    .0;
                    let map = preview.unwrap().transform;
                    view.refresh_clipping_with_transform(&document, rect, preview)
                        .unwrap();
                    let scene =
                        view.object_scene_with_transform(rect, &document, None, &[], preview);
                    let pixels = renderer.render_cached(&scene);
                    let pixel_at = |p| {
                        let p = view.project(p, rect).unwrap();
                        pixels[p.y.round() as usize * SIZE as usize + p.x.round() as usize]
                    };
                    let selected = pixel_at(map.transform_point(point(3., 3., 0.)).unwrap());
                    assert!(selected[0] > selected[1] && selected[1] > selected[2] + 20);
                    let source = pixel_at(point(3., 3., 0.));
                    if copy {
                        assert!(source[0] > source[1] + 20);
                    } else {
                        assert_eq!(source[0], source[1]);
                        assert_eq!(source[1], source[2]);
                    }
                    assert_eq!(pixel_at(point(3., -1. / 3., 0.))[3], 0);
                    let target = pixel_at(map.transform_point(point(3., -1. / 3., 0.)).unwrap());
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
}
