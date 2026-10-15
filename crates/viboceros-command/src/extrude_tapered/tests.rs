use super::*;
#[test]
fn tapered_options_are_atomic_and_round_trip() {
    let mut o = TaperedExtrusionOptions::default();
    o.update(&[
        "DraftAngle=12",
        "Solid=Yes",
        "Corners=Round",
        "DeleteInput=Yes",
        "Direction=0,0,8",
    ])
    .unwrap();
    assert_eq!(
        TaperedExtrusionOptions::parse(&o.command_options().split_whitespace().collect::<Vec<_>>())
            .unwrap(),
        o
    );
    o.update(&["FlipAngle"]).unwrap();
    assert_eq!(o.draft_angle, -12.);
    let before = o;
    for words in [
        &["Solid=No", "DraftAngle=90"][..],
        &["Corners=Smooth"][..],
        &["Direction=0,0,0"][..],
        &["Solid=Yes", "Solid=No"][..],
    ] {
        assert!(o.update(words).is_err());
        assert_eq!(o, before);
    }
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn tapered_portable_build_declines_without_document_changes() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Circle 0,0,0 3").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let before = format!("{doc:?}");
    assert!(matches!(
        registry.execute(&mut doc, "ExtrudeCrvTapered 2 DraftAngle=10"),
        Err(CommandError::TaperedExtrusionRequiresNativeKernel)
    ));
    assert_eq!(format!("{doc:?}"), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn tapered_command_stages_deletion_and_replays_history() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Circle 0,0,0 3").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let source = doc.objects().next().unwrap().clone();
    registry
        .execute(
            &mut doc,
            "ExtrudeCrvTapered 2 DraftAngle=10 Solid=Yes DeleteInput=Yes",
        )
        .unwrap();
    assert_eq!(doc.objects().count(), 1);
    assert!(doc.object(source.id()).is_none());
    assert_eq!(doc.selected_object_count(), 1);
    let Geometry::Brep(b) = doc.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(b.is_solid());
    assert_eq!(doc.undo_label(), Some("ExtrudeCrvTapered"));
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(source.id()), Some(&source));
    registry.execute(&mut doc, "Redo").unwrap();
    assert!(doc.object(source.id()).is_none());
}
#[cfg(feature = "native-smlib")]
#[test]
fn tapered_command_failure_preserves_batch_sources_selection_and_redo() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Circle 0,0,0 3").unwrap();
    let bad = NurbsCurve::try_new(
        3,
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1., 0., 1.).unwrap(),
            Point3::try_new(1., 1., 0.).unwrap(),
            Point3::try_new(0., 1., 2.).unwrap(),
        ],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    doc.add_geometry(Geometry::NurbsCurve(bad)).unwrap();
    registry.execute(&mut doc, "Point 50,50,50").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let before = format!("{doc:?}");
    assert!(
        registry
            .execute(
                &mut doc,
                "ExtrudeCrvTapered 2 DraftAngle=10 DeleteInput=Yes"
            )
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
    assert!(doc.can_redo());
}
