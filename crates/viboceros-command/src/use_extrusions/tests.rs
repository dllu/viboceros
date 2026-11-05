use super::*;

#[test]
fn preference_is_shared_by_documents_and_excluded_from_history() {
    let commands = CommandRegistry::with_builtins();
    let mut a = Document::default();
    let mut b = Document::default();
    commands.execute(&mut a, "Point 1,2,3").unwrap();
    commands.execute(&mut a, "SelAll").unwrap();
    commands.execute(&mut a, "Point 10,20,30").unwrap();
    commands.execute(&mut a, "Undo").unwrap();
    let before = a.objects().cloned().collect::<Vec<_>>();
    let selected = a.selected_object_ids().collect::<Vec<_>>();
    commands
        .execute(&mut a, "_-UseExtrusions _Polysurface")
        .unwrap();
    assert!(!commands.use_extrusions());
    assert!(CommandRegistry::with_builtins().use_extrusions());
    assert_eq!(a.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(a.selected_object_ids().collect::<Vec<_>>(), selected);
    assert_eq!(a.redo_label(), Some("Point"));
    commands.execute(&mut b, "Cylinder 0,0,0 2 3").unwrap();
    assert!(
        matches!(b.objects().next().unwrap().geometry(),Geometry::Brep(b) if !b.is_extrusion())
    );
    for input in [
        "UseExtrusions Yes",
        "UseExtrusions Extrusion Polysurface",
        "UseExtrusions Mode=Extrusion",
    ] {
        assert!(commands.execute(&mut b, input).is_err());
        assert!(!commands.use_extrusions());
        assert_eq!(b.undo_label(), Some("Cylinder"));
    }
    commands.execute(&mut b, "UseExtrusions Extrusion").unwrap();
    commands.execute(&mut b, "Undo").unwrap();
    assert!(commands.use_extrusions());
    assert_eq!(b.redo_label(), Some("Cylinder"));
    commands.execute(&mut a, "Redo").unwrap();
    assert_eq!(a.objects().count(), 2);
}
