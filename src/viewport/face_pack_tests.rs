use super::*;
use crate::viewport::preview_test_support::point;
use viboceros_command::{CommandRegistry, to_nurbs::FacePackPreview};
use viboceros_geometry::SubD;
#[test]
fn face_pack_colors_follow_source_faces_in_all_display_modes_and_cache_scenes() {
    let mut doc = Document::default();
    let source = SubD::try_from_polygons(
        vec![
            point(0., 0., 0.),
            point(2., 0., 1.),
            point(4., 0., 0.),
            point(0., 3., 0.),
            point(2., 3., 2.),
            point(4., 3., 0.),
        ],
        vec![vec![0, 1, 4, 3], vec![1, 2, 5, 4]],
    )
    .unwrap();
    let edge = source
        .edges()
        .iter()
        .position(|e| [e[0].min(e[1]), e[0].max(e[1])] == [1, 4])
        .unwrap();
    let source = source.try_with_edge_tags(&[(edge, 2)]).unwrap();
    let id = doc.add_geometry(Geometry::SubD(source)).unwrap();
    doc.select_object(id, viboceros_document::SelectionMode::Replace)
        .unwrap();
    let original = doc.object(id).unwrap().geometry_snapshot().clone();
    let commands = CommandRegistry::with_builtins();
    let prompt = commands
        .object_selection_prompt("ToNURBS ShowFacePacks=Yes ForceUpdateFacePacks=Yes")
        .unwrap()
        .unwrap();
    let mut cache = FacePackPreview::default();
    let colors = cache.view(&doc, Some(&prompt)).unwrap().unwrap();
    for flat in [false, true] {
        doc.set_subd_control_net(flat);
        for mode in [
            DisplayMode::Wireframe,
            DisplayMode::Shaded,
            DisplayMode::Ghosted,
        ] {
            let mut view = Viewport::new(ViewKind::Perspective);
            view.display_mode = mode;
            let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(512.));
            let build = || {
                view.object_scene_with_analyses(
                    rect,
                    &doc,
                    None,
                    &[],
                    None,
                    scene::SurfaceAnalyses {
                        face_packs: Some(&colors),
                        ..Default::default()
                    },
                )
            };
            let scene = build();
            assert!(!scene.triangles.is_empty());
            let mut seen = std::collections::BTreeSet::new();
            for triangle in scene.triangles.chunks_exact(3) {
                assert!(
                    triangle
                        .iter()
                        .all(|v| v.color == triangle[0].color && v.shading == 0.)
                );
                seen.insert(
                    triangle[0].color[..3]
                        .iter()
                        .map(|v| (v * 255.).round() as u8)
                        .collect::<Vec<_>>(),
                );
            }
            assert_eq!(seen.len(), 2);
            assert!(std::sync::Arc::ptr_eq(&scene, &build()));
            let plain = view.object_scene(rect, &doc);
            if mode == DisplayMode::Wireframe {
                assert!(plain.triangles.is_empty());
            }
        }
    }
    assert!(
        doc.object(id)
            .unwrap()
            .geometry_snapshot()
            .shares_storage_with(&original)
    );
    commands.execute(&mut doc, "Move 0,0,0 1,2,3").unwrap();
    assert!(colors.colors(doc.object(id).unwrap()).is_none());
    assert!(cache.view(&doc, None).unwrap().is_none());
}
