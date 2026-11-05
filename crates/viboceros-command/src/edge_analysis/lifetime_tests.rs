use super::*;

#[test]
fn deleted_sources_release_cached_geometry_and_rejoin_after_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry
        .execute(&mut doc, "SrfPt 0,0,0 2,0,0 2,3,0 0,3,0")
        .unwrap();
    let id = doc.objects().next().unwrap().id();
    let geometry = doc.object(id).unwrap().geometry_snapshot().clone();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry
        .execute(&mut doc, "ShowEdges Show=Naked Color=12,34,56")
        .unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry.execute(&mut doc, "Delete").unwrap();
    let missing = registry.edge_analysis_view(&doc).unwrap().unwrap();
    assert_eq!(missing.sources, 0);
    assert!(missing.edges.is_empty());
    assert!(registry.edge_analysis.0.lock().unwrap().cache.is_empty());
    assert!(
        registry
            .edge_analysis
            .0
            .lock()
            .unwrap()
            .sources
            .contains(&id)
    );
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.redo_label(), Some("Delete"));
    let restored = registry.edge_analysis_view(&doc).unwrap().unwrap();
    assert_eq!(restored.sources, 1);
    assert_eq!(restored.displayed().count(), 4);
    assert_eq!(restored.color, [12, 34, 56]);
    assert!(geometry.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
    assert_eq!(doc.redo_label(), Some("Delete"));
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(
        registry.edge_analysis_view(&doc).unwrap().unwrap().sources,
        0
    );
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(
        registry.edge_analysis_view(&doc).unwrap().unwrap().sources,
        1
    );
}

#[test]
fn explicit_removal_and_closing_analysis_discard_restoration_membership() {
    for off in [false, true] {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        registry
            .execute(&mut doc, "SrfPt 0,0,0 2,0,0 2,3,0 0,3,0")
            .unwrap();
        registry.execute(&mut doc, "SelAll").unwrap();
        registry.execute(&mut doc, "ShowEdges").unwrap();
        registry.execute(&mut doc, "SelAll").unwrap();
        if !off {
            registry.execute(&mut doc, "ShowEdges Remove").unwrap();
        }
        registry.execute(&mut doc, "Delete").unwrap();
        registry.edge_analysis_view(&doc).unwrap();
        if off {
            registry.execute(&mut doc, "ShowEdgesOff").unwrap();
        }
        registry.execute(&mut doc, "Undo").unwrap();
        if off {
            assert!(registry.edge_analysis_view(&doc).unwrap().is_none());
        } else {
            assert_eq!(
                registry.edge_analysis_view(&doc).unwrap().unwrap().sources,
                0
            );
        }
        assert!(registry.edge_analysis.0.lock().unwrap().sources.is_empty());
    }
}

#[test]
fn assembly_refresh_evicts_missing_entries_and_reuses_surviving_caches() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let mesh = Geometry::Mesh(
        TriangleMesh::try_new(
            vec![
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(1., 0., 0.).unwrap(),
                Point3::try_new(0., 1., 0.).unwrap(),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap(),
    );
    let ids = (0..512)
        .map(|_| doc.add_geometry(mesh.clone()).unwrap())
        .collect::<Vec<_>>();
    doc.clear_history().unwrap();
    doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut doc, "ShowEdges").unwrap();
    let survivor = ids[1];
    let cached = registry.edge_analysis.0.lock().unwrap().cache[&survivor]
        .edges
        .clone();
    doc.select_objects_direct(ids.iter().step_by(2).copied(), SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut doc, "Delete").unwrap();
    let missing = registry.edge_analysis_view(&doc).unwrap().unwrap();
    assert_eq!(missing.sources, 256);
    assert_eq!(missing.displayed().count(), 256 * 3);
    assert_eq!(registry.edge_analysis.0.lock().unwrap().cache.len(), 256);
    registry.execute(&mut doc, "Undo").unwrap();
    let restored = registry.edge_analysis_view(&doc).unwrap().unwrap();
    assert_eq!(restored.sources, 512);
    assert_eq!(restored.displayed().count(), 512 * 3);
    assert!(Arc::ptr_eq(
        &cached,
        &registry.edge_analysis.0.lock().unwrap().cache[&survivor].edges
    ));
    assert_eq!(doc.redo_label(), Some("Delete"));
}

#[test]
fn modeless_getter_picks_remain_temporary_until_zoom_finishes() {
    use viboceros_document::ObjectSelectionState;
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry
        .execute(&mut doc, "SrfPt 0,0,0 2,0,0 2,3,0 0,3,0")
        .unwrap();
    registry
        .execute(&mut doc, "SrfPt 5,0,0 7,0,0 7,3,0 5,3,0")
        .unwrap();
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    doc.select_objects_direct([ids[0]], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut doc, "ShowEdges").unwrap();
    assert_eq!(
        registry.edge_analysis_view(&doc).unwrap().unwrap().mode,
        Mode::All
    );
    doc.select_objects_direct([ids[1]], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut doc, "ShowEdges Add").unwrap();
    assert_eq!(
        doc.object_selection_state(ids[1]),
        ObjectSelectionState::Temporary
    );
    assert!(
        registry
            .execute(&mut doc, "ShowEdgesOff unexpected")
            .is_err()
    );
    assert_eq!(
        doc.object_selection_state(ids[1]),
        ObjectSelectionState::Temporary
    );
    registry.execute(&mut doc, "ShowEdges Show=Naked").unwrap();
    let name = registry
        .begin_edge_analysis_zoom(&mut doc)
        .unwrap()
        .unwrap();
    assert_eq!(
        doc.object_selection_state(ids[1]),
        ObjectSelectionState::Temporary
    );
    let mut group = doc.begin_history_group(name).unwrap();
    registry
        .execute_edge_zoom_actions(&mut doc, "All Mark", &mut group)
        .unwrap();
    registry.finish_edge_zoom(&mut doc);
    assert_eq!(
        doc.object_selection_state(ids[1]),
        ObjectSelectionState::Unselected
    );
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().len(), 2);
}

#[test]
fn ordinary_command_completion_releases_only_temporary_panel_picks() {
    use viboceros_document::ObjectSelectionState;
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Point 0,0,0").unwrap();
    registry.execute(&mut doc, "Point 1,0,0").unwrap();
    registry.execute(&mut doc, "Point 2,0,0").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    doc.mark_object_selection_temporary([ids[1]]);
    let redo = doc.redo_label().map(str::to_owned);
    registry.execute(&mut doc, "Help").unwrap();
    assert_eq!(
        doc.object_selection_state(ids[0]),
        ObjectSelectionState::Persistent
    );
    assert_eq!(
        doc.object_selection_state(ids[1]),
        ObjectSelectionState::Unselected
    );
    assert_eq!(doc.redo_label(), redo.as_deref());
}
