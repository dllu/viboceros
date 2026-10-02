//! Native transform replay policy must not weaken pending-step rollback.
use crate::*;

fn point(x: f64) -> Geometry {
    Geometry::Point(Point3::try_new(x, 0., 0.).unwrap())
}

fn scale(factor: f64) -> AffineTransform3 {
    AffineTransform3::try_uniform_scale(Point3::try_new(0., 0., 0.).unwrap(), factor).unwrap()
}

#[test]
fn retained_copy_groups_and_renewed_order_replay_atomically_without_retaining_failed_groups() {
    let mut doc = Document::default();
    let source = doc.add_geometry(point(2.)).unwrap();
    let peer = doc.add_geometry(point(9.)).unwrap();
    doc.add_group(Some("Sources".into()), [source, peer])
        .unwrap();
    doc.select_objects_direct([source, peer], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    let mut batch = doc.begin_history_group("Transform").unwrap();
    batch.keep_created_group_definitions();
    batch.renew_changed_object_order();
    doc.begin_group_transaction(&batch).unwrap();
    let copies = doc
        .copy_objects_transformed([source, peer], scale(2.))
        .unwrap();
    doc.select_objects_direct([source, peer], SelectionMode::Replace)
        .unwrap();
    doc.commit_group_transaction(&mut batch).unwrap();
    let created = doc.groups().last().unwrap().id();
    let accepted = format!("{doc:?}");
    doc.begin_group_transaction(&batch).unwrap();
    doc.add_empty_group(Some("Failed allocation".into()))
        .unwrap();
    doc.transform_objects([source], scale(3.)).unwrap();
    doc.rollback_transaction().unwrap();
    assert_eq!(format!("{doc:?}"), accepted);
    assert!(doc.history_group_is_current(&batch));
    doc.begin_group_transaction(&batch).unwrap();
    doc.transform_objects([source, peer], scale(3.)).unwrap();
    doc.commit_group_transaction(&mut batch).unwrap();
    assert_eq!(
        doc.objects().map(Object::id).collect::<Vec<_>>(),
        [copies[0], copies[1], source, peer]
    );
    let after = doc.objects().cloned().collect::<Vec<_>>();
    doc.remove_group(created).unwrap();
    doc.undo().unwrap(); // Later edits still restore the retained definition.
    assert_eq!(doc.group(created).unwrap().members().len(), 2);
    doc.undo().unwrap();
    assert!(!doc.can_undo());
    assert_eq!(
        doc.objects().map(Object::id).collect::<Vec<_>>(),
        [source, peer]
    );
    assert_eq!(doc.group(created).unwrap().members().len(), 0);
    assert_eq!(
        doc.objects()
            .map(|obj| obj.geometry().clone())
            .collect::<Vec<_>>(),
        [point(2.), point(9.)]
    );
    doc.redo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
    assert_eq!(doc.group(created).unwrap().members().len(), 2);
    doc.redo().unwrap();
    assert!(doc.group(created).is_none());
}

#[test]
fn renewal_skips_deleted_objects_and_does_not_retain_groups_removed_in_the_same_step() {
    let mut doc = Document::default();
    let source = doc.add_geometry(point(1.)).unwrap();
    doc.clear_history().unwrap();
    let mut batch = doc.begin_history_group("Mixed replacement").unwrap();
    batch.keep_created_group_definitions();
    batch.renew_changed_object_order();
    doc.begin_group_transaction(&batch).unwrap();
    doc.transform_objects([source], scale(2.)).unwrap();
    let group = doc.add_empty_group(None).unwrap();
    doc.remove_group(group).unwrap();
    doc.delete_object(source).unwrap();
    doc.commit_group_transaction(&mut batch).unwrap();
    assert_eq!(doc.objects().len(), 0);
    assert_eq!(doc.groups().len(), 0);
    doc.undo().unwrap();
    assert_eq!(doc.object(source).unwrap().geometry(), &point(1.));
    assert_eq!(doc.groups().len(), 0);
    doc.redo().unwrap();
    assert_eq!(doc.objects().len(), 0);
    assert_eq!(doc.groups().len(), 0);
}
