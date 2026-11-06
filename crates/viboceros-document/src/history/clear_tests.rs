use crate::{Document, DocumentError, Geometry};
use viboceros_geometry::Point3;

#[test]
fn clearing_undo_records_keeps_last_object_recall_and_geometry() {
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
        .unwrap();
    doc.clear_undo_records().unwrap();
    assert!(!doc.can_undo());
    assert!(!doc.can_redo());
    assert_eq!(doc.select_last_changed(true), 1);
    assert!(doc.is_selected(id));
    doc.clear_history().unwrap();
    assert_eq!(doc.select_last_changed(true), 0);
    assert_eq!(doc.objects().len(), 1);
}

#[test]
fn clearing_undo_records_drops_redo_and_rejects_active_transactions() {
    let mut doc = Document::default();
    doc.add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
        .unwrap();
    doc.undo().unwrap();
    assert!(doc.can_redo());
    doc.clear_undo_records().unwrap();
    assert!(!doc.can_redo());
    assert!(doc.objects().next().is_none());
    doc.begin_transaction("pending").unwrap();
    assert!(matches!(
        doc.clear_undo_records(),
        Err(DocumentError::TransactionInProgress)
    ));
    doc.rollback_transaction().unwrap();
}
