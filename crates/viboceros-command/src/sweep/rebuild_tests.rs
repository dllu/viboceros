use super::*;

#[test]
fn rebuild_options_and_resource_failures_preserve_sources_history_and_redo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    for command in [
        "Line 0,0,0 0,0,5",
        "SelLast",
        "SetObjectName Rail",
        "Circle 0,0,0 2",
        "SelLast",
        "Point 9,8,7",
        "Undo",
    ] {
        registry.execute(&mut doc, command).unwrap();
    }
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let undo = doc.undo_label().map(str::to_owned);
    let redo = doc.redo_label().map(str::to_owned);
    assert!(redo.is_some());
    for count in [1, 2, 512, 513] {
        assert!(
            registry
                .execute(
                    &mut doc,
                    &format!("Sweep1 RailName=Rail Parameters=0 Rebuild={count}")
                )
                .is_err()
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.undo_label(), undo.as_deref());
        assert_eq!(doc.redo_label(), redo.as_deref());
    }
    registry
        .execute(&mut doc, "Sweep1 RailName=Rail Parameters=0 Rebuild=5")
        .unwrap();
    for source in &before {
        assert_eq!(doc.object(source.id()), Some(source));
    }
    let output = doc
        .objects()
        .find_map(|o| match o.geometry() {
            Geometry::Brep(b) => Some(b),
            _ => None,
        })
        .unwrap();
    assert_eq!(output.faces().len(), 1);
    assert_eq!(output.faces()[0].surface().degree_v(), 3);
    assert_eq!(output.faces()[0].surface().control_point_count_v(), 6);
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
}
