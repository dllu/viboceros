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
    assert!(o.update(&["DeleteInput=Yes", "Thickness=-1"]).is_err());
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
#[test]
fn shell_thickness_uses_model_units_and_converts_explicit_units_atomically() {
    for (units, expected) in [
        (LengthUnitSystem::Millimeters, 10.),
        (LengthUnitSystem::Meters, 0.01),
        (LengthUnitSystem::Inches, 1. / 2.54),
    ] {
        let parsed = ShellInput::parse_with_units(&["Thickness=1cm"], &units).unwrap();
        assert!((parsed.options.thickness - expected).abs() < 1e-12);
        assert_eq!(
            ShellInput::parse_with_units(&["Thickness=2"], &units)
                .unwrap()
                .options
                .thickness,
            2.
        );
        let mut options = parsed.options;
        assert!(
            options
                .update_with_units(&["DeleteInput=No", "Thickness=-1mm"], &units)
                .is_err()
        );
        assert_eq!(options, parsed.options);
    }
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
fn shell_replaces_source_retains_attributes_groups_and_replays_history() {
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
    assert_eq!(doc.selected_objects().count(), 0);
    let result = doc.object(s).unwrap();
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
    assert_eq!(id, s);
    assert_ne!(doc.object(s), Some(&original));
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
    assert_eq!(doc.selected_objects().count(), 0);
    assert_eq!(doc.objects().count(), 2);
    assert_eq!(doc.group(group).unwrap().members().count(), 2);
    for o in doc.objects() {
        let Geometry::Brep(b) = o.geometry() else {
            unreachable!()
        };
        assert!(b.is_solid());
        assert!((b.signed_volume(doc.tolerance()).unwrap().abs() - 48.).abs() < 1e-5);
    }
}

#[cfg(feature = "native-smlib")]
#[test]
fn native_shell_ring_retains_points_past_the_inner_cavity_corner() {
    let mut doc = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap());
    let id = source(&mut doc);
    let t = doc.tolerance();
    let Geometry::Brep(b) = doc.object(id).unwrap().geometry() else {
        unreachable!()
    };
    let top = b
        .faces()
        .iter()
        .position(|f| {
            f.surface()
                .evaluate(
                    f.surface().parameter_at_u(0.5).unwrap(),
                    f.surface().parameter_at_v(0.5).unwrap(),
                )
                .unwrap()
                .z()
                > 5.9
        })
        .unwrap();
    let result = stage_shell(&doc, &BTreeMap::from([(id, vec![top])]), 2.).unwrap();
    let Geometry::Brep(b) = &result[0].1 else {
        unreachable!()
    };
    let p = Point3::try_new(8.75, 2., 6.).unwrap();
    for (i, f) in b.faces().iter().enumerate() {
        let (u, v) = f.surface().closest_parameters(p, t).unwrap();
        let q = f.surface().evaluate(u, v).unwrap();
        if q.distance_to(p).unwrap() < 1e-6 {
            let (a, c) = b.closest_parameters_on_face(i, p, t).unwrap().unwrap();
            assert!(f.surface().evaluate(a, c).unwrap().distance_to(p).unwrap() < 1e-6);
            return;
        }
    }
    panic!("shell ring surface must contain the witness point");
}
