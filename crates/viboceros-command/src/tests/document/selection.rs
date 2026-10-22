//! document selection command regressions.
use super::*;

#[test]
fn selection_commands_drive_group_ungroup_and_delete() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Point 0,0,0").unwrap();
    registry.execute(&mut document, "Point 1,0,0").unwrap();
    let history_label = document.undo_label().map(str::to_owned);

    assert_eq!(
        registry.execute(&mut document, "SelAll").unwrap(),
        "Selected 2 object(s)"
    );
    assert_eq!(
        registry.execute(&mut document, "SelNone").unwrap(),
        "Deselected 2 object(s)"
    );
    assert_eq!(
        registry.execute(&mut document, "Invert").unwrap(),
        "Selected 2 object(s)"
    );
    assert_eq!(document.undo_label(), history_label.as_deref());

    registry.execute(&mut document, "Group Pair").unwrap();
    assert_eq!(document.group_by_name("Pair").unwrap().members().len(), 2);
    let first = document.objects().next().unwrap().id();
    document.clear_selection();
    document
        .select_object(first, SelectionMode::Replace)
        .unwrap();
    assert_eq!(document.selected_object_count(), 2);

    registry.execute(&mut document, "Ungroup").unwrap();
    assert_eq!(document.groups().len(), 1);
    assert!(document.groups().all(|group| group.members().len() == 0));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.groups().len(), 1);

    registry.execute(&mut document, "Delete").unwrap();
    assert_eq!(document.objects().len(), 0);
    assert_eq!(document.groups().len(), 1);
    assert!(document.groups().all(|group| group.members().len() == 0));
    assert_eq!(document.undo_label(), Some("Delete"));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.groups().len(), 1);
}

#[test]
fn sel_last_remembers_valid_options_independently_of_sel_prev() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let ids = (0..3)
        .map(|i| {
            document
                .add_geometry(Geometry::Point(
                    Point3::try_new(i as f64, 0.0, 0.0).unwrap(),
                ))
                .unwrap()
        })
        .collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);
    registry
        .execute(&mut document, "SelLast DeselectOthersBeforeSelect=No")
        .unwrap();
    assert!(
        registry
            .execute(&mut document, "SelLast DeselectOthersBeforeSelect=Maybe")
            .is_err()
    );
    registry
        .execute(&mut document, "SelPrev DeselectOthersBeforeSelect=Yes")
        .unwrap();
    document
        .select_object(ids[0], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut document, "SelLast").unwrap();
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        BTreeSet::from([ids[0], ids[2]])
    );
    CommandRegistry::with_builtins()
        .execute(&mut document, "SelLast")
        .unwrap();
    assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [ids[2]]);
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn sel_prev_remembers_only_valid_options_and_isolates_registries() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let ids = (0..3)
        .map(|i| {
            document
                .add_geometry(Geometry::Point(
                    Point3::try_new(i as f64, 0.0, 0.0).unwrap(),
                ))
                .unwrap()
        })
        .collect::<Vec<_>>();
    document
        .select_object(ids[0], SelectionMode::Replace)
        .unwrap();
    document.clear_selection();
    document.select_object(ids[1], SelectionMode::Add).unwrap();
    let history = document.undo_label().map(str::to_owned);
    registry
        .execute(&mut document, "SelPrev DeselectOthersBeforeSelect=No")
        .unwrap();
    assert!(
        registry
            .execute(&mut document, "SelPrev DeselectOthersBeforeSelect=Maybe")
            .is_err()
    );
    document.clear_selection();
    document.select_object(ids[2], SelectionMode::Add).unwrap();
    registry.execute(&mut document, "SelPrev").unwrap();
    assert_eq!(
        document.selected_object_ids().collect::<Vec<_>>(),
        [ids[2], ids[1], ids[0]]
    );
    CommandRegistry::with_builtins()
        .execute(&mut document, "SelPrev")
        .unwrap();
    assert_eq!(
        document.selected_object_ids().collect::<Vec<_>>(),
        [ids[1], ids[0]]
    );
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn action_order_selection_commands_match_rhino_replace_add_and_toggle_modes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for x in 0..4 {
        registry
            .execute(&mut document, &format!("Point {x},0,0"))
            .unwrap();
    }
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let selected = |document: &Document| {
        document
            .selected_object_ids()
            .collect::<BTreeSet<ObjectId>>()
    };
    let history = document.undo_label().map(str::to_owned);

    document
        .select_object(ids[0], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "SelLast").unwrap(),
        "Selection contains 1 object(s)"
    );
    assert_eq!(selected(&document), BTreeSet::from([ids[3]]));
    registry.execute(&mut document, "SelPrev").unwrap();
    assert_eq!(selected(&document), BTreeSet::from([ids[0]]));
    registry.execute(&mut document, "SelPrev").unwrap();
    assert_eq!(selected(&document), BTreeSet::from([ids[3]]));

    document
        .select_object(ids[0], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "SelLast DeselectOthersBeforeSelect=No",)
            .unwrap(),
        "Selection contains 2 object(s)"
    );
    assert_eq!(selected(&document), BTreeSet::from([ids[0], ids[3]]));

    document
        .select_objects([ids[0], ids[1]], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut document, "SelNone").unwrap();
    document.select_object(ids[2], SelectionMode::Add).unwrap();
    registry.execute(&mut document, "SelPrev").unwrap();
    assert_eq!(selected(&document), BTreeSet::from([ids[0], ids[1]]));
    registry.execute(&mut document, "SelPrev").unwrap();
    assert_eq!(selected(&document), BTreeSet::from([ids[2]]));

    document
        .select_objects([ids[0], ids[1]], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut document, "SelNone").unwrap();
    document.select_object(ids[2], SelectionMode::Add).unwrap();
    registry
        .execute(&mut document, "SelPrev DeselectOthersBeforeSelect No")
        .unwrap();
    assert_eq!(
        selected(&document),
        BTreeSet::from([ids[0], ids[1], ids[2]])
    );

    let before = selected(&document);
    assert!(matches!(
        registry.execute(&mut document, "SelLast DeselectOthersBeforeSelect=Maybe"),
        Err(CommandError::Usage(
            "SelLast [DeselectOthersBeforeSelect=Yes|No]"
        ))
    ));
    assert_eq!(selected(&document), before);
    assert_eq!(document.undo_label(), history.as_deref());
}
