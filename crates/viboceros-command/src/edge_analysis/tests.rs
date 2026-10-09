use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn selected_box(registry: &CommandRegistry, doc: &mut Document) {
    registry.execute(doc, "Box 0,0,0 2,3,0 4").unwrap();
    registry.execute(doc, "SelAll").unwrap();
}
#[test]
fn show_edges_is_non_destructive_and_marking_uses_one_undo_step() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    selected_box(&registry, &mut doc);
    let objects = doc.objects().cloned().collect::<Vec<_>>();
    let history = doc.undo_label().map(str::to_owned);
    registry.execute(&mut doc, "ShowEdges Show=Naked").unwrap();
    assert_eq!(
        registry
            .edge_analysis_view(&doc)
            .unwrap()
            .unwrap()
            .displayed()
            .count(),
        0
    );
    registry.execute(&mut doc, "ShowEdges Show=All").unwrap();
    let view = registry.edge_analysis_view(&doc).unwrap().unwrap();
    assert_eq!(view.displayed().count(), 12);
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(doc.undo_label(), history.as_deref());
    registry
        .execute(&mut doc, "ShowEdges Zoom Current")
        .unwrap();
    registry.execute(&mut doc, "ShowEdges Mark").unwrap();
    assert_eq!(doc.objects().len(), 3);
    doc.undo().unwrap();
    assert_eq!(doc.objects().len(), 1);
    assert!(registry.edge_analysis_view(&doc).unwrap().is_some());
    registry.execute(&mut doc, "ShowEdgesOff").unwrap();
    assert!(registry.edge_analysis_view(&doc).unwrap().is_none());
}
#[test]
fn mesh_modes_distinguish_welded_unwelded_and_non_manifold_sides() {
    let registry = CommandRegistry::with_builtins();
    for (vertices, faces, all, naked, non_manifold) in [
        (
            vec![p(0., 0., 0.), p(1., 0., 0.), p(0., 1., 0.), p(1., 1., 0.)],
            vec![[0, 1, 2], [1, 3, 2]],
            4,
            4,
            0,
        ),
        (
            vec![
                p(0., 0., 0.),
                p(1., 0., 0.),
                p(0., 1., 0.),
                p(1., 0., 0.),
                p(1., 1., 0.),
                p(0., 1., 0.),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            5,
            4,
            0,
        ),
        (
            vec![
                p(0., 0., 0.),
                p(1., 0., 0.),
                p(0., 1., 0.),
                p(0., -1., 0.),
                p(0., 0., 1.),
            ],
            vec![[0, 1, 2], [1, 0, 3], [0, 1, 4]],
            6,
            6,
            1,
        ),
    ] {
        let mut doc = Document::default();
        let mesh = TriangleMesh::try_new(vertices, faces, Tolerance::DEFAULT).unwrap();
        let id = doc.add_geometry(Geometry::Mesh(mesh)).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        for (mode, count) in [
            ("All", all),
            ("Naked", naked),
            ("NonManifold", non_manifold),
        ] {
            registry
                .execute(&mut doc, &format!("ShowEdges Show={mode}"))
                .unwrap();
            assert_eq!(
                registry
                    .edge_analysis_view(&doc)
                    .unwrap()
                    .unwrap()
                    .displayed()
                    .count(),
                count
            );
        }
        registry.execute(&mut doc, "ShowEdgesOff").unwrap();
    }
}
#[test]
fn cache_survives_options_and_rebuilds_after_geometry_edits_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    selected_box(&registry, &mut doc);
    registry.execute(&mut doc, "ShowEdges Show=All").unwrap();
    let original = registry.edge_analysis_view(&doc).unwrap().unwrap();
    registry
        .execute(&mut doc, "ShowEdges Color=12,34,56")
        .unwrap();
    let recolored = registry.edge_analysis_view(&doc).unwrap().unwrap();
    assert!(Arc::ptr_eq(&original.edges, &recolored.edges));
    assert_eq!(recolored.color, [12, 34, 56]);
    registry.execute(&mut doc, "Move 0,0,0 5,0,0").unwrap();
    let moved = registry.edge_analysis_view(&doc).unwrap().unwrap();
    assert!(!Arc::ptr_eq(&original.edges, &moved.edges));
    assert_eq!(
        moved.edges[0].endpoints[0].x(),
        original.edges[0].endpoints[0].x() + 5.
    );
    doc.undo().unwrap();
    let restored = registry.edge_analysis_view(&doc).unwrap().unwrap();
    assert_eq!(restored.edges[0].endpoints, original.edges[0].endpoints);
}
#[test]
fn adding_removing_navigating_and_invalid_options_preserve_session_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    selected_box(&registry, &mut doc);
    let first = doc.objects().next().unwrap().id();
    registry.execute(&mut doc, "ShowEdges Show=All").unwrap();
    registry.execute(&mut doc, "Box 5,0,0 7,3,0 4").unwrap();
    let second = doc.objects().last().unwrap().id();
    doc.select_objects_direct([second], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut doc, "ShowEdges Color=1,2,3").unwrap();
    assert_eq!(
        registry.edge_analysis_view(&doc).unwrap().unwrap().sources,
        1
    );
    registry.execute(&mut doc, "ShowEdges Add").unwrap();
    assert_eq!(
        registry.edge_analysis_view(&doc).unwrap().unwrap().sources,
        2
    );
    registry.execute(&mut doc, "ShowEdges Zoom Next").unwrap();
    assert_eq!(
        registry.edge_analysis_view(&doc).unwrap().unwrap().current,
        Some(1)
    );
    for command in [
        "ShowEdges Color=999,0,0",
        "ShowEdges Show=bad",
        "ShowEdges Add Remove",
        "ShowEdgesOff unexpected",
    ] {
        assert!(registry.execute(&mut doc, command).is_err());
        assert_eq!(
            registry.edge_analysis_view(&doc).unwrap().unwrap().sources,
            2
        );
    }
    registry.execute(&mut doc, "ShowEdges Remove").unwrap();
    let view = registry.edge_analysis_view(&doc).unwrap().unwrap();
    assert_eq!(view.sources, 1);
    assert!(view.edges.iter().all(|e| e.object == first));
    registry.execute(&mut doc, "Delete").unwrap();
    assert_eq!(
        registry.edge_analysis_view(&doc).unwrap().unwrap().sources,
        1
    );
}
#[test]
fn surface_boundaries_and_periodic_sphere_seams_have_distinct_diagnostic_valence() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry
        .execute(&mut doc, "SrfPt 0,0,0 2,0,0 2,3,0 0,3,0")
        .unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry.execute(&mut doc, "ShowEdges").unwrap();
    assert_eq!(
        registry
            .edge_analysis_view(&doc)
            .unwrap()
            .unwrap()
            .displayed()
            .count(),
        4
    );
    registry.execute(&mut doc, "ShowEdgesOff").unwrap();
    registry.execute(&mut doc, "Clear").unwrap();
    registry.execute(&mut doc, "Sphere 0,0,0 2").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry.execute(&mut doc, "ShowEdges Show=Naked").unwrap();
    assert_eq!(
        registry
            .edge_analysis_view(&doc)
            .unwrap()
            .unwrap()
            .displayed()
            .count(),
        0
    );
    registry.execute(&mut doc, "ShowEdges Show=All").unwrap();
    assert!(
        registry
            .edge_analysis_view(&doc)
            .unwrap()
            .unwrap()
            .displayed()
            .count()
            > 0
    );
}
