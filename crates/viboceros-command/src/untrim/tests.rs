use super::*;

fn point(x: f64, y: f64) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}

fn trimmed() -> Brep {
    let surface = NurbsSurface::try_bilinear([
        point(0., 0.),
        point(10., 0.),
        point(10., 10.),
        point(0., 10.),
    ])
    .unwrap()
    .try_reparameterized(0.0..=10., 0.0..=10.)
    .unwrap();
    let cut = Polyline3::try_new(
        vec![
            point(3., 3.),
            point(5., 3.),
            point(5., 5.),
            point(3., 5.),
            point(3., 3.),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_native_nurbs()
    .unwrap();
    Brep::try_split_rectangular_surface_face_with_closed_curve(
        surface,
        1.0..=9.,
        1.0..=9.,
        cut,
        false,
        Tolerance::DEFAULT,
    )
    .unwrap()
    .into_iter()
    .next()
    .unwrap()
}

#[test]
fn untrim_preserves_exact_surface_identity_metadata_and_one_step_history() {
    for reversed in [false, true] {
        for post in [false, true] {
            let registry = CommandRegistry::with_builtins();
            let mut doc = Document::default();
            let source = if reversed {
                trimmed().reversed()
            } else {
                trimmed()
            };
            let id = doc
                .add_geometry_with_attributes(
                    Geometry::Brep(source.clone()),
                    ObjectAttributes::on_layer(doc.current_layer_id())
                        .with_name("Patch")
                        .with_object_color(ColorRgb::new(10, 20, 30)),
                )
                .unwrap();
            let peer = doc.add_geometry(Geometry::Point(point(20., 30.))).unwrap();
            let group = doc.add_group(Some("Peers".into()), [id, peer]).unwrap();
            doc.select_objects_direct([id, peer], SelectionMode::Replace)
                .unwrap();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            let input = "_UntrimAll _KeepTrimObjects=_Yes";
            let result = if post {
                registry.execute_postselected(&mut doc, input, Default::default())
            } else {
                registry.execute(&mut doc, input)
            };
            assert_eq!(
                result.unwrap(),
                "Untrimmed 2 loop(s) in 1 surface(s); retained 2 trim curve(s)"
            );
            let output = doc.object(id).unwrap();
            assert_eq!(output.attributes(), before[0].attributes());
            assert_eq!(output.group_ids(), &[group]);
            let Geometry::Brep(untrimmed) = output.geometry() else {
                panic!()
            };
            assert_eq!(untrimmed.faces()[0].surface(), source.faces()[0].surface());
            assert_eq!(untrimmed.faces()[0].is_reversed(), reversed);
            assert!(untrimmed.faces()[0].is_untrimmed(doc.tolerance()).unwrap());
            let objects = doc.objects().cloned().collect::<Vec<_>>();
            assert_eq!(objects.len(), 4);
            assert_eq!(objects[0].id(), peer);
            assert_eq!(objects[3].id(), id);
            for curve in &objects[1..3] {
                assert!(curve.geometry().curve_ref().unwrap().is_closed().unwrap());
                assert!(curve.group_ids().is_empty());
                assert_eq!(curve.attributes().name(), None);
                assert!(!doc.is_selected(curve.id()));
            }
            assert_eq!(doc.is_selected(id), !post);
            assert_eq!(doc.is_selected(peer), !post);
            assert_eq!(doc.undo_label(), Some("UntrimAll"));
            doc.undo().unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            assert_eq!(doc.is_selected(id), !post);
            doc.redo().unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), objects);
        }
    }
}

#[test]
fn invalid_options_are_atomic_and_natural_faces_are_undoable_replacements() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let source = trimmed();
    let id = doc.add_geometry(Geometry::Brep(source)).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    for input in [
        "UntrimAll Bad=Yes",
        "UntrimAll KeepTrimObjects=Maybe",
        "UntrimAll KeepTrimObjects=Yes extra",
        "UntrimAll KeepTrimObjects=Yes KeepTrimObjects=No",
    ] {
        let before = format!("{doc:?}");
        assert!(registry.execute(&mut doc, input).is_err());
        assert_eq!(format!("{doc:?}"), before);
    }
    registry
        .execute(&mut doc, "UntrimAll KeepTrimObjects=No")
        .unwrap();
    registry.execute(&mut doc, "UntrimAll").unwrap();
    assert_eq!(doc.objects().len(), 1);
    assert_eq!(doc.undo_label(), Some("UntrimAll"));
    doc.undo().unwrap();
    assert!(
        matches!(doc.object(id).unwrap().geometry(),Geometry::Brep(b) if b.faces()[0].is_untrimmed(doc.tolerance()).unwrap())
    );
    doc.undo().unwrap();
    assert!(
        matches!(doc.object(id).unwrap().geometry(),Geometry::Brep(b) if b.faces()[0].loops().len()==2)
    );
}

#[test]
fn whole_polysurfaces_are_rejected_and_preferences_are_registry_local() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Brep(
            Brep::try_box(
                CommandContext::default().construction_plane,
                [[0., 10.]; 3],
                doc.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let objects = doc.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        registry.execute(&mut doc, "UntrimAll"),
        Err(CommandError::UnsupportedUntrimAllGeometry)
    ));
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(doc.selected_object_count(), 0);
    let mut prompt = registry
        .object_selection_prompt("UntrimAll")
        .unwrap()
        .unwrap();
    assert!(!prompt.options[0].value);
    assert!(prompt.allows_selection_options());
    prompt.update_options("KeepTrimObjects=Yes").unwrap();
    registry.accept_object_selection_options(&prompt).unwrap();
    assert!(
        registry
            .object_selection_prompt("UntrimAll")
            .unwrap()
            .unwrap()
            .options[0]
            .value
    );
    assert!(
        !CommandRegistry::with_builtins()
            .object_selection_prompt("UntrimAll")
            .unwrap()
            .unwrap()
            .options[0]
            .value
    );
}

#[test]
fn untrim_border_preserves_holes_and_metadata_with_both_retention_and_selection_workflows() {
    for reversed in [false, true] {
        for post in [false, true] {
            for keep in [false, true] {
                let registry = CommandRegistry::with_builtins();
                let mut doc = Document::default();
                let source = if reversed {
                    trimmed().reversed()
                } else {
                    trimmed()
                };
                let layer = doc.add_layer("Source", ColorRgb::new(20, 30, 40)).unwrap();
                let id = doc
                    .add_geometry_with_attributes(
                        Geometry::Brep(source.clone()),
                        ObjectAttributes::on_layer(layer)
                            .with_name("Holey patch")
                            .with_object_color(ColorRgb::new(50, 60, 70)),
                    )
                    .unwrap();
                let peer = doc.add_geometry(Geometry::Point(point(20., 30.))).unwrap();
                doc.add_group(Some("Together".into()), [id, peer]).unwrap();
                doc.select_objects_direct([id, peer], SelectionMode::Replace)
                    .unwrap();
                let before = doc.objects().cloned().collect::<Vec<_>>();
                let input = format!(
                    "UntrimBorder KeepTrimObjects={}",
                    if keep { "Yes" } else { "No" }
                );
                let result = if post {
                    registry.execute_postselected(&mut doc, &input, Default::default())
                } else {
                    registry.execute(&mut doc, &input)
                };
                assert_eq!(
                    result.unwrap(),
                    format!(
                        "Untrimmed 1 loop(s) in 1 surface(s); retained {} trim curve(s)",
                        usize::from(keep)
                    )
                );
                let output = doc.object(id).unwrap();
                assert_eq!(output.attributes(), before[0].attributes());
                assert_eq!(output.group_ids(), before[0].group_ids());
                let Geometry::Brep(restored) = output.geometry() else {
                    panic!()
                };
                assert_eq!(restored.faces()[0].surface(), source.faces()[0].surface());
                assert_eq!(restored.faces()[0].is_reversed(), reversed);
                assert_eq!(restored.faces()[0].loops().len(), 2);
                assert!((restored.area(doc.tolerance()).unwrap() - 96.).abs() < 1e-9);
                for (a, b) in restored.faces()[0].loops()[1]
                    .trims()
                    .iter()
                    .zip(source.faces()[0].loops()[1].trims())
                {
                    assert_eq!(a.curve(), b.curve());
                    assert_eq!(
                        restored.edges()[a.edge().unwrap()].curve(),
                        source.edges()[b.edge().unwrap()].curve()
                    );
                }
                let after = doc.objects().cloned().collect::<Vec<_>>();
                assert_eq!(after.len(), 2 + usize::from(keep));
                assert_eq!(after[0].id(), peer);
                assert_eq!(after.last().unwrap().id(), id);
                if keep {
                    assert!(
                        after[1]
                            .geometry()
                            .curve_ref()
                            .unwrap()
                            .is_closed()
                            .unwrap()
                    );
                    assert!(after[1].group_ids().is_empty());
                    assert_eq!(after[1].attributes().layer_id(), doc.current_layer_id());
                    assert_eq!(after[1].attributes().name(), None);
                    assert!(!doc.is_selected(after[1].id()));
                }
                assert_eq!(doc.is_selected(id), !post);
                assert_eq!(doc.undo_label(), Some("UntrimBorder"));
                doc.undo().unwrap();
                assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
                doc.redo().unwrap();
                assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
            }
        }
    }
}

#[test]
fn border_options_are_independent_and_failed_or_noop_commands_preserve_history_contract() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::Brep(trimmed())).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    for input in [
        "UntrimBorder Unknown=Yes",
        "UntrimBorder KeepTrimObjects=Maybe",
        "UntrimBorder Yes extra",
    ] {
        let before = format!("{doc:?}");
        assert!(registry.execute(&mut doc, input).is_err());
        assert_eq!(format!("{doc:?}"), before);
    }
    registry
        .execute(&mut doc, "UntrimBorder KeepTrimObjects=Yes")
        .unwrap();
    assert!(
        registry
            .object_selection_prompt("UntrimBorder")
            .unwrap()
            .unwrap()
            .options[0]
            .value
    );
    assert!(
        !registry
            .object_selection_prompt("UntrimAll")
            .unwrap()
            .unwrap()
            .options[0]
            .value
    );
    doc.undo().unwrap();
    assert!(
        registry
            .object_selection_prompt("UntrimBorder")
            .unwrap()
            .unwrap()
            .options[0]
            .value
    );
    registry
        .execute(&mut doc, "UntrimBorder KeepTrimObjects=No")
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    registry.execute(&mut doc, "UntrimBorder").unwrap();
    assert!(doc.can_undo());
    doc.undo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    let box_id = doc
        .add_geometry(Geometry::Brep(
            Brep::try_box(
                CommandContext::default().construction_plane,
                [[0., 10.]; 3],
                doc.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    doc.select_objects_direct([box_id], SelectionMode::Replace)
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        registry.execute(&mut doc, "UntrimBorder"),
        Err(CommandError::UnsupportedUntrimBorderGeometry)
    ));
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.selected_object_count(), 0);
}
