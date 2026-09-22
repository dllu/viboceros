use super::*;
use viboceros_geometry::BrepSolidOrientation;

#[test]
fn extreme_scale_join_and_copy_normalize_without_mass_and_keep_atomic_history() {
    for exponent in [0, -360, 360] {
        let scale = 2_f64.powi(exponent);
        let tolerance = Tolerance::try_new(scale * 1e-9, 1e-12, 1e-10).unwrap();
        let source = Brep::try_box(
            CommandContext::default().construction_plane,
            [[0., scale], [0., 2. * scale], [0., 4. * scale]],
            tolerance,
        )
        .unwrap();
        for inward in [false, true] {
            for reverse_order in [false, true] {
                for copy in [false, true] {
                    for post in [false, true] {
                        let mut document = Document::new(tolerance);
                        // Every consecutive face contacts the preceding face,
                        // even in reverse order; postselection skips no source.
                        let mut order = [0, 2, 4, 1, 3, 5];
                        if reverse_order {
                            order.reverse();
                        }
                        let ids = order
                            .into_iter()
                            .map(|i| {
                                let part = source.sub_brep(&[i], tolerance).unwrap();
                                document
                                    .add_geometry_with_attributes(
                                        Geometry::Brep(if inward { part.reversed() } else { part }),
                                        ObjectAttributes::on_layer(document.current_layer_id())
                                            .with_name(format!("face-{i}")),
                                    )
                                    .unwrap()
                            })
                            .collect::<Vec<_>>();
                        let group = document
                            .add_group(Some("shell".into()), ids.iter().copied())
                            .unwrap();
                        document
                            .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                            .unwrap();
                        let before = document.objects().cloned().collect::<Vec<_>>();
                        let attributes = document.object(ids[0]).unwrap().attributes().clone();
                        let registry = CommandRegistry::with_builtins();
                        execute(&registry, &mut document, copy, post).unwrap();
                        let outputs = document
                            .objects()
                            .filter(|o| !ids.contains(&o.id()))
                            .collect::<Vec<_>>();
                        assert_eq!(outputs.len(), 1);
                        let output = outputs[0];
                        assert_eq!(output.attributes(), &attributes);
                        assert_eq!(output.group_ids(), &[group]);
                        assert_eq!(document.is_selected(output.id()), !post);
                        let Geometry::Brep(joined) = output.geometry() else {
                            panic!("B-rep");
                        };
                        assert_eq!(joined.faces().len(), 6);
                        assert_eq!(
                            joined.solid_orientation().unwrap(),
                            BrepSolidOrientation::Outward
                        );
                        assert!(joined.faces().iter().all(|f| !f.is_reversed()));
                        for id in &ids {
                            assert_eq!(document.object(*id).is_some(), copy);
                            assert_eq!(document.is_selected(*id), copy);
                            if copy {
                                assert_eq!(
                                    document.object(*id),
                                    before.iter().find(|o| o.id() == *id)
                                );
                            }
                        }
                        let after = document.objects().cloned().collect::<Vec<_>>();
                        assert_eq!(
                            document.undo_label(),
                            Some(if copy { "JoinCopy" } else { "Join" })
                        );
                        registry.execute(&mut document, "Undo").unwrap();
                        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
                        registry.execute(&mut document, "Redo").unwrap();
                        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
                    }
                }
            }
        }
    }
}
