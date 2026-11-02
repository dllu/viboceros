use super::*;
use crate::viewport::preview_test_support::{frame, point};
use viboceros_command::CommandRegistry;
fn setup() -> (Document, CommandRegistry, ObjectId) {
    let mut doc = Document::default();
    let r = CommandRegistry::with_builtins();
    r.execute(&mut doc, "SrfPt -2,-2,0 2,-2,0 2,2,0 -2,2,0")
        .unwrap();
    let id = doc.objects().next().unwrap().id();
    r.execute(&mut doc, "SelAll").unwrap();
    r.execute(&mut doc, "CurvatureAnalysis Range=-1,1").unwrap();
    (doc, r, id)
}

#[test]
fn curvature_faces_pick_in_wireframe_cache_colors_and_keep_model_unchanged() {
    let (mut doc, r, id) = setup();
    let original = doc.object(id).unwrap().geometry_snapshot().clone();
    let context = egui::Context::default();
    for kind in [
        ViewKind::Top,
        ViewKind::Perspective,
        ViewKind::Front,
        ViewKind::Right,
    ] {
        for mode in [
            DisplayMode::Wireframe,
            DisplayMode::Shaded,
            DisplayMode::Ghosted,
        ] {
            let mut view = Viewport::new(kind);
            view.display_mode = mode;
            let analysis = r.curvature_analysis_view(&doc).unwrap();
            frame(
                &context,
                &mut view,
                &doc,
                ViewportInput {
                    curvature_analysis: Some(&analysis),
                    ..Default::default()
                },
                vec![],
            );
            let rect = view.last_rect.unwrap();
            let build = || {
                view.object_scene_with_analyses(
                    rect,
                    &doc,
                    None,
                    &[],
                    None,
                    scene::SurfaceAnalyses {
                        draft: None,
                        curvature: Some(&analysis),
                    },
                )
            };
            let first = build();
            assert!(!first.triangles.is_empty());
            assert!(first.lines.is_empty());
            assert!(
                first
                    .triangles
                    .iter()
                    .all(|v| v.color == [0., 1., 0., 1.] && v.shading == 0.)
            );
            assert!(std::sync::Arc::ptr_eq(&first, &build()));
            if matches!(kind, ViewKind::Top | ViewKind::Perspective) {
                assert_eq!(
                    view.pick_object(view.project(point(0.3, 0.7, 0.), rect).unwrap(), rect, &doc),
                    Some(id)
                );
            }
        }
    }
    r.execute(&mut doc, "CurvatureAnalysis ShowIsoCurves=Yes")
        .unwrap();
    let analysis = r.curvature_analysis_view(&doc).unwrap();
    let view = Viewport::new(ViewKind::Top);
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(256.));
    let scene = view.object_scene_with_analyses(
        rect,
        &doc,
        None,
        &[],
        None,
        scene::SurfaceAnalyses {
            curvature: Some(&analysis),
            ..Default::default()
        },
    );
    assert!(!scene.lines.is_empty());
    assert!(
        doc.object(id)
            .unwrap()
            .geometry_snapshot()
            .shares_storage_with(&original)
    );
}

#[test]
#[ignore = "requires a graphics adapter; offscreen Vulkan without a window"]
fn curvature_colors_are_unlit_and_opaque_for_every_style_and_display_mode() {
    use crate::viewport_gpu::readback::{OffscreenRenderer, SIZE};
    let (mut doc, r, _) = setup();
    r.execute(&mut doc, "Line -1,0,-1 1,0,-1").unwrap();
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(SIZE as f32));
    for format in [
        eframe::wgpu::TextureFormat::Rgba8Unorm,
        eframe::wgpu::TextureFormat::Rgba8UnormSrgb,
    ] {
        let mut renderer = OffscreenRenderer::new(format);
        for (style, range, color) in [
            ("Gaussian", "-1,1", [0, 255, 0, 255]),
            ("Mean", "0,1", [0, 0, 255, 255]),
            ("MinRadius", "0,1", [255, 0, 0, 255]),
            ("MaxRadius", "0,1", [255, 0, 0, 255]),
        ] {
            r.execute(
                &mut doc,
                &format!("CurvatureAnalysis Style={style} Range={range}"),
            )
            .unwrap();
            let analysis = r.curvature_analysis_view(&doc).unwrap();
            for mode in [
                DisplayMode::Wireframe,
                DisplayMode::Shaded,
                DisplayMode::Ghosted,
            ] {
                let mut view = Viewport::new(ViewKind::Top);
                view.display_mode = mode;
                let scene = view.object_scene_with_analyses(
                    rect,
                    &doc,
                    None,
                    &[],
                    None,
                    scene::SurfaceAnalyses {
                        curvature: Some(&analysis),
                        ..Default::default()
                    },
                );
                let pixels = renderer.render(&scene);
                assert_eq!(
                    pixels[(SIZE / 2 * SIZE + SIZE / 2) as usize],
                    color,
                    "{style} {format:?} {mode:?}"
                );
            }
        }
    }
}
