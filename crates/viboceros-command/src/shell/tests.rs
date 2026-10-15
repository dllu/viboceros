use super::*;
fn source(doc: &mut Document) -> ObjectId {
    doc.add_geometry(Geometry::Brep(
        Brep::try_box(
            CommandContext::default().construction_plane,
            [[0., 10.], [0., 8.], [0., 6.]],
            doc.tolerance(),
        )
        .unwrap(),
    ))
    .unwrap()
}
#[test]
fn shell_options_are_atomic_and_invalid_face_sets_decline() {
    let mut o = ShellOptions::default();
    o.update(&["Thickness=2", "DeleteInput=No"]).unwrap();
    let before = o;
    assert!(o.update(&["DeleteInput=Yes", "Thickness=0"]).is_err());
    assert_eq!(o, before);
    assert!(ShellInput::parse(&["Face=0", "Faces=1"]).is_err());
    let mut doc = Document::default();
    let s = source(&mut doc);
    doc.select_objects_direct([s], SelectionMode::Replace)
        .unwrap();
    assert!(shell_targets(&doc, &SurfaceFaces::All).is_err());
    assert_eq!(
        shell_targets(&doc, &SurfaceFaces::References(vec![(s, 0), (s, 0)])).unwrap()[&s],
        vec![0]
    );
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn shell_portable_decline_preserves_document_and_history() {
    let mut doc = Document::default();
    let s = source(&mut doc);
    let before = format!("{doc:?}");
    assert!(matches!(
        CommandRegistry::with_builtins()
            .execute(&mut doc, &format!("Shell Thickness=1 FaceRefs={s}:0")),
        Err(CommandError::ShellRequiresNativeKernel)
    ));
    assert_eq!(format!("{doc:?}"), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn shell_deletes_source_copies_attributes_groups_and_replays_history() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let s = source(&mut doc);
    doc.set_object_names([(s, Some("wall source".into()))])
        .unwrap();
    let group = doc.add_group(None, [s]).unwrap();
    let original = doc.object(s).unwrap().clone();
    registry
        .execute(&mut doc, &format!("Shell Thickness=1 FaceRefs={s}:0,{s}:0"))
        .unwrap();
    assert!(doc.object(s).is_none());
    let result = doc.selected_objects().next().unwrap();
    let id = result.id();
    assert_eq!(result.attributes(), original.attributes());
    assert_eq!(
        doc.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![id]
    );
    assert!(matches!(result.geometry(),Geometry::Brep(b)if b.is_solid()));
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(s), Some(&original));
    registry.execute(&mut doc, "Redo").unwrap();
    assert!(doc.object(s).is_none());
    assert!(doc.object(id).is_some());
}
#[cfg(feature = "native-smlib")]
#[test]
fn shell_preserves_input_when_requested_and_later_failure_rolls_back() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let a = source(&mut doc);
    let b = doc
        .add_geometry(Geometry::Brep(
            Brep::try_box(
                CommandContext::default().construction_plane,
                [[0., 0.5], [0., 0.5], [0., 0.5]],
                doc.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let original = doc.object(a).unwrap().clone();
    registry
        .execute(
            &mut doc,
            &format!("Shell Thickness=1 DeleteInput=No FaceRefs={a}:0"),
        )
        .unwrap();
    assert_eq!(doc.object(a), Some(&original));
    assert_eq!(doc.objects().count(), 3);
    registry.execute(&mut doc, "Undo").unwrap();
    let before = format!("{doc:?}");
    assert!(
        registry
            .execute(
                &mut doc,
                &format!("Shell Thickness=1 FaceRefs={a}:0,{b}:0 DeleteInput=Yes")
            )
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
    assert!(doc.can_redo());
}

#[cfg(feature = "native-smlib")]
#[test]
fn shell_disconnected_walls_inherit_source_groups_as_separate_solids() {
    let mut doc = Document::default();
    let id = source(&mut doc);
    let group = doc.add_group(None, [id]).unwrap();
    let Geometry::Brep(b) = doc.object(id).unwrap().geometry() else {
        unreachable!()
    };
    let mut removed = Vec::new();
    for (i, f) in b.faces().iter().enumerate() {
        let s = f.surface();
        let c = s
            .evaluate(
                s.parameter_at_u(0.5).unwrap(),
                s.parameter_at_v(0.5).unwrap(),
            )
            .unwrap();
        if c.y().abs() < 1e-8
            || (c.y() - 8.).abs() < 1e-8
            || c.z().abs() < 1e-8
            || (c.z() - 6.).abs() < 1e-8
        {
            removed.push(format!("{id}:{i}"));
        }
    }
    CommandRegistry::with_builtins()
        .execute(
            &mut doc,
            &format!("Shell Thickness=1 FaceRefs={}", removed.join(",")),
        )
        .unwrap();
    assert_eq!(doc.selected_objects().count(), 2);
    assert_eq!(doc.group(group).unwrap().members().count(), 2);
    for o in doc.selected_objects() {
        let Geometry::Brep(b) = o.geometry() else {
            unreachable!()
        };
        assert!(b.is_solid());
        assert!((b.signed_volume(doc.tolerance()).unwrap().abs() - 48.).abs() < 1e-5);
    }
}
