#![cfg(feature = "native-smlib")]
use viboceros_command::CommandRegistry;
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, SelectionMode};
use viboceros_geometry::{Brep, Frame3, Point3, Tolerance, Vector3};

fn box_sheets(remote: bool, swapped: bool, mask: u8) -> (Brep, Vec<Brep>) {
    let t = Tolerance::DEFAULT;
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        t,
    )
    .unwrap();
    let cube = Brep::try_box(frame, [[0., 2.], [0., 3.], [0., 4.]], t).unwrap();
    // Every command-first pick must touch an accepted sheet. Insert a side
    // between the two opposite caps so all prefixes are connected.
    let pieces = [0, 2, 1, 3, 4, 5]
        .into_iter()
        .map(|i| {
            let face = &cube.faces()[i];
            let (u, v) = if remote {
                (1e12..=1e12 + 1., -2e12..=-2e12 + 2.)
            } else {
                (0. ..=1., -2. ..=3.)
            };
            let surface = face.surface().try_reparameterized(u, v).unwrap();
            let surface = if swapped {
                surface.try_swapped_uv().unwrap()
            } else {
                surface
            };
            let sheet = Brep::try_surface_face(surface, t).unwrap();
            if face.is_reversed() ^ swapped ^ (mask & (1 << i) != 0) {
                sheet.reversed()
            } else {
                sheet
            }
        })
        .collect();
    (cube, pieces)
}

fn check_geometry(output: &Brep, cube: &Brep, sources: &[Brep]) {
    assert!(output.is_solid());
    assert!(output.is_manifold());
    assert_eq!(output.faces().len(), 6);
    assert_eq!(output.edges().len(), 12);
    assert_eq!(output.vertices().len(), 8);
    assert_eq!(output.edge_use_counts(), vec![2; 12]);
    assert!((output.signed_volume(Tolerance::DEFAULT).unwrap() - 24.).abs() < 1e-9);
    let mut matched = [false; 6];
    for face in output.faces() {
        let i = sources
            .iter()
            .position(|source| face.surface() == source.faces()[0].surface())
            .expect("complete source surface coefficients and original chart");
        assert!(!matched[i]);
        matched[i] = true;
    }
    assert_eq!(matched, [true; 6]);
    for vertex in output.vertices() {
        assert!(cube.vertices().iter().any(|v| v.point() == vertex.point()));
    }
    // A degree-one positive-weight two-control curve has exactly its endpoint
    // segment as its image. Match every complete segment against the original
    // box, independently of native curve parameters, tolerances and ordering.
    let mut matched_edges = [false; 12];
    for edge in output.edges() {
        assert_eq!(edge.curve().degree(), 1);
        let controls = edge.curve().control_points();
        assert_eq!(controls.len(), 2);
        let points = [controls[0].point(), controls[1].point()];
        let i = cube
            .edges()
            .iter()
            .position(|expected| {
                let endpoints = expected.vertices().map(|i| cube.vertices()[i].point());
                points == endpoints || points == [endpoints[1], endpoints[0]]
            })
            .expect("exact original spatial segment");
        assert!(!matched_edges[i]);
        matched_edges[i] = true;
    }
    assert_eq!(matched_edges, [true; 12]);
}

#[test]
fn closed_join_orients_local_and_translated_components_and_preserves_history() {
    let registry = CommandRegistry::with_builtins();
    let mut checked = 0;
    for remote in [false, true] {
        for swapped in [false, true] {
            for mask in [0_u8, 3, 21, 63] {
                for copy in [false, true] {
                    for postselected in [false, true] {
                        eprintln!(
                            "CLOSED_JOIN remote={remote} swapped={swapped} mask={mask} copy={copy} post={postselected}"
                        );
                        let (cube, sources) = box_sheets(remote, swapped, mask);
                        let mut document =
                            Document::new(Tolerance::try_new(0.001, 1e-12, 1e-10).unwrap());
                        let ids = sources
                            .iter()
                            .enumerate()
                            .map(|(i, source)| {
                                document
                                    .add_geometry_with_attributes(
                                        Geometry::Brep(source.clone()),
                                        ObjectAttributes::on_layer(document.current_layer_id())
                                            .with_name(format!("face-{i}"))
                                            .with_object_color(ColorRgb::new(10 + i as u8, 30, 50)),
                                    )
                                    .unwrap()
                            })
                            .collect::<Vec<_>>();
                        let seed = document.object(ids[0]).unwrap().attributes().clone();
                        let peer = document
                            .add_geometry(Geometry::Point(
                                Point3::try_new(100., 100., 100.).unwrap(),
                            ))
                            .unwrap();
                        let group = document
                            .add_group(
                                Some("Box sources".into()),
                                ids.iter().copied().chain([peer]),
                            )
                            .unwrap();
                        document
                            .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                            .unwrap();
                        document.clear_history().unwrap();
                        let before = document.objects().cloned().collect::<Vec<_>>();
                        let memberships =
                            document.group(group).unwrap().members().collect::<Vec<_>>();
                        let command = if copy { "JoinCopy" } else { "Join" };
                        if postselected {
                            registry.execute_postselected(
                                &mut document,
                                command,
                                Default::default(),
                            )
                        } else {
                            registry.execute(&mut document, command)
                        }
                        .unwrap();
                        let outputs = document
                            .objects()
                            .filter(|o| !ids.contains(&o.id()) && o.id() != peer)
                            .collect::<Vec<_>>();
                        assert_eq!(outputs.len(), 1);
                        let output = outputs[0];
                        assert_eq!(output.attributes(), &seed);
                        assert_eq!(output.group_ids(), [group]);
                        assert_eq!(document.is_selected(output.id()), !postselected);
                        assert!(!document.is_selected(peer));
                        let Geometry::Brep(brep) = output.geometry() else {
                            panic!("closed B-rep output");
                        };
                        check_geometry(brep, &cube, &sources);
                        for (i, &id) in ids.iter().enumerate() {
                            assert_eq!(document.object(id).is_some(), copy);
                            assert_eq!(document.is_selected(id), copy);
                            if copy {
                                assert_eq!(
                                    document.object(id).unwrap().geometry(),
                                    &Geometry::Brep(sources[i].clone())
                                );
                            }
                        }
                        assert_eq!(document.undo_label(), Some(command));
                        let after = document.objects().cloned().collect::<Vec<_>>();
                        let after_memberships =
                            document.group(group).unwrap().members().collect::<Vec<_>>();
                        for _ in 0..2 {
                            registry.execute(&mut document, "Undo").unwrap();
                            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
                            assert_eq!(
                                document.group(group).unwrap().members().collect::<Vec<_>>(),
                                memberships
                            );
                            registry.execute(&mut document, "Redo").unwrap();
                            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
                            assert_eq!(
                                document.group(group).unwrap().members().collect::<Vec<_>>(),
                                after_memberships
                            );
                        }
                        checked += 1;
                    }
                }
            }
        }
    }
    assert_eq!(checked, 64);
    eprintln!("CLOSED_JOIN cases={checked}");
}
