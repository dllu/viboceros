use super::*;
fn fixture() -> (Document, CommandRegistry, ObjectId) {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 1,2,3").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry.execute(&mut doc, "Block 0,0,0 part").unwrap();
    registry
        .execute(&mut doc, "Insert part 10,20,30 Scale=2,3,4")
        .unwrap();
    let root = doc.objects().last().unwrap().id();
    doc.select_objects_direct([root], SelectionMode::Replace)
        .unwrap();
    (doc, registry, root)
}
#[test]
fn default_one_and_remembered_automatic_survive_undo_and_cancelled_choices() {
    let (mut doc, registry, root) = fixture();
    let mut prompt = registry
        .object_selection_prompt("BlockResetScale")
        .unwrap()
        .unwrap();
    assert_eq!(prompt.choices[0].value, "One");
    registry.execute(&mut doc, "BlockResetScale").unwrap();
    let Geometry::BlockInstance(i) = doc.object(root).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(
        i.reference().transform().linear_rows(),
        [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]
    );
    doc.undo().unwrap();
    prompt.update_options("Mode=Automatic").unwrap();
    registry.accept_object_selection_options(&prompt).unwrap();
    registry.execute(&mut doc, "BlockResetScale").unwrap();
    let Geometry::BlockInstance(i) = doc.object(root).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(
        i.reference().transform().linear_rows(),
        [[3., 0., 0.], [0., 3., 0.], [0., 0., 3.]]
    );
    doc.undo().unwrap();
    assert_eq!(
        registry
            .object_selection_prompt("BlockResetScale")
            .unwrap()
            .unwrap()
            .choices[0]
            .value,
        "Automatic"
    );
    assert_eq!(
        CommandRegistry::with_builtins()
            .object_selection_prompt("BlockResetScale")
            .unwrap()
            .unwrap()
            .choices[0]
            .value,
        "One"
    );
}
#[test]
fn malformed_modes_preserve_document_and_preferences() {
    let (mut doc, registry, _) = fixture();
    let before = format!("{doc:?}");
    for input in [
        "BlockResetScale Mode=Invalid",
        "BlockResetScale Mode=One Mode=Automatic",
        "BlockResetScale Unexpected=One",
    ] {
        assert!(registry.execute(&mut doc, input).is_err());
        assert_eq!(format!("{doc:?}"), before);
    }
    assert_eq!(
        registry
            .object_selection_prompt("BlockResetScale")
            .unwrap()
            .unwrap()
            .choices[0]
            .value,
        "One"
    );
    registry
        .execute(&mut doc, "BlockResetScale _Mode _Automatic")
        .unwrap();
}
