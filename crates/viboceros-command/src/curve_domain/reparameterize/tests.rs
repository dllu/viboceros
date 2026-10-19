use super::*;

fn setup() -> (CommandRegistry, Document, ObjectId) {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let brep = Brep::try_box(
        CommandContext::default().construction_plane,
        [[0., 4.], [0., 6.], [0., 2.]],
        doc.tolerance(),
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::Brep(brep)).unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    registry.execute(&mut doc, "Group Charts").unwrap();
    registry
        .execute(&mut doc, "SetObjectName ChartSource")
        .unwrap();
    registry
        .execute(&mut doc, "SetUserText ObjectCode retained")
        .unwrap();
    doc.set_object_geometry_user_text([id], "GeometryCode", Some("retained"))
        .unwrap();
    doc.clear_history().unwrap();
    (registry, doc, id)
}

#[test]
fn individual_polysurface_face_edits_preserve_shared_edges_metadata_selection_and_history() {
    for face in 0..6 {
        for automatic in [false, true] {
            let (registry, mut doc, id) = setup();
            let before = doc.object(id).unwrap().clone();
            let Geometry::Brep(original) = before.geometry() else {
                panic!()
            };
            let values = if automatic { "Automatic" } else { "-2,8,10,30" };
            registry
                .execute(&mut doc, &format!("Reparameterize Face={face} {values}"))
                .unwrap();
            let after = doc.object(id).unwrap().clone();
            let Geometry::Brep(result) = after.geometry() else {
                panic!()
            };
            assert_eq!(result.vertices(), original.vertices());
            assert_eq!(result.edges(), original.edges());
            assert!(result.is_solid());
            assert_eq!(before.attributes(), after.attributes());
            assert_eq!(before.group_ids(), after.group_ids());
            assert_eq!(before.geometry_user_text(), after.geometry_user_text());
            assert_eq!(
                doc.selected_objects().map(|o| o.id()).collect::<Vec<_>>(),
                vec![id]
            );
            for (i, (a, b)) in result.faces().iter().zip(original.faces()).enumerate() {
                if i != face {
                    assert_eq!(a, b);
                    continue;
                }
                let domains = if automatic {
                    let [w, h] = b.surface().estimated_size().unwrap();
                    [0. ..=w, 0. ..=h]
                } else {
                    [-2. ..=8., 10. ..=30.]
                };
                assert_eq!(a.surface().domain_u(), domains[0]);
                assert_eq!(a.surface().domain_v(), domains[1]);
                for u in [0., 0.25, 0.75, 1.] {
                    for v in [0., 0.5, 1.] {
                        let at = a.surface();
                        let bt = b.surface();
                        let p = at
                            .evaluate(at.parameter_at_u(u).unwrap(), at.parameter_at_v(v).unwrap())
                            .unwrap();
                        let q = bt
                            .evaluate(bt.parameter_at_u(u).unwrap(), bt.parameter_at_v(v).unwrap())
                            .unwrap();
                        assert!(p.distance_to(q).unwrap() < 1e-8);
                    }
                }
            }
            // Some box domains already equal their automatic size: a no-op
            // need not consume history. Explicit chart edits always do.
            if before.geometry() != after.geometry() {
                assert_eq!(doc.undo_label(), Some("Reparameterize"));
                registry.execute(&mut doc, "Undo").unwrap();
                assert_eq!(doc.object(id).unwrap(), &before);
                assert!(!doc.can_undo());
                registry.execute(&mut doc, "Redo").unwrap();
                assert_eq!(doc.object(id).unwrap(), &after);
            } else {
                assert!(!doc.can_undo());
            }
        }
    }
}

#[test]
fn invalid_ambiguous_restricted_and_noop_face_requests_preserve_redo() {
    let (registry, mut doc, id) = setup();
    registry
        .execute(&mut doc, "Reparameterize Face=0 -2,8,10,30")
        .unwrap();
    let edited = doc.object(id).unwrap().clone();
    registry.execute(&mut doc, "Undo").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for args in [
        "Automatic",
        "Face=6 Automatic",
        "Face=0 Face=0 Automatic",
        "Face=-1 Automatic",
        "Face=0 0,0,0,1",
        "Face=0 0,1",
        "Face=0 0,1,0,NaN",
        "Face=0 Automatic extra",
    ] {
        assert!(
            registry
                .execute(&mut doc, &format!("Reparameterize {args}"))
                .is_err(),
            "{args}"
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(doc.can_redo());
        assert!(!doc.can_undo());
    }
    let Geometry::Brep(b) = doc.object(id).unwrap().geometry() else {
        panic!()
    };
    let s = b.faces()[0].surface();
    let u = s.domain_u();
    let v = s.domain_v();
    registry
        .execute(
            &mut doc,
            &format!(
                "Reparameterize Surface={id} Face=0 {},{},{},{}",
                u.start(),
                u.end(),
                v.start(),
                v.end()
            ),
        )
        .unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(doc.can_redo());
    assert!(!doc.can_undo());
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.object(id).unwrap(), &edited);
    for command in ["Hide", "Lock"] {
        let (registry, mut doc, id) = setup();
        registry.execute(&mut doc, command).unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let label = doc.undo_label().map(str::to_owned);
        assert!(
            registry
                .execute(
                    &mut doc,
                    &format!("Reparameterize Surface={id} Face=0 -2,8,10,30")
                )
                .is_err()
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.undo_label(), label.as_deref());
    }
}

#[test]
fn curve_and_surface_parameter_edits_retain_geometry_user_text() {
    for source in ["Circle 0,0,0 3", "SrfPt 0,0,0 4,0,0 4,6,0 0,6,0"] {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        registry.execute(&mut doc, source).unwrap();
        let id = doc.objects().next().unwrap().id();
        doc.select_object(id, SelectionMode::Replace).unwrap();
        doc.set_object_geometry_user_text([id], "Code", Some("retained"))
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.object(id).unwrap().clone();
        registry
            .execute(
                &mut doc,
                if source.starts_with("Circle") {
                    "Reparameterize -2,8"
                } else {
                    "Reparameterize -2,8,10,30"
                },
            )
            .unwrap();
        assert_eq!(
            doc.object(id).unwrap().geometry_user_text(),
            before.geometry_user_text()
        );
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.object(id).unwrap(), &before);
    }
}

#[test]
fn object_reference_edits_an_unselected_curve_without_changing_other_selection() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Line 0,0,0 3,4,0").unwrap();
    let id = doc.objects().next().unwrap().id();
    registry.execute(&mut doc, "Circle 0,0,0 3").unwrap();
    let other = doc.objects().find(|o| o.id() != id).unwrap().id();
    doc.select_object(other, SelectionMode::Replace).unwrap();
    doc.set_object_geometry_user_text([id], "Code", Some("retained"))
        .unwrap();
    doc.clear_history().unwrap();
    let before = doc.object(id).unwrap().clone();
    let untouched = doc.object(other).unwrap().clone();
    registry
        .execute(&mut doc, &format!("Reparameterize Object={id} -2,8"))
        .unwrap();
    assert_eq!(
        doc.object(id)
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap()
            .domain(),
        -2. ..=8.
    );
    assert_eq!(
        doc.object(id).unwrap().geometry_user_text(),
        before.geometry_user_text()
    );
    assert!(!doc.is_selected(id));
    assert!(doc.is_selected(other));
    assert_eq!(doc.object(other).unwrap(), &untouched);
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(id).unwrap(), &before);
    assert!(!doc.can_undo());
    for args in [
        format!("Surface={id} -2,8"),
        format!("Object={id} Surface={id} -2,8"),
        format!("Object={id} Object={id} -2,8"),
        format!("Object={id} Face=0 -2,8"),
    ] {
        assert!(
            registry
                .execute(&mut doc, &format!("Reparameterize {args}"))
                .is_err()
        );
        assert_eq!(doc.object(id).unwrap(), &before);
        assert!(doc.can_redo());
        assert!(!doc.can_undo());
    }
    registry.execute(&mut doc, "Redo").unwrap();
    registry
        .execute(&mut doc, &format!("Reparameterize Object={id} Automatic"))
        .unwrap();
    assert_eq!(
        doc.object(id)
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap()
            .domain(),
        0. ..=5.
    );
    assert_eq!(doc.object(other).unwrap(), &untouched);
}

#[test]
fn automatic_size_handles_an_interior_collapsed_isocurve_and_rejects_zero_width_atomically() {
    let surface = |end: Real| {
        NurbsSurface::try_new(
            1,
            1,
            2,
            2,
            [[0., 0., 0.], [end, 0., 0.], [0., 1., 0.], [-end, 1., 0.]]
                .into_iter()
                .map(|p| Point3::try_from(p).unwrap())
                .collect(),
            vec![0., 0., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap()
    };
    let tolerance = Tolerance::try_new(1e-8, 1e-12, 1e-8).unwrap();
    let size = automatic_surface_size(&surface(1.), tolerance).unwrap();
    assert!((size[0] - 0.6).abs() < 1e-14);
    assert!(size[1] > 1. && size[1] < 5_f64.sqrt());
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::new(tolerance);
    let id = doc
        .add_geometry(Geometry::NurbsSurface(surface(0.)))
        .unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    doc.clear_history().unwrap();
    registry.execute(&mut doc, "SetObjectName changed").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let before = doc.object(id).unwrap().clone();
    assert!(matches!(
        registry.execute(&mut doc, &format!("Reparameterize Surface={id} Automatic")),
        Err(CommandError::Geometry(_))
    ));
    assert_eq!(doc.object(id).unwrap(), &before);
    assert!(!doc.can_undo());
    assert!(doc.can_redo());
}

fn block_document() -> (CommandRegistry, Document, ObjectId) {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/reparameterize_blocks/line_explicit.3dm");
    registry
        .execute(&mut doc, &format!("Open \"{}\"", file.display()))
        .unwrap();
    let root = doc
        .objects()
        .find(|o| o.attributes().name() == Some("Root0"))
        .unwrap()
        .id();
    doc.clear_selection();
    doc.clear_history().unwrap();
    (registry, doc, root)
}
#[test]
fn block_member_paths_copy_placed_geometry_with_fresh_attributes_and_preserved_definitions() {
    let (registry, mut doc, root) = block_document();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let definitions = doc.block_definitions().cloned().collect::<Vec<_>>();
    for bad in [
        "Member=",
        "Member=2",
        "Member=0/0",
        "Member=-1",
        "Member=0 Member=1",
    ] {
        assert!(
            registry
                .execute(
                    &mut doc,
                    &format!("Reparameterize Object={root} {bad} Automatic")
                )
                .is_err()
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
    }
    assert!(
        registry
            .execute(
                &mut doc,
                &format!("Reparameterize Surface={root} Member=0 Automatic")
            )
            .is_err()
    );
    registry
        .execute(
            &mut doc,
            &format!("Reparameterize Object={root} Member=1 -2 8"),
        )
        .unwrap();
    for source in &before {
        assert_eq!(doc.object(source.id()), Some(source));
    }
    assert_eq!(
        doc.block_definitions().cloned().collect::<Vec<_>>(),
        definitions
    );
    let copy = doc
        .objects()
        .find(|o| !before.iter().any(|s| s.id() == o.id()))
        .unwrap()
        .clone();
    assert_eq!(copy.geometry().curve_ref().unwrap().domain(), -2. ..=8.);
    let curve = copy.geometry().curve_ref().unwrap();
    assert!(
        curve
            .evaluate(-2.)
            .unwrap()
            .distance_to(Point3::try_new(30., 0., 0.).unwrap())
            .unwrap()
            < 1e-12
    );
    assert_eq!(
        copy.attributes(),
        &ObjectAttributes::on_layer(doc.current_layer_id())
    );
    assert!(copy.group_ids().is_empty());
    assert!(!doc.is_selected(copy.id()));
    assert_eq!(
        copy.geometry_user_text()
            .get("GeometryCode")
            .map(String::as_str),
        Some("alternate")
    );
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!doc.can_undo());
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.object(copy.id()), Some(&copy));
}
#[test]
fn staged_block_batches_reject_later_bad_paths_and_stale_sources_without_partial_copies() {
    for change in [
        "path",
        "prototype_visibility",
        "root_visibility",
        "layer",
        "metadata",
        "tolerance",
    ] {
        let (_, mut doc, root) = block_document();
        let references = [0, 1].map(|member| ReparameterizeReference {
            object: root,
            member: Some(vec![member]),
            face: None,
        });
        let before = doc.objects().cloned().collect::<Vec<_>>();
        if change == "path" {
            let refs = [
                references[0].clone(),
                ReparameterizeReference {
                    member: Some(vec![2]),
                    ..references[1].clone()
                },
            ];
            assert!(
                ReparameterizeSelection::prepare(&doc, refs, ReparameterizeDomains::Automatic)
                    .is_err()
            );
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            continue;
        }
        let selection = ReparameterizeSelection::prepare(
            &doc,
            references,
            ReparameterizeDomains::Explicit(vec![-2., 8.]),
        )
        .unwrap();
        match change {
            "prototype_visibility" => {
                let Geometry::BlockInstance(instance) = doc.object(root).unwrap().geometry() else {
                    panic!()
                };
                let layer = instance.members()[0].attributes.layer_id();
                doc.set_layer_visibility(layer, false).unwrap();
            }
            "root_visibility" => {
                doc.set_objects_visibility([root], false).unwrap();
            }
            "layer" => {
                let layer = doc
                    .add_layer("Another", viboceros_document::ColorRgb::new(1, 2, 3))
                    .unwrap();
                doc.set_current_layer(layer).unwrap();
            }
            "metadata" => {
                doc.set_object_geometry_user_text([root], "Changed", Some("yes"))
                    .unwrap();
            }
            _ => {
                doc.set_tolerance(Tolerance::try_new(1e-5, 1e-12, 1e-8).unwrap());
            }
        }
        let before_commit = doc.objects().cloned().collect::<Vec<_>>();
        let label = doc.undo_label().map(str::to_owned);
        assert!(selection.commit(&mut doc).is_err(), "{change}");
        assert_eq!(
            doc.objects().cloned().collect::<Vec<_>>(),
            before_commit,
            "{change}"
        );
        assert_eq!(doc.undo_label(), label.as_deref());
    }
}
