use super::*;

fn line(a: [f64; 3], b: [f64; 3]) -> Geometry {
    Geometry::Line(
        LineSegment::try_new(
            Point3::try_from(a).unwrap(),
            Point3::try_from(b).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
    )
}

fn run(
    registry: &CommandRegistry,
    document: &mut Document,
    command: &str,
    post: bool,
) -> Result<String, CommandError> {
    if post {
        registry.execute_postselected(document, command, Default::default())
    } else {
        registry.execute(document, command)
    }
}

#[test]
fn closure_completion_check_is_read_only_and_copy_keeps_only_participating_sources_selected() {
    let mut doc = Document::default();
    let ids = [
        ([0., 0., 0.], [1., 0., 0.]),
        ([8., 0., 0.], [9., 0., 0.]),
        ([1., 0., 0.], [1., 2., 0.]),
        ([1., 2., 0.], [0., 0., 0.]),
    ]
    .map(|(a, b)| doc.add_geometry(line(a, b)).unwrap());
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 20,0,0").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let prompt = registry
        .object_selection_prompt("JoinCopy")
        .unwrap()
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for (index, id) in ids.into_iter().enumerate() {
        doc.select_objects_direct([id], SelectionMode::Add).unwrap();
        assert_eq!(
            registry.object_selection_complete(&doc, &prompt).unwrap(),
            index == 3
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.redo_label(), Some("Point"));
    }
    registry
        .execute_postselected(&mut doc, "JoinCopy", Default::default())
        .unwrap();
    for (index, id) in ids.into_iter().enumerate() {
        assert_eq!(doc.is_selected(id), index != 1);
    }
    let output = doc.objects().find(|o| !ids.contains(&o.id())).unwrap();
    assert!(!doc.is_selected(output.id()));
    assert_eq!(*output.geometry().curve_ref().unwrap().domain().start(), 0.);
}

#[test]
fn join_copy_preserves_exact_sources_attributes_groups_and_two_history_cycles() {
    for post in [false, true] {
        for mesh in [false, true] {
            let mut doc = Document::default();
            let layer = doc.add_layer("Seed", ColorRgb::BLACK).unwrap();
            let attrs = ObjectAttributes::on_layer(layer)
                .with_name("seed")
                .with_object_color(ColorRgb::new(11, 22, 33));
            let first = doc
                .add_geometry_with_attributes(
                    if mesh {
                        quad(0.)
                    } else {
                        line([0., 0., 0.], [1., 0., 0.])
                    },
                    attrs.clone(),
                )
                .unwrap();
            let second = doc
                .add_geometry(if mesh {
                    quad(2.)
                } else {
                    line([1., 0., 0.], [1., 2., 0.])
                })
                .unwrap();
            let peer = doc
                .add_geometry(Geometry::Point(Point3::try_new(8., 0., 0.).unwrap()))
                .unwrap();
            let group = doc
                .add_group(Some("Shared".into()), [first, second, peer])
                .unwrap();
            doc.select_objects_direct([first, second], SelectionMode::Replace)
                .unwrap();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            let before_members = doc.group(group).unwrap().members().collect::<Vec<_>>();
            let registry = CommandRegistry::with_builtins();
            run(&registry, &mut doc, "JoinCopy", post).unwrap();
            assert_eq!(doc.objects().len(), 4);
            for source in &before {
                assert_eq!(doc.object(source.id()), Some(source));
            }
            assert!(!doc.is_selected(peer));
            let output = doc
                .objects()
                .find(|o| !before.iter().any(|b| b.id() == o.id()))
                .unwrap();
            assert_eq!(output.attributes(), &attrs);
            assert_eq!(output.group_ids(), [group]);
            for id in [first, second, output.id()] {
                assert_eq!(doc.is_selected(id), !post || (mesh && id != output.id()));
            }
            assert_eq!(doc.undo_label(), Some("JoinCopy"));
            let after = doc.objects().cloned().collect::<Vec<_>>();
            let after_members = doc.group(group).unwrap().members().collect::<Vec<_>>();
            for _ in 0..2 {
                registry.execute(&mut doc, "Undo").unwrap();
                assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
                assert_eq!(
                    doc.group(group).unwrap().members().collect::<Vec<_>>(),
                    before_members
                );
                registry.execute(&mut doc, "Redo").unwrap();
                assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
                assert_eq!(
                    doc.group(group).unwrap().members().collect::<Vec<_>>(),
                    after_members
                );
            }
        }
    }
}

#[test]
fn nonjoining_selections_preserve_redo_and_report_the_named_command_result() {
    for command in ["Join", "JoinCopy"] {
        for count in [1, 2] {
            for post in [false, true] {
                let mut doc = Document::default();
                let ids = (0..count)
                    .map(|i| {
                        doc.add_geometry(line(
                            [i as f64 * 10., 0., 0.],
                            [i as f64 * 10. + 1., 0., 0.],
                        ))
                        .unwrap()
                    })
                    .collect::<Vec<_>>();
                let registry = CommandRegistry::with_builtins();
                registry.execute(&mut doc, "Point 20,0,0").unwrap();
                registry.execute(&mut doc, "Undo").unwrap();
                doc.select_objects_direct(ids, SelectionMode::Replace)
                    .unwrap();
                let before = doc.objects().cloned().collect::<Vec<_>>();
                let result = run(&registry, &mut doc, command, post);
                if post || count == 1 {
                    assert!(matches!(result, Err(CommandError::NothingJoined)));
                } else {
                    result.unwrap();
                }
                assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
                assert_eq!(doc.redo_label(), Some("Point"));
                assert_eq!(doc.selected_object_count(), if post { 1 } else { count });
            }
        }
    }
}

#[test]
fn closed_only_selection_is_released_without_model_edits_or_losing_redo() {
    for command in ["Join", "JoinCopy"] {
        for post in [false, true] {
            let mut doc = Document::default();
            let registry = CommandRegistry::with_builtins();
            registry.execute(&mut doc, "Circle 0,0,0 2").unwrap();
            registry.execute(&mut doc, "Point 20,0,0").unwrap();
            registry.execute(&mut doc, "Undo").unwrap();
            let id = doc.objects().next().unwrap().id();
            doc.select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            assert!(matches!(
                run(&registry, &mut doc, command, post),
                Err(CommandError::NoOpenCurvesToJoin)
            ));
            assert_eq!(doc.selected_object_count(), 0);
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            assert_eq!(doc.redo_label(), Some("Point"));
        }
    }
}

#[test]
fn curve_preselection_batches_chains_while_postselection_extends_only_the_first() {
    for command in ["Join", "JoinCopy"] {
        for post in [false, true] {
            let mut doc = Document::default();
            let ids = [(0., 1.), (1., 2.), (10., 11.), (11., 12.)]
                .map(|(a, b)| doc.add_geometry(line([a, 0., 0.], [b, 0., 0.])).unwrap());
            for id in ids.into_iter().rev() {
                doc.select_objects_direct([id], SelectionMode::Add).unwrap();
            }
            run(&CommandRegistry::with_builtins(), &mut doc, command, post).unwrap();
            let joined = doc
                .objects()
                .filter(|o| !ids.contains(&o.id()))
                .collect::<Vec<_>>();
            assert_eq!(joined.len(), if post { 1 } else { 2 });
            let first = joined[0].geometry().curve_ref().unwrap();
            assert_eq!(first.domain(), if post { -1.0..=1.0 } else { 0.0..=2.0 });
            assert_eq!(
                first.start_point().unwrap().x(),
                if post { 10. } else { 0. }
            );
            for id in ids {
                assert_eq!(
                    doc.object(id).is_some(),
                    command == "JoinCopy" || (post && ids[..2].contains(&id))
                );
            }
        }
    }
}
