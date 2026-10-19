use super::*;
#[test]
fn malformed_intervals_atomic_failures_and_cancel_preserve_redo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Line 0,0,0 10,0,0").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    doc.clear_history().unwrap();
    registry.execute(&mut doc, "SetObjectName Future").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for input in [
        "",
        "Parameter=2",
        "Parameter=NaN,3",
        "Parameter=-1,3",
        "Parameter=2,11",
        "Numeric=2,30,8",
        "2,0,0 7,0,0 Copy=Yes",
        "Parameter=2,7 Locked=Maybe",
    ] {
        assert!(
            registry
                .execute(&mut doc, &format!("DeleteSubCrv {input}"))
                .is_err(),
            "{input}"
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(doc.can_redo());
    }
    registry
        .execute(&mut doc, "DeleteSubCrv 2,0,0 7,0,0 Locked=Backward")
        .unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(doc.can_redo());
    assert_eq!(doc.selected_object_count(), 0);
    registry.execute(&mut doc, "SelAll").unwrap();
    registry
        .execute(&mut doc, "DeleteSubCrv Numeric=2,0,8")
        .unwrap();
    assert!(doc.can_redo());
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.selected_object_count(), 0);
    registry.execute(&mut doc, "SelAll").unwrap();
    registry.execute(&mut doc, "Break Parameter=2,7").unwrap();
    assert_eq!(doc.objects().len(), 2);
    assert_eq!(doc.undo_label(), Some("DeleteSubCrv"));
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.selected_object_count(), 0);
}
