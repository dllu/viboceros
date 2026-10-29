use super::*;
fn scene() -> (Document, ObjectId, ObjectId) {
    let mut doc = Document::default();
    let commands = CommandRegistry::with_builtins();
    commands.execute(&mut doc, "Line 0,0,0 3,4,0").unwrap();
    let id = doc.objects().next().unwrap().id();
    doc.enable_control_points([id]).unwrap();
    let marker = doc
        .add_geometry(Geometry::Point(Point3::try_new(99., 99., 99.).unwrap()))
        .unwrap();
    doc.undo().unwrap();
    (doc, id, marker)
}
#[test]
fn malformed_point_display_arguments_do_not_consume_redo_or_change_state() {
    let (mut doc, id, _) = scene();
    let commands = CommandRegistry::with_builtins();
    doc.select_objects_direct([id], SelectionMode::Add).unwrap();
    let before = format!("{doc:?}");
    assert!(commands.execute(&mut doc, "PtOffSelected extra").is_err());
    assert!(
        commands
            .object_selection_prompt("PtOffSelected extra")
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
}
#[test]
fn point_display_filter_requires_a_current_selectable_display_not_merely_supported_geometry() {
    let (mut doc, id, _) = scene();
    let object = doc.object(id).unwrap();
    let filter = ObjectSelectionFilter::PointDisplayOwners;
    assert!(filter.accepts_in_document(&doc, object));
    assert!(!filter.accepts_object(object));
    doc.disable_control_points_for_objects([id]);
    assert!(!filter.accepts_in_document(&doc, doc.object(id).unwrap()));
    doc.enable_edit_points([id]).unwrap();
    assert!(filter.accepts_in_document(&doc, doc.object(id).unwrap()));
    doc.set_objects_locked([id], true).unwrap();
    assert!(!filter.accepts_in_document(&doc, doc.object(id).unwrap()));
}
#[test]
fn selected_display_command_records_one_unchanged_undo_step_including_empty_execution() {
    for selected in [false, true] {
        let (mut doc, id, marker) = scene();
        let commands = CommandRegistry::with_builtins();
        if selected {
            doc.select_objects_direct([id], SelectionMode::Add).unwrap();
        }
        let undo = doc.undo_label().map(str::to_owned);
        assert_eq!(
            commands.execute(&mut doc, "PtOffSelected").is_ok(),
            selected
        );
        assert_eq!(doc.undo_label(), Some("PtOffSelected"));
        assert!(!doc.can_redo());
        doc.redo().unwrap();
        assert!(doc.object(marker).is_none());
        assert_eq!(doc.control_point_locations(id).is_none(), selected);
        assert_eq!(doc.is_selected(id), selected);
        let after = format!("{:?}", doc.object(id));
        assert_eq!(doc.undo().unwrap().as_deref(), Some("PtOffSelected"));
        assert_eq!(doc.undo_label(), undo.as_deref());
        assert_eq!(format!("{:?}", doc.object(id)), after);
        assert_eq!(doc.control_point_locations(id).is_none(), selected);
        assert_eq!(doc.redo().unwrap().as_deref(), Some("PtOffSelected"));
    }
}
