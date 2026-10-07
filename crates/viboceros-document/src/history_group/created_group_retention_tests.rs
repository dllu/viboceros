use super::*;

fn setup() -> (Document, ObjectId) {
    let mut d = Document::default();
    let id = d
        .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
        .unwrap();
    d.add_group(Some("source".into()), [id]).unwrap();
    d.clear_history().unwrap();
    (d, id)
}
#[test]
fn ordinary_copy_retains_definitions_across_undo_without_changing_membership_replay() {
    let (mut d, id) = setup();
    d.begin_transaction("copy").unwrap();
    let copy = d
        .copy_object_geometries_with_groups(
            [(id, Geometry::Point(Point3::try_new(1., 0., 0.).unwrap()))],
            CopyGroupPolicy::Preserve,
        )
        .unwrap()[0];
    let copied_group = d.object(copy).unwrap().group_ids()[0];
    d.retain_created_group_definitions_on_undo().unwrap();
    assert!(d.commit_transaction().unwrap());
    d.undo().unwrap();
    assert!(d.object(copy).is_none());
    assert_eq!(d.group(copied_group).unwrap().members().len(), 0);
    d.redo().unwrap();
    assert_eq!(d.object(copy).unwrap().group_ids(), [copied_group]);
}
#[test]
fn retention_request_does_not_survive_rollback_or_create_an_empty_undo_step() {
    let (mut d, id) = setup();
    let before = format!("{d:?}");
    d.begin_transaction("rejected copy").unwrap();
    d.copy_object_geometries_with_groups(
        [(id, Geometry::Point(Point3::try_new(1., 0., 0.).unwrap()))],
        CopyGroupPolicy::Preserve,
    )
    .unwrap();
    d.retain_created_group_definitions_on_undo().unwrap();
    d.rollback_transaction().unwrap();
    assert_eq!(format!("{d:?}"), before);
    d.begin_transaction("empty").unwrap();
    d.retain_created_group_definitions_on_undo().unwrap();
    d.release_command_selection_on_history_replay([id]).unwrap();
    assert!(!d.commit_transaction().unwrap());
    assert!(!d.can_undo());
    assert!(matches!(
        d.retain_created_group_definitions_on_undo(),
        Err(DocumentError::NoActiveTransaction)
    ));
}
