use super::*;
use crate::Geometry;
use viboceros_geometry::Point3;

fn add(doc: &mut Document, x: f64) {
    doc.add_geometry(Geometry::Point(Point3::try_new(x, 0., 0.).unwrap()))
        .unwrap();
}
#[test]
fn hundreds_of_commands_retain_one_undo_and_the_prior_history_limit() {
    let mut doc = Document::default();
    for i in 0..100 {
        add(&mut doc, i as f64);
    }
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let batch = doc.begin_history_batch("ReadCommandFile").unwrap();
    for i in 100..500 {
        add(&mut doc, i as f64);
        doc.checkpoint_history_batch(&batch).unwrap();
    }
    assert_eq!(doc.history.undo.len(), 101);
    doc.finish_history_batch(&batch).unwrap();
    assert_eq!(doc.history.undo.len(), 100);
    let after = doc.objects().cloned().collect::<Vec<_>>();
    assert_eq!(doc.undo().unwrap().as_deref(), Some("ReadCommandFile"));
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    doc.redo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
    doc.undo().unwrap();
    doc.undo().unwrap();
    assert_eq!(doc.objects().len(), 99);
}
#[test]
fn no_op_batches_keep_pending_redo_and_do_not_add_history() {
    let mut doc = Document::default();
    add(&mut doc, 1.);
    doc.undo().unwrap();
    let redo = doc.redo_label().map(str::to_owned);
    let batch = doc.begin_history_batch("empty").unwrap();
    assert!(!doc.finish_history_batch(&batch).unwrap());
    assert!(!doc.can_undo());
    assert_eq!(doc.redo_label(), redo.as_deref());
    doc.redo().unwrap();
}
#[test]
fn child_getters_keep_local_undo_until_the_command_checkpoint() {
    let mut doc = Document::default();
    let batch = doc.begin_history_batch("file").unwrap();
    let mut child = doc.begin_history_group("Points").unwrap();
    for i in 0..3 {
        doc.begin_group_transaction(&child).unwrap();
        add(&mut doc, i as f64);
        doc.commit_group_transaction(&mut child).unwrap();
    }
    doc.undo_history_group(&mut child).unwrap();
    assert_eq!(doc.objects().len(), 2);
    doc.checkpoint_history_batch(&batch).unwrap();
    doc.finish_history_batch(&batch).unwrap();
    doc.undo().unwrap();
    assert_eq!(doc.objects().len(), 0);
    doc.redo().unwrap();
    assert_eq!(doc.objects().len(), 2);
}
#[test]
fn nested_batches_merge_without_copying_or_reordering_edits() {
    let mut doc = Document::default();
    let outer = doc.begin_history_batch("outer").unwrap();
    add(&mut doc, 1.);
    doc.checkpoint_history_batch(&outer).unwrap();
    let inner = doc.begin_history_batch("inner").unwrap();
    add(&mut doc, 2.);
    doc.finish_history_batch(&inner).unwrap();
    doc.finish_history_batch(&outer).unwrap();
    assert_eq!(doc.undo_label(), Some("outer"));
    doc.undo().unwrap();
    assert_eq!(doc.objects().len(), 0);
    doc.redo().unwrap();
    assert_eq!(doc.objects().len(), 2);
}
#[test]
fn changed_prefix_is_reported_and_releases_the_deferred_history_limit() {
    let mut doc = Document::default();
    add(&mut doc, 1.);
    let batch = doc.begin_history_batch("file").unwrap();
    doc.clear_history().unwrap();
    assert!(!doc.history_batch_is_current(&batch));
    assert!(matches!(
        doc.finish_history_batch(&batch),
        Err(DocumentError::HistoryBatchStale)
    ));
    assert!(doc.history.batches.is_empty());
    assert_eq!(doc.objects().len(), 1);
}

#[test]
fn ordinary_undo_and_redo_wait_until_the_outer_command_finishes() {
    let mut doc = Document::default();
    add(&mut doc, 1.);
    let batch = doc.begin_history_batch("file").unwrap();
    add(&mut doc, 2.);
    assert!(doc.undo().unwrap().is_none());
    assert!(doc.redo().unwrap().is_none());
    assert_eq!(doc.objects().len(), 2);
    doc.finish_history_batch(&batch).unwrap();
    doc.undo().unwrap();
    assert_eq!(doc.objects().len(), 1);
}
