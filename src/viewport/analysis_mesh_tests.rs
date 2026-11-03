//! Extraction copies the same analytic vertex colors submitted to the renderer.
use super::*;
use viboceros_command::CommandRegistry;

#[test]
fn analysis_nurbs_density_refreshes_rendered_faces_and_temporary_meshes() {
    use viboceros_geometry::{AnalysisMeshSettings, NurbsMeshDensity, NurbsSurface};
    let r = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let surface = NurbsSurface::try_new(
        3,
        1,
        4,
        2,
        (0..2)
            .flat_map(|v| {
                [0., 1., -1., 0.]
                    .into_iter()
                    .enumerate()
                    .map(move |(u, z)| Point3::try_new(u as Real / 3., v as Real, z).unwrap())
            })
            .collect(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::NurbsSurface(surface)).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut doc, "DraftAngleAnalysis").unwrap();
    let view = r.draft_angle_analysis_view(&doc).unwrap();
    let geometry = doc.object(id).unwrap().geometry_snapshot().clone();
    let display = display_cache::DisplayGeometry::new(geometry.clone(), 1, doc.tolerance());
    let ordinary = display.mesh().unwrap().faces().len();
    let mut viewport = Viewport::new(ViewKind::Top);
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(512.));
    let mut counts = vec![];
    for density in [0.25, 0.8, 1.] {
        r.execute(
            &mut doc,
            &format!("DraftAngleAnalysis NurbsDensity={density}"),
        )
        .unwrap();
        let settings = AnalysisMeshSettings {
            nurbs_density: NurbsMeshDensity::new(density).unwrap(),
            ..Default::default()
        };
        let temporary = display.curvature_mesh_with_settings(settings).unwrap();
        assert!(std::sync::Arc::ptr_eq(
            &temporary,
            &display.curvature_mesh_with_settings(settings).unwrap()
        ));
        counts.push(temporary.mesh.faces().len());
        assert_eq!(display.mesh().unwrap().faces().len(), ordinary);
        for mode in [
            DisplayMode::Wireframe,
            DisplayMode::Shaded,
            DisplayMode::Ghosted,
        ] {
            viewport.display_mode = mode;
            let scene =
                viewport.object_scene_with_analysis(rect, &doc, None, &[], None, Some(&view));
            assert_eq!(scene.triangles.len(), temporary.mesh.triangles().len() * 3);
        }
        assert!(geometry.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
    }
    assert!(counts.windows(2).all(|c| c[0] <= c[1]));
    assert!(counts[0] < counts[2]);
}

#[test]
fn analysis_subd_density_refreshes_rendered_faces_and_temporary_meshes() {
    use viboceros_geometry::{SubD, SubDMeshDensity};
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let subd = SubD::try_from_polygons(
        [[0., 0., 0.], [3., 0., 0.], [3., 2., 0.], [0., 2., 0.]]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        vec![vec![0, 1, 2, 3]],
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::SubD(subd)).unwrap();
    let geometry = doc.object(id).unwrap().geometry_snapshot().clone();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut doc, "DraftAngleAnalysis").unwrap();
    let analysis = registry.draft_angle_analysis_view(&doc).unwrap();
    let display = display_cache::DisplayGeometry::new(geometry.clone(), 1, doc.tolerance());
    let ordinary_faces = display.mesh().unwrap().faces().len();
    let mut view = Viewport::new(ViewKind::Top);
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(512.));
    for level in [2, 4, 1] {
        registry
            .execute(&mut doc, &format!("DraftAngleAnalysis SubDLevel={level}"))
            .unwrap();
        let density = SubDMeshDensity::new(level, true).unwrap();
        let temporary = display
            .curvature_mesh_with_settings(viboceros_geometry::AnalysisMeshSettings {
                subd_density: density,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(temporary.mesh.faces().len(), 4usize.pow(u32::from(level)));
        assert!(std::sync::Arc::ptr_eq(
            &temporary,
            &display
                .curvature_mesh_with_settings(viboceros_geometry::AnalysisMeshSettings {
                    subd_density: density,
                    ..Default::default()
                })
                .unwrap()
        ));
        assert_eq!(display.mesh().unwrap().faces().len(), ordinary_faces);
        for mode in [
            DisplayMode::Wireframe,
            DisplayMode::Shaded,
            DisplayMode::Ghosted,
        ] {
            view.display_mode = mode;
            let scene =
                view.object_scene_with_analysis(rect, &doc, None, &[], None, Some(&analysis));
            assert_eq!(scene.triangles.len(), temporary.mesh.triangles().len() * 3);
        }
        assert!(geometry.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
    }
}

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
    let source = analysis.mesh(doc.object(id).unwrap(), &doc).unwrap();
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
