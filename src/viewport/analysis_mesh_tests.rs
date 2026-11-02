//! Extraction copies the same analytic vertex colors submitted to the renderer.
use super::*;
use viboceros_command::CommandRegistry;

#[test]
fn extracted_analysis_mesh_copies_displayed_colors_and_leaves_other_faces_visible() {
    let r = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    r.execute(&mut doc, "SrfPt 0,0,0 3,0,1 3,2,0 0,2,2")
        .unwrap();
    let id = doc.objects().next().unwrap().id();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut doc, "DraftAngleAnalysis Range=-45,45 Up=1,0,0")
        .unwrap();
    let analysis = r.draft_angle_analysis_view(&doc).unwrap();
    let source = analysis
        .mesh(doc.object(id).unwrap(), doc.tolerance())
        .unwrap();
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(512.));
    let mut view = Viewport::new(ViewKind::Top);
    for mode in [
        DisplayMode::Wireframe,
        DisplayMode::Shaded,
        DisplayMode::Ghosted,
    ] {
        view.display_mode = mode;
        let scene = view.object_scene_with_analysis(rect, &doc, None, &[], None, Some(&analysis));
        let expected = viboceros_command::analysis_mesh::draft_colors(
            &source,
            analysis.options.mapping().unwrap(),
        );
        assert_eq!(scene.triangles.len(), 3 * expected.len());
        let mut by_position = std::collections::HashMap::<[u32; 2], Vec<[f32; 4]>>::new();
        for (triangle, colors) in source.mesh.triangles().iter().zip(&expected) {
            for (index, color) in triangle.iter().zip(colors) {
                let position = view
                    .gpu_position(source.mesh.vertices()[*index as usize])
                    .unwrap();
                let [r, g, b] = color.map(|v| f32::from(v) / 255.);
                by_position
                    .entry([position[0].to_bits(), position[1].to_bits()])
                    .or_default()
                    .push([r, g, b, 1.]);
            }
        }
        for vertex in &scene.triangles {
            let key = [vertex.position[0].to_bits(), vertex.position[1].to_bits()];
            assert!(by_position[&key].contains(&vertex.color));
            assert_eq!(vertex.shading, 0.);
        }
    }
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut doc, "ExtractAnalysisMesh").unwrap();
    let output = doc.objects().find(|o| o.id() != id).unwrap();
    let copied = output.geometry_snapshot().clone();
    let Geometry::Mesh(mesh) = output.geometry() else {
        panic!()
    };
    let expected = viboceros_command::analysis_mesh::draft_colors(
        &source,
        analysis.options.mapping().unwrap(),
    );
    for (tri, colors) in mesh.triangles().iter().zip(&expected) {
        for (index, color) in tri.iter().zip(colors) {
            assert_eq!(&mesh.vertex_colors().unwrap()[*index as usize][..3], color);
        }
    }
    let output_id = output.id();
    let copied_triangles = mesh.triangles().len();
    r.execute(&mut doc, "DraftAngleAnalysis Up=-1,0,0").unwrap();
    assert!(copied.shares_storage_with(doc.object(output_id).unwrap().geometry_snapshot()));
    // A nonmember surface must continue through ordinary shaded rendering.
    r.execute(&mut doc, "SrfPt 5,0,0 8,0,0 8,2,0 5,2,0")
        .unwrap();
    view.display_mode = DisplayMode::Shaded;
    let current = r.draft_angle_analysis_view(&doc).unwrap();
    let scene = view.object_scene_with_analysis(rect, &doc, None, &[], None, Some(&current));
    assert!(scene.triangles.iter().any(|v| v.shading != 0.));
    assert!(scene.triangles.len() > 3 * expected.len() + 3 * copied_triangles);
}
