use super::*;

fn source(doc: &mut Document) -> ObjectId {
    doc.add_geometry(Geometry::Brep(
        Brep::try_box(
            CommandContext::default().construction_plane,
            [[0., 10.], [0., 12.], [0., 14.]],
            doc.tolerance(),
        )
        .unwrap(),
    ))
    .unwrap()
}

#[test]
fn collection_is_read_only_and_one_commit_preserves_identity_and_history() {
    let mut doc = Document::default();
    let id = source(&mut doc);
    let group = doc.add_group(Some("keep".into()), [id]).unwrap();
    let before = doc.object(id).unwrap().clone();
    let history = doc.undo_label().map(str::to_owned);
    let state = format!("{doc:?}");
    let mut selection = SplitEdgeSelection::prepare(&doc, id, 0).unwrap();
    selection
        .add_point(Point3::try_new(6., 0., 0.).unwrap())
        .unwrap();
    selection.add_parameter(2.).unwrap();
    assert_eq!(format!("{doc:?}"), state);
    selection.commit(&mut doc).unwrap();
    assert_eq!(doc.undo_label(), Some("SplitEdge"));
    let after = doc.object(id).unwrap().clone();
    let Geometry::Brep(brep) = after.geometry() else {
        panic!()
    };
    assert_eq!(brep.edges().len(), 14);
    assert_eq!(after.group_ids(), &[group]);
    doc.undo().unwrap();
    assert_eq!(doc.object(id).unwrap(), &before);
    assert_eq!(doc.undo_label(), history.as_deref());
    doc.redo().unwrap();
    assert_eq!(doc.object(id).unwrap(), &after);
}

#[test]
fn endpoint_replacement_has_history_but_empty_and_invalid_batches_do_not() {
    let mut doc = Document::default();
    let id = source(&mut doc);
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 1,2,3").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    SplitEdgeSelection::prepare(&doc, id, 0)
        .unwrap()
        .commit(&mut doc)
        .unwrap();
    for tail in ["", "0", "0 NaN", "0 2 2", "0 2 11", "99 2"] {
        assert!(
            registry
                .execute(&mut doc, &format!("SplitEdge {id} {tail}"))
                .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
    }
    let original = doc.object(id).unwrap().clone();
    registry
        .execute(&mut doc, &format!("SplitEdge {id} 0 0 10"))
        .unwrap();
    assert_eq!(doc.object(id).unwrap(), &original);
    assert_eq!(doc.undo_label(), Some("SplitEdge"));
    doc.undo().unwrap();
    assert_eq!(doc.object(id).unwrap(), &original);
}

#[test]
fn edits_tolerance_and_access_changes_invalidate_collected_points() {
    for change in ["edit", "hide", "lock", "tolerance"] {
        let mut doc = Document::default();
        let id = source(&mut doc);
        let mut pending = SplitEdgeSelection::prepare(&doc, id, 0).unwrap();
        pending.add_parameter(2.).unwrap();
        match change {
            "hide" => {
                doc.set_objects_visibility([id], false).unwrap();
            }
            "lock" => {
                doc.set_objects_locked([id], true).unwrap();
            }
            "tolerance" => {
                CommandRegistry::with_builtins()
                    .execute(&mut doc, "Tolerance Absolute=0.01")
                    .unwrap();
            }
            _ => {
                CommandRegistry::with_builtins()
                    .execute(&mut doc, &format!("SplitEdge {id} 0 4"))
                    .unwrap();
            }
        }
        let before = format!("{doc:?}");
        assert!(matches!(
            pending.commit(&mut doc),
            Err(CommandError::SplitEdgeStale)
        ));
        assert_eq!(format!("{doc:?}"), before);
    }
}
