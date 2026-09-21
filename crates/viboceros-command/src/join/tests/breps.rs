use super::*;
use viboceros_geometry::Frame3;

fn box_brep(height: f64) -> Brep {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    Brep::try_box(
        frame,
        [[0., 2.], [0., 3.], [0., height]],
        Tolerance::DEFAULT,
    )
    .unwrap()
}

fn faces(indices: &[usize], height: f64) -> Geometry {
    Geometry::Brep(
        box_brep(height)
            .sub_brep(indices, Tolerance::DEFAULT)
            .unwrap(),
    )
}

fn execute(
    registry: &CommandRegistry,
    document: &mut Document,
    copy: bool,
    post: bool,
) -> Result<String, CommandError> {
    let command = if copy { "JoinCopy" } else { "Join" };
    if post {
        registry.execute_postselected(document, command, Default::default())
    } else {
        registry.execute(document, command)
    }
}

#[test]
fn every_disconnected_piece_inherits_seed_attributes_groups_and_replays_history() {
    for copy in [false, true] {
        for post in [false, true] {
            let mut document = Document::default();
            let layer = document.add_layer("Seed", ColorRgb::BLACK).unwrap();
            let attrs = ObjectAttributes::on_layer(layer)
                .with_name("seed")
                .with_object_color(ColorRgb::new(11, 22, 33));
            let first = document
                .add_geometry_with_attributes(faces(&[0, 1], 5.), attrs.clone())
                .unwrap();
            let second = document.add_geometry(faces(&[2], 2.)).unwrap();
            let peer = document
                .add_geometry(Geometry::Point(Point3::try_new(9., 0., 0.).unwrap()))
                .unwrap();
            let group = document
                .add_group(Some("Shared".into()), [first, second, peer])
                .unwrap();
            document
                .select_objects_direct([first, second], SelectionMode::Replace)
                .unwrap();
            let before = document.objects().cloned().collect::<Vec<_>>();
            let before_members = document.group(group).unwrap().members().collect::<Vec<_>>();
            let registry = CommandRegistry::with_builtins();
            execute(&registry, &mut document, copy, post).unwrap();
            let outputs = document
                .objects()
                .filter(|o| ![first, second, peer].contains(&o.id()))
                .collect::<Vec<_>>();
            assert_eq!(outputs.len(), 2);
            for (output, count) in outputs.iter().zip([2, 1]) {
                assert_eq!(output.attributes(), &attrs);
                assert_eq!(output.group_ids(), [group]);
                assert_eq!(document.is_selected(output.id()), !post);
                let Geometry::Brep(b) = output.geometry() else {
                    panic!("B-rep output")
                };
                assert_eq!(b.faces().len(), count);
                assert!(b.is_manifold());
            }
            assert!(!document.is_selected(peer));
            for id in [first, second] {
                assert_eq!(document.object(id).is_some(), copy);
                assert_eq!(document.is_selected(id), copy);
            }
            let after = document.objects().cloned().collect::<Vec<_>>();
            let after_members = document.group(group).unwrap().members().collect::<Vec<_>>();
            for _ in 0..2 {
                registry.execute(&mut document, "Undo").unwrap();
                assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
                assert_eq!(
                    document.group(group).unwrap().members().collect::<Vec<_>>(),
                    before_members
                );
                registry.execute(&mut document, "Redo").unwrap();
                assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
                assert_eq!(
                    document.group(group).unwrap().members().collect::<Vec<_>>(),
                    after_members
                );
            }
        }
    }
}

#[test]
fn command_first_grows_in_pick_order_without_retrying_skipped_faces() {
    for copy in [false, true] {
        for post in [false, true] {
            let mut document = Document::default();
            let ids = (0..6)
                .map(|i| document.add_geometry(faces(&[i], 5.)).unwrap())
                .collect::<Vec<_>>();
            document
                .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
            let registry = CommandRegistry::with_builtins();
            let prompt = registry.object_selection_prompt("Join").unwrap().unwrap();
            assert!(
                !registry
                    .object_selection_complete(&document, &prompt)
                    .unwrap()
            );
            execute(&registry, &mut document, copy, post).unwrap();
            let output = document.objects().find(|o| !ids.contains(&o.id())).unwrap();
            let Geometry::Brep(b) = output.geometry() else {
                panic!("B-rep output")
            };
            assert_eq!(b.faces().len(), if post { 5 } else { 6 });
            assert_eq!(b.is_solid(), !post);
            if !post {
                assert!((b.signed_volume(document.tolerance()).unwrap() - 30.).abs() < 1e-12);
            }
            assert_eq!(document.object(ids[1]).is_some(), copy || post);
            assert_eq!(document.is_selected(ids[1]), copy && !post);
            assert_eq!(document.is_selected(output.id()), !post);
        }
    }
}

#[test]
fn disconnected_preselection_makes_fresh_copies_but_postselection_preserves_failed_seed_and_redo() {
    for copy in [false, true] {
        for post in [false, true] {
            let mut document = Document::default();
            let ids = [
                document.add_geometry(faces(&[0], 5.)).unwrap(),
                document.add_geometry(faces(&[1], 5.)).unwrap(),
            ];
            let registry = CommandRegistry::with_builtins();
            registry.execute(&mut document, "Point 20,0,0").unwrap();
            registry.execute(&mut document, "Undo").unwrap();
            let redo = document.redo_label().map(str::to_owned);
            let before = document.objects().cloned().collect::<Vec<_>>();
            document
                .select_objects_direct(ids, SelectionMode::Replace)
                .unwrap();
            let result = execute(&registry, &mut document, copy, post);
            if post {
                assert!(matches!(result, Err(CommandError::NothingJoined)));
                assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
                assert_eq!(document.redo_label(), redo.as_deref());
                assert!(document.is_selected(ids[0]));
                assert!(!document.is_selected(ids[1]));
            } else {
                result.unwrap();
                assert_eq!(document.objects().len(), if copy { 4 } else { 2 });
                assert_eq!(document.selected_object_count(), document.objects().len());
                assert_eq!(
                    document
                        .objects()
                        .filter(|o| !ids.contains(&o.id()))
                        .count(),
                    2
                );
            }
        }
    }
}

#[test]
fn raw_surfaces_mix_with_breps_and_use_the_correct_attribute_seed() {
    for post in [false, true] {
        let mut document = Document::default();
        let sheet = box_brep(5.).sub_brep(&[0], Tolerance::DEFAULT).unwrap();
        let first = document
            .add_geometry(Geometry::NurbsSurface(sheet.faces()[0].surface().clone()))
            .unwrap();
        let second = document.add_geometry(faces(&[2], 5.)).unwrap();
        document
            .set_object_names([(second, Some("picked first".into()))])
            .unwrap();
        let attrs = document
            .object(if post { second } else { first })
            .unwrap()
            .attributes()
            .clone();
        for id in [second, first] {
            document
                .select_objects_direct([id], SelectionMode::Add)
                .unwrap();
        }
        execute(
            &CommandRegistry::with_builtins(),
            &mut document,
            false,
            post,
        )
        .unwrap();
        let result = document.objects().next().unwrap();
        assert_eq!(result.attributes(), &attrs);
        assert!(matches!(result.geometry(), Geometry::Brep(b) if b.faces().len() == 2));
    }
}

#[test]
fn closed_inputs_are_released_and_mixed_families_fail_atomically() {
    for post in [false, true] {
        let mut document = Document::default();
        let solid = document.add_geometry(Geometry::Brep(box_brep(5.))).unwrap();
        let sheet = document.add_geometry(faces(&[0], 5.)).unwrap();
        let mesh = document.add_geometry(quad(0.)).unwrap();
        let registry = CommandRegistry::with_builtins();
        for (ids, expected_selected) in [(vec![solid], vec![]), (vec![solid, sheet], vec![sheet])] {
            document
                .select_objects_direct(ids, SelectionMode::Replace)
                .unwrap();
            assert!(execute(&registry, &mut document, false, post).is_err());
            assert_eq!(
                document
                    .selected_objects()
                    .map(|o| o.id())
                    .collect::<Vec<_>>(),
                expected_selected
            );
        }
        document
            .select_objects_direct([sheet, mesh], SelectionMode::Replace)
            .unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        assert!(matches!(
            execute(&registry, &mut document, false, post),
            Err(CommandError::UnsupportedJoinGeometry)
        ));
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(document.selected_object_count(), 2);
    }
}

#[test]
fn later_picks_reconsider_accepted_original_boundaries_without_losing_output_pieces() {
    for copy in [false, true] {
        let mut document = Document::default();
        let ids = [0, 2, 2].map(|i| document.add_geometry(faces(&[i], 5.)).unwrap());
        let peer = document.add_geometry(faces(&[1], 5.)).unwrap();
        let group = document.add_group(None, [ids[0], peer]).unwrap();
        for id in ids {
            document
                .select_objects_direct([id], SelectionMode::Add)
                .unwrap();
        }
        let before = document.objects().cloned().collect::<Vec<_>>();
        let registry = CommandRegistry::with_builtins();
        execute(&registry, &mut document, copy, true).unwrap();
        let outputs = document
            .objects()
            .filter(|o| !ids.contains(&o.id()) && o.id() != peer)
            .collect::<Vec<_>>();
        assert_eq!(outputs.len(), 2);
        for (output, faces) in outputs.iter().zip([1, 2]) {
            assert!(matches!(output.geometry(), Geometry::Brep(b) if b.faces().len() == faces));
            assert_eq!(output.group_ids(), [group]);
            assert!(!document.is_selected(output.id()));
        }
        assert_eq!(document.selected_object_count(), if copy { 3 } else { 0 });
        assert!(!document.is_selected(peer));
        let after = document.objects().cloned().collect::<Vec<_>>();
        for _ in 0..2 {
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
            registry.execute(&mut document, "Redo").unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
        }
    }
}
