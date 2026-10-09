use super::*;
use crate::CommandRegistry;
fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../viboceros-io/tests/fixtures/instances")
        .join(name)
}

#[test]
fn placed_block_import_is_undoable_reports_expansion_and_exports_independent_geometry() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let path = fixture("nested_blocks.3dm");
    let message = registry
        .execute(&mut doc, &format!("Import3dm \"{}\"", path.display()))
        .unwrap();
    assert!(message.contains("expanded 2 block references as independent geometry"));
    assert_eq!(doc.objects().len(), 9);
    let after = doc.objects().cloned().collect::<Vec<_>>();
    let points = doc
        .objects()
        .filter_map(|o| {
            if let Geometry::Point(p) = o.geometry() {
                Some(p.to_array())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(points, [[2., 23., 45.], [-9., -18., -27.], [99., 98., 97.]]);
    assert_eq!(doc.undo_label(), Some("Import3dm"));
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
    let directory =
        std::env::temp_dir().join(format!("viboceros-block-export-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let export = directory.join("expanded.3dm");
    registry
        .execute(&mut doc, &format!("Export3dm \"{}\"", export.display()))
        .unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
    let loaded = viboceros_io::read_3dm_file(&export, doc.tolerance()).unwrap();
    assert_eq!(loaded.expanded_instance_count(), 0);
    assert_eq!(loaded.objects.len(), 9);
    for (actual, expected) in loaded.objects.iter().zip(doc.objects()) {
        assert_eq!(
            document_geometry_from_3dm(actual.geometry.clone()),
            *expected.geometry()
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
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
