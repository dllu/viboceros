use super::*;
#[test]
fn scriptable_edit_lifecycle_uses_ordinary_commands_and_preserves_cancel() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Point 1,2,3").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry.execute(&mut doc, "Block 0,0,0 part").unwrap();
    let root = doc.objects().next().unwrap().id();
    registry
        .execute(&mut doc, &format!("BlockEdit Open {root}"))
        .unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry.execute(&mut doc, "Move 0,0,0 2,-3,4").unwrap();
    for command in [
        "Open3dm block-edit-protected.3dm",
        "SaveAs block-edit-protected.3dm",
        "Export3dm block-edit-protected.3dm",
        "ExportStl block-edit-protected.stl",
        "ExportStep block-edit-protected.step",
    ] {
        let before = format!("{doc:?}");
        assert!(registry.execute(&mut doc, command).is_err());
        assert_eq!(format!("{doc:?}"), before);
    }
    registry
        .execute(&mut doc, "BlockEdit SaveAndClose")
        .unwrap();
    assert_eq!(doc.undo_label(), Some("BlockEdit"));
    let before = doc.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(&mut doc, &format!("BlockEdit Open {root}"))
        .unwrap();
    registry.execute(&mut doc, "Point 9,8,7").unwrap();
    registry
        .execute(&mut doc, "BlockEdit DiscardAndCancel")
        .unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn scriptable_member_controls_copy_release_and_rebase_in_one_saved_edit() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Point 1,2,3").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry.execute(&mut doc, "Block 0,0,0 part").unwrap();
    let root = doc.objects().next().unwrap().id();
    registry.execute(&mut doc, "Point 4,5,6").unwrap();
    let external = doc.objects().find(|o| o.id() != root).unwrap().id();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(&mut doc, &format!("BlockEdit Open {root}"))
        .unwrap();
    let member = doc.block_edit_objects()[0];
    registry
        .execute(&mut doc, &format!("BlockEdit AddObject {external}"))
        .unwrap();
    doc.select_objects([member], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut doc, "BlockEdit RemoveObject")
        .unwrap();
    registry
        .execute(&mut doc, "BlockEdit SetBasePoint 1,0,0")
        .unwrap();
    registry
        .execute(&mut doc, "BlockEdit SaveAndClose")
        .unwrap();
    assert_eq!(doc.undo_label(), Some("BlockEdit"));
    assert!(doc.object(external).is_some());
    assert!(doc.object(member).is_some());
    let Geometry::BlockInstance(i) = doc.object(root).unwrap().geometry() else {
        panic!()
    };
    assert!(
        matches!(&*i.members()[0].geometry,Geometry::Point(point) if point.to_array()==[3.,5.,6.])
    );
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn nested_context_command_routes_member_paths_and_returns_to_the_root() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Point 1,2,3").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry.execute(&mut doc, "Block 0,0,0 child").unwrap();
    registry.execute(&mut doc, "Point 4,5,6").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry.execute(&mut doc, "Block 0,0,0 parent").unwrap();
    let root = doc.objects().next().unwrap().id();
    registry
        .execute(&mut doc, &format!("BlockEdit Open {root}"))
        .unwrap();
    let child = doc.block_definition_by_name("child").unwrap().id();
    let node = doc
        .block_edit_tree()
        .unwrap()
        .into_iter()
        .find(|n| n.definition == child)
        .unwrap();
    let path = node
        .path
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join("/");
    registry
        .execute(&mut doc, &format!("BlockEdit EditPath {path}"))
        .unwrap();
    assert_eq!(doc.block_edit_definition(), Some(child));
    registry.execute(&mut doc, "SelAll").unwrap();
    registry.execute(&mut doc, "Move 0,0,0 2,-3,4").unwrap();
    registry
        .execute(&mut doc, "BlockEdit EditPath Root")
        .unwrap();
    registry
        .execute(&mut doc, "BlockEdit SaveAndClose")
        .unwrap();
    assert_eq!(doc.undo_label(), Some("BlockEdit"));
}
