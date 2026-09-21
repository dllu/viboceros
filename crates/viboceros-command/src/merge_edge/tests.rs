use super::*;

fn fixture(document: &mut Document) -> ObjectId {
    let brep = Brep::try_box(
        CommandContext::default().construction_plane,
        [[0., 2.], [0., 3.], [0., 5.]],
        document.tolerance(),
    )
    .unwrap()
    .try_split_edges_at_parameters(&[(0, vec![0.25, 0.75, 1.75])], document.tolerance())
    .unwrap();
    document.add_geometry(Geometry::Brep(brep)).unwrap()
}

#[test]
fn choices_do_not_mutate_and_each_commit_is_one_identity_preserving_undo() {
    let registry = CommandRegistry::with_builtins();
    for (choice, removed) in [
        (MergeEdgeChoice::EdgeA, 1),
        (MergeEdgeChoice::EdgeB, 1),
        (MergeEdgeChoice::Both, 2),
        (MergeEdgeChoice::All, 3),
    ] {
        let mut doc = Document::default();
        let id = fixture(&mut doc);
        let group = doc.add_group(Some("keep".into()), [id]).unwrap();
        let before = format!("{doc:?}");
        let history_before = doc.undo_label().map(str::to_owned);
        let selection = MergeEdgeSelection::prepare(&doc, id, 13).unwrap();
        assert_eq!(selection.choices().len(), 4);
        assert_eq!(format!("{doc:?}"), before);
        selection.commit(&mut doc, choice).unwrap();
        assert_eq!(doc.undo_label(), Some("MergeEdge"));
        assert_eq!(doc.object(id).unwrap().group_ids(), &[group]);
        let Geometry::Brep(brep) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        assert_eq!(brep.edges().len(), 15 - removed);
        let after = doc.object(id).unwrap().clone();
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.object(id).unwrap().geometry(), &selection.source);
        assert_eq!(doc.undo_label(), history_before.as_deref());
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.object(id).unwrap(), &after);
    }
}

#[test]
fn no_op_bad_inputs_and_stale_picks_preserve_redo_and_the_document() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let id = fixture(&mut doc);
    registry.execute(&mut doc, "Point 7,8,9").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let before = format!("{doc:?}");
    assert!(
        MergeEdgeSelection::prepare(&doc, id, 1)
            .unwrap()
            .choices()
            .is_empty()
    );
    for tail in [
        "",
        "x",
        "0 extra garbage",
        "1 All",
        "999 All",
        "13 nonsense",
        "0 EdgeA",
    ] {
        assert!(
            registry
                .execute(&mut doc, &format!("MergeEdge {id} {tail}"))
                .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
    }
    let pending = MergeEdgeSelection::prepare(&doc, id, 13).unwrap();
    registry
        .execute(&mut doc, &format!("MergeEdge {id} 13 All"))
        .unwrap();
    let after = format!("{doc:?}");
    assert!(matches!(
        pending.commit(&mut doc, MergeEdgeChoice::All),
        Err(CommandError::MergeEdgeStale)
    ));
    assert_eq!(format!("{doc:?}"), after);
}

#[test]
fn endpoints_offer_edge_or_all_and_hidden_or_locked_components_cannot_be_edited() {
    let registry = CommandRegistry::with_builtins();
    for edge in [0, 12] {
        let mut doc = Document::default();
        let id = fixture(&mut doc);
        let selection = MergeEdgeSelection::prepare(&doc, id, edge).unwrap();
        assert_eq!(
            selection.choices(),
            &[MergeEdgeChoice::Edge, MergeEdgeChoice::All]
        );
        registry
            .execute(&mut doc, &format!("_-MergeEdge {id} {edge} _Edge"))
            .unwrap();
        let Geometry::Brep(brep) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        assert_eq!(brep.edges().len(), 14);
        doc.set_objects_locked([id], true).unwrap();
        assert!(MergeEdgeSelection::prepare(&doc, id, 0).is_err());
        let before = format!("{doc:?}");
        assert!(
            registry
                .execute(&mut doc, &format!("MergeEdge {id} 0 All"))
                .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
    }
}
