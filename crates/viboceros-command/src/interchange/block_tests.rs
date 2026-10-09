use super::*;
use crate::CommandRegistry;
fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../viboceros-io/tests/fixtures/instances")
        .join(name)
}

#[test]
fn reopened_definitions_remain_shared_editable_and_import_name_conflicts_are_isolated() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry
        .execute(
            &mut doc,
            &format!("Import3dm \"{}\"", fixture("nested_blocks.3dm").display()),
        )
        .unwrap();
    let roots = doc
        .objects()
        .take(2)
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let definition = doc.block_definition_by_name("Prototype").unwrap().clone();
    let mut members = definition.members().to_vec();
    members[0] = viboceros_document::BlockMember::new(
        viboceros_document::BlockContent::Geometry(
            Geometry::Point(viboceros_geometry::Point3::try_new(2., 4., 6.).unwrap()).into(),
        ),
        members[0].attributes().clone(),
    );
    doc.replace_block_definition_members(definition.id(), members)
        .unwrap();
    for (root, expected) in roots.iter().zip([[-4., 25., 57.], [-8., -16., -24.]]) {
        let Geometry::BlockInstance(instance) = doc.object(*root).unwrap().geometry() else {
            panic!()
        };
        let Geometry::Point(point) = &*instance.members()[0].geometry else {
            panic!()
        };
        assert_eq!(point.to_array(), expected);
    }
    let existing = doc.block_definition_by_name("Prototype").unwrap().id();
    registry
        .execute(
            &mut doc,
            &format!("Import3dm \"{}\"", fixture("nested_blocks.3dm").display()),
        )
        .unwrap();
    assert_eq!(doc.objects().len(), 6);
    assert_eq!(doc.block_definitions().len(), 4);
    assert_eq!(
        doc.block_definition_by_name("Prototype").unwrap().id(),
        existing
    );
    assert!(
        doc.block_definition_by_name("Prototype (Imported 1)")
            .is_some()
    );
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().len(), 3);
    assert_eq!(doc.block_definitions().len(), 2);
}

#[test]
fn hidden_locked_root_state_is_retained_by_document_import() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry
        .execute(
            &mut doc,
            &format!("Import3dm \"{}\"", fixture("hidden_blocks.3dm").display()),
        )
        .unwrap();
    let root = doc
        .objects()
        .find(|object| matches!(object.geometry(), Geometry::BlockInstance(_)))
        .unwrap();
    assert!(!root.attributes().is_visible());
    assert!(root.attributes().is_locked());
    assert!(!doc.is_object_selectable(root.id()));
}

#[test]
fn structural_block_import_is_undoable_and_export_preserves_definitions_and_references() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let path = fixture("nested_blocks.3dm");
    let message = registry
        .execute(&mut doc, &format!("Import3dm \"{}\"", path.display()))
        .unwrap();
    assert!(message.contains("preserved 2 block definitions"));
    assert_eq!(doc.objects().len(), 3);
    assert_eq!(doc.block_definitions().len(), 2);
    let after = doc.objects().cloned().collect::<Vec<_>>();
    let points = doc
        .objects()
        .map(|object| match object.geometry() {
            Geometry::BlockInstance(instance) => {
                let Geometry::Point(point) = &*instance.members()[0].geometry else {
                    panic!()
                };
                point.to_array()
            }
            Geometry::Point(point) => point.to_array(),
            _ => panic!(),
        })
        .collect::<Vec<_>>();
    assert_eq!(points, [[2., 23., 45.], [-9., -18., -27.], [99., 98., 97.]]);
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().len(), 0);
    assert_eq!(doc.block_definitions().len(), 0);
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("structured.3dm");
    registry
        .execute(&mut doc, &format!("Export3dm \"{}\"", file.display()))
        .unwrap();
    let loaded = viboceros_io::read_3dm_file_with_blocks(&file, doc.tolerance()).unwrap();
    assert_eq!(loaded.definitions.len(), 2);
    assert_eq!(loaded.objects.len(), 3);
    let flat = viboceros_io::read_3dm_file(&file, doc.tolerance()).unwrap();
    assert_eq!(flat.expanded_instance_count(), 2);
    assert_eq!(flat.objects.len(), 9);
    let (opened, _, _) = open_3dm_with_named_views(file.to_str().unwrap()).unwrap();
    assert_eq!(opened.block_definitions().len(), 2);
    assert_eq!(opened.objects().len(), 3);
    assert!(
        opened
            .objects()
            .filter(|object| matches!(object.geometry(), Geometry::BlockInstance(_)))
            .count()
            == 2
    );
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
}
#[test]
fn malformed_block_graphs_leave_document_and_redo_history_unchanged() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Point 7,8,9").unwrap();
    registry.execute(&mut doc, "Point 1,2,3").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let before = format!("{doc:?}");
    for name in [
        "missing_block.3dm",
        "cyclic_block.3dm",
        "projective_block.3dm",
        "singular_block.3dm",
        "branching_empty_blocks.3dm",
    ] {
        assert!(
            registry
                .execute(
                    &mut doc,
                    &format!("Import3dm \"{}\"", fixture(name).display())
                )
                .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
    }
    assert!(doc.can_redo());
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().len(), 2);
}
