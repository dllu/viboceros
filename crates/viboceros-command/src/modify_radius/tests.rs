use super::*;
#[test]
fn sizes_are_strict_atomic_and_share_copy_preferences() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Circle 3,-4,5 2").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    doc.clear_history().unwrap();
    registry.execute(&mut doc, "SetObjectName Future").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for invalid in [
        "",
        "0",
        "-1",
        "NaN",
        "inf",
        "Area=-1",
        "Diameter=1e-324",
        "Radius=2 Diameter=4",
        "2 3",
        "2 Copy=Yes Copy=No",
        "2 Unexpected=Yes",
    ] {
        assert!(
            registry
                .execute(&mut doc, &format!("ModifyRadius {invalid}"))
                .is_err(),
            "{invalid}"
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(doc.can_redo());
        assert_eq!(registry.copy_default("ModifyRadius"), Some(false));
    }
    registry
        .execute(&mut doc, "ModifyRadius Diameter 8 Copy=Yes")
        .unwrap();
    assert_eq!(doc.objects().len(), 2);
    assert_eq!(registry.copy_default("ModifyRadius"), Some(true));
    registry.execute(&mut doc, "Undo").unwrap();
    registry.execute(&mut doc, "ModifyRadius 3").unwrap();
    assert_eq!(doc.objects().len(), 2);
    registry.execute(&mut doc, "Undo").unwrap();
    registry
        .execute(&mut doc, "RememberCopyOptions No")
        .unwrap();
    registry.execute(&mut doc, "ModifyRadius 3").unwrap();
    assert_eq!(doc.objects().len(), 1);
    assert_eq!(registry.copy_default("ModifyRadius"), Some(false));
}
#[test]
fn staged_radius_overflow_retains_sources_selection_and_redo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let circle = |center, radius| {
        Geometry::Circle(
            Circle3::try_new(
                center,
                radius,
                Vector3::try_new(0., 0., 1.)
                    .unwrap()
                    .normalized_nonzero()
                    .unwrap(),
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap(),
        )
    };
    let a = doc
        .add_geometry(circle(Point3::try_new(0., 0., 0.).unwrap(), 2.))
        .unwrap();
    let b = doc
        .add_geometry(circle(Point3::try_new(1e308, 0., 0.).unwrap(), 1.))
        .unwrap();
    doc.select_objects_direct([a, b], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    registry.execute(&mut doc, "SetObjectName Future").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let selected = doc.selected_object_ids().collect::<Vec<_>>();
    assert!(
        registry
            .execute_postselected(&mut doc, "ModifyRadius 1e308", Default::default())
            .is_err()
    );
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selected);
    assert!(doc.can_redo());
}
