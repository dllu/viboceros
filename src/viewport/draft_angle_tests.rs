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
    r.execute(&mut doc, "DraftAngleAnalysis").unwrap();
    (doc, r, id)
}

#[test]
fn faces_draw_and_pick_in_wireframe_and_settings_invalidate_scene_cache() {
    let (mut doc, r, id) = setup();
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
            let a = r.draft_angle_analysis_view(&doc).unwrap();
            frame(
                &context,
                &mut view,
                &doc,
                ViewportInput {
                    draft_angle_analysis: Some(&a),
                    ..Default::default()
                },
                vec![],
            );
            let rect = view.last_rect.unwrap();
            let build = |v: &Viewport, analysis| {
                v.object_scene_with_analysis(rect, &doc, None, &[], None, analysis)
            };
            let first = build(&view, Some(&a));
            assert!(!first.triangles.is_empty(), "{kind:?} {mode:?}");
            assert!(first.lines.is_empty());
            assert!(
                first
                    .triangles
                    .iter()
                    .all(|v| v.color == [0., 0., 1., 1.] && v.shading == 0.)
            );
            assert!(std::sync::Arc::ptr_eq(&first, &build(&view, Some(&a))));
            if matches!(kind, ViewKind::Top | ViewKind::Perspective) {
                let pointer = view.project(point(0.3, 0.7, 0.), rect).unwrap();
                assert_eq!(view.pick_object(pointer, rect, &doc), Some(id));
            }
            r.execute(&mut doc, "DraftAngleAnalysis ShowIsoCurves=Yes")
                .unwrap();
            let b = r.draft_angle_analysis_view(&doc).unwrap();
            let changed = view.object_scene_with_analysis(rect, &doc, None, &[], None, Some(&b));
            assert!(!std::sync::Arc::ptr_eq(&first, &changed));
            assert!(!changed.lines.is_empty());
            r.execute(&mut doc, "DraftAngleAnalysis ShowIsoCurves=No")
                .unwrap();
        }
    }
}

#[test]
fn flip_and_undo_refresh_colors() {
    let (mut doc, r, _) = setup();
    let view = Viewport::new(ViewKind::Top);
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(256.));
    let analysis = r.draft_angle_analysis_view(&doc).unwrap();
    let before = view.object_scene_with_analysis(rect, &doc, None, &[], None, Some(&analysis));
    r.execute(&mut doc, "SelAll").unwrap();
    r.execute(&mut doc, "Flip").unwrap();
    let flipped = view.object_scene_with_analysis(rect, &doc, None, &[], None, Some(&analysis));
    assert!(
        flipped
            .triangles
            .iter()
            .all(|v| v.color == [1., 0., 0., 1.])
    );
    r.execute(&mut doc, "Undo").unwrap();
    let restored = view.object_scene_with_analysis(rect, &doc, None, &[], None, Some(&analysis));
    assert_eq!(restored.triangles.len(), before.triangles.len());
    assert!(
        restored
            .triangles
            .iter()
            .all(|v| v.color == [0., 0., 1., 1.])
    );
}

#[test]
fn mesh_and_subd_analysis_override_material_colors_without_model_edits() {
    let mesh = TriangleMesh::try_new_faces(
        vec![
            point(-2., -2., 0.),
            point(2., -2., 0.),
            point(2., 2., 0.),
            point(-2., 2., 0.),
        ],
        vec![viboceros_geometry::MeshFace::Quad([0, 1, 2, 3])],
        Tolerance::DEFAULT,
    )
    .unwrap()
    .try_with_vertex_colors(Some(vec![[255, 0, 255, 0]; 4]))
    .unwrap();
    let subd =
        viboceros_geometry::SubD::try_from_mesh(&mesh, Default::default(), Tolerance::DEFAULT)
            .unwrap();
    for source in [Geometry::Mesh(mesh), Geometry::SubD(subd)] {
        let mut doc = Document::default();
        let r = CommandRegistry::with_builtins();
        let id = doc.add_geometry(source.clone()).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        r.execute(&mut doc, "DraftAngleAnalysis").unwrap();
        r.execute(&mut doc, "DraftAngleAnalysis ShowIsoCurves=No")
            .unwrap();
        let analysis = r.draft_angle_analysis_view(&doc).unwrap();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(256.));
        let view = Viewport::new(ViewKind::Top);
        let scene = view.object_scene_with_analysis(rect, &doc, None, &[], None, Some(&analysis));
        assert!(!scene.triangles.is_empty());
        assert!(scene.lines.is_empty());
        assert!(
            scene
                .triangles
                .iter()
                .all(|v| v.color == [0., 0., 1., 1.] && v.shading == 0.)
        );
        assert_eq!(doc.object(id).unwrap().geometry(), &source);
    }
}

#[test]
#[ignore = "requires a graphics adapter; headless offscreen rendering"]
fn false_colors_are_unlit_and_opaque_in_all_modes_and_target_formats() {
    use crate::viewport_gpu::readback::{OffscreenRenderer, SIZE};
    let (mut doc, r, _) = setup();
    // This line must be occluded by analysis faces, including in Ghosted.
    r.execute(&mut doc, "Line -1,0,-1 1,0,-1").unwrap();
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(SIZE as f32));
    for format in [
        eframe::wgpu::TextureFormat::Rgba8Unorm,
        eframe::wgpu::TextureFormat::Rgba8UnormSrgb,
    ] {
        let mut renderer = OffscreenRenderer::new(format);
        for (up, color) in [("0,0,1", [0, 0, 255, 255]), ("0,0,-1", [255, 0, 0, 255])] {
            r.execute(&mut doc, &format!("DraftAngleAnalysis Up={up}"))
                .unwrap();
            let analysis = r.draft_angle_analysis_view(&doc).unwrap();
            for mode in [
                DisplayMode::Wireframe,
                DisplayMode::Shaded,
                DisplayMode::Ghosted,
            ] {
                let mut view = Viewport::new(ViewKind::Top);
                view.display_mode = mode;
                let scene =
                    view.object_scene_with_analysis(rect, &doc, None, &[], None, Some(&analysis));
                let pixels = renderer.render(&scene);
                let offset = (SIZE / 2 * SIZE + SIZE / 2) as usize;
                assert_eq!(pixels[offset], color, "{format:?} {mode:?}");
            }
        }
    }
}
