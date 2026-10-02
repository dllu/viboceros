use super::*;

fn source(document: &mut Document) -> ObjectId {
    document
        .add_geometry(Geometry::Brep(
            Brep::try_box(
                CommandContext::default().construction_plane,
                [[0., 2.], [0., 3.], [0., 4.]],
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap()
}

#[test]
fn different_face_sets_share_one_edit_and_leave_remainder_groups_intact() {
    let mut doc = Document::default();
    let a = source(&mut doc);
    let b = source(&mut doc);
    let original = doc
        .objects()
        .map(|o| (o.id(), o.geometry().clone()))
        .collect::<Vec<_>>();
    let group = doc.add_group(Some("shells".into()), [a, b]).unwrap();
    doc.clear_history().unwrap();
    let staged =
        ExtractSurfaceSelection::prepare(&doc, [(a, 2), (a, 2), (b, 5), (a, 0)], false, false)
            .unwrap();
    assert_eq!(doc.objects().len(), 2);
    assert!(!doc.can_undo());
    staged.commit(&mut doc).unwrap();
    assert_eq!(doc.objects().len(), 5);
    assert_eq!(doc.selected_object_count(), 3);
    assert_eq!(
        doc.group(group).unwrap().members().collect::<BTreeSet<_>>(),
        [a, b].into_iter().collect()
    );
    for object in doc.selected_objects() {
        assert!(object.group_ids().is_empty());
    }
    let Geometry::Brep(first) = doc.object(a).unwrap().geometry() else {
        panic!()
    };
    let Geometry::Brep(second) = doc.object(b).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(first.faces().len(), 4);
    assert_eq!(second.faces().len(), 5);
    assert_eq!(doc.undo_label(), Some("ExtractSrf"));
    doc.undo().unwrap();
    assert_eq!(doc.objects().len(), 2);
    for (id, geometry) in original {
        assert_eq!(doc.object(id).unwrap().geometry(), &geometry);
    }
    doc.redo().unwrap();
    assert_eq!(doc.objects().len(), 5);
}

#[test]
fn invalid_later_index_and_stale_geometry_or_attributes_do_not_extract_partial_faces() {
    let mut doc = Document::default();
    let a = source(&mut doc);
    let b = source(&mut doc);
    doc.clear_history().unwrap();
    assert!(ExtractSurfaceSelection::prepare(&doc, [(a, 0), (b, 6)], false, false).is_err());
    assert_eq!(doc.objects().len(), 2);
    assert!(!doc.can_undo());
    let stage = ExtractSurfaceSelection::prepare(&doc, [(a, 0), (b, 1)], false, false).unwrap();
    doc.set_object_names([(b, Some("changed".into()))]).unwrap();
    assert!(matches!(
        stage.commit(&mut doc),
        Err(CommandError::ExtractSurfaceStale)
    ));
    assert_eq!(doc.objects().len(), 2);
    let stage = ExtractSurfaceSelection::prepare(&doc, [(a, 0), (b, 1)], false, false).unwrap();
    let geometry = doc.object(b).unwrap().geometry().clone();
    let Geometry::Brep(brep) = geometry else {
        panic!()
    };
    doc.replace_object_geometries([(
        b,
        Geometry::Brep(brep.sub_brep(&[0, 1, 2, 3, 4], doc.tolerance()).unwrap()),
    )])
    .unwrap();
    assert!(matches!(
        stage.commit(&mut doc),
        Err(CommandError::ExtractSurfaceStale)
    ));
    assert_eq!(doc.objects().len(), 2);
}

#[test]
fn copying_all_faces_retains_sources_and_creates_ungrouped_current_layer_outputs() {
    let mut doc = Document::default();
    let id = source(&mut doc);
    let group = doc.add_group(None, [id]).unwrap();
    let layer = doc.add_layer("output", ColorRgb::new(10, 20, 30)).unwrap();
    doc.set_current_layer(layer).unwrap();
    let before = doc.object(id).unwrap().geometry().clone();
    ExtractSurfaceSelection::prepare(&doc, (0..6).map(|f| (id, f)), true, true)
        .unwrap()
        .commit(&mut doc)
        .unwrap();
    assert_eq!(doc.object(id).unwrap().geometry(), &before);
    assert_eq!(
        doc.group(group).unwrap().members().collect::<Vec<_>>(),
        [id]
    );
    assert_eq!(doc.selected_object_count(), 6);
    for object in doc.selected_objects() {
        assert_eq!(object.attributes().layer_id(), layer);
        assert!(object.group_ids().is_empty());
    }
}

#[test]
fn changed_source_order_or_current_output_layer_rejects_staged_outputs_atomically() {
    let mut doc = Document::default();
    let a = source(&mut doc);
    let b = source(&mut doc);
    let unrelated = source(&mut doc);
    let stage = ExtractSurfaceSelection::prepare(&doc, [(a, 0), (b, 1)], false, false).unwrap();
    doc.move_objects_to_end_in_order([a]).unwrap();
    doc.clear_history().unwrap();
    assert!(matches!(
        stage.commit(&mut doc),
        Err(CommandError::ExtractSurfaceStale)
    ));
    assert_eq!(
        doc.objects().map(|object| object.id()).collect::<Vec<_>>(),
        [b, unrelated, a]
    );
    assert!(!doc.can_undo());

    let stage = ExtractSurfaceSelection::prepare(&doc, [(a, 0), (b, 1)], true, true).unwrap();
    let layer = doc
        .add_layer("different output", ColorRgb::new(10, 20, 30))
        .unwrap();
    doc.set_current_layer(layer).unwrap();
    doc.clear_history().unwrap();
    assert!(matches!(
        stage.commit(&mut doc),
        Err(CommandError::ExtractSurfaceStale)
    ));
    assert_eq!(doc.objects().len(), 3);
    assert!(!doc.can_undo());

    // Renewing an unrelated object leaves the sources' relative order valid.
    let stage = ExtractSurfaceSelection::prepare(&doc, [(a, 0), (b, 1)], true, true).unwrap();
    doc.move_objects_to_end_in_order([unrelated]).unwrap();
    doc.clear_history().unwrap();
    stage.commit(&mut doc).unwrap();
    assert_eq!(doc.objects().len(), 5);
    assert_eq!(doc.undo_label(), Some("ExtractSrf"));
}
