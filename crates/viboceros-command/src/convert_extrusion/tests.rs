use super::*;

#[test]
fn extrusion_conversion_validates_before_editing_and_resets_deletion_default() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Box 0,0,0 4,3,0 2").unwrap();
    let id = doc.objects().next().unwrap().id();
    registry.execute(&mut doc, "Point 10,20,30").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = format!("{doc:?}");
    for input in [
        "ConvertExtrusion DeleteInput=Maybe",
        "ConvertExtrusion DeleteInput=Yes DeleteInput=No",
        "ConvertExtrusion Unknown=Yes",
    ] {
        assert!(registry.execute(&mut doc, input).is_err());
        assert_eq!(format!("{doc:?}"), before);
        assert_eq!(doc.redo_label(), Some("Point"));
    }
    registry
        .execute(&mut doc, "ConvertExtrusion DeleteInput=No")
        .unwrap();
    assert_eq!(doc.objects().len(), 2);
    assert!(matches!(doc.object(id).unwrap().geometry(),Geometry::Brep(b) if b.is_extrusion()));
    assert!(
        registry
            .object_selection_prompt("ConvertExtrusion")
            .unwrap()
            .unwrap()
            .options[0]
            .value
    );
    registry.execute(&mut doc, "ConvertExtrusion").unwrap();
    assert_eq!(doc.objects().len(), 2);
    assert!(
        doc.objects()
            .all(|o| matches!(o.geometry(),Geometry::Brep(b) if !b.is_extrusion()))
    );
    let after = format!("{doc:?}");
    assert!(matches!(
        registry.execute(&mut doc, "ConvertExtrusion"),
        Err(CommandError::NoObjectsSelected)
    ));
    assert_eq!(format!("{doc:?}"), after);
}

#[test]
fn extrusion_getter_rejects_explicit_breps_and_filters_unsupported_peers() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Box 0,0,0 4,3,0 2").unwrap();
    let source = doc.objects().next().unwrap().clone();
    let Geometry::Brep(b) = source.geometry() else {
        panic!("box extrusion missing")
    };
    let explicit = doc.add_geometry(Geometry::Brep(b.explicit_brep())).unwrap();
    let point = doc
        .add_geometry(Geometry::Point(Point3::try_new(10., 20., 30.).unwrap()))
        .unwrap();
    let prompt = registry
        .object_selection_prompt("ConvertExtrusion")
        .unwrap()
        .unwrap();
    assert!(prompt.filter.accepts_object(&source));
    for id in [explicit, point] {
        assert!(!prompt.filter.accepts_object(doc.object(id).unwrap()));
    }
    assert_eq!(
        prompt.workflow,
        ObjectSelectionWorkflow::ConfirmAfterSelection
    );
}
