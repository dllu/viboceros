#![cfg(feature = "native-smlib")]
use viboceros_command::CommandRegistry;
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, SelectionMode};
use viboceros_geometry::{Brep, NurbsSurface, Point3, Tolerance};

fn surface(unclamped: bool, remote: bool) -> NurbsSurface {
    let profile = if remote && unclamped {
        [[-4., -2.], [4., 2.], [12., -2.]]
    } else if remote {
        [[0., 0.], [4., 2.], [8., 0.]]
    } else if unclamped {
        [[-15., -15.], [15., 15.], [45., -15.]]
    } else {
        [[0., 0.], [15., 15.], [30., 0.]]
    };
    let height = if unclamped { -3. } else { 3. };
    NurbsSurface::try_new(
        2,
        1,
        3,
        2,
        [0., height]
            .into_iter()
            .flat_map(|z| {
                profile
                    .into_iter()
                    .map(move |p| Point3::try_new(p[0], p[1], z).unwrap())
            })
            .collect(),
        if unclamped {
            vec![-2., -1., 0., 1., 2., 3.]
        } else {
            vec![0., 0., 0., 1., 1., 1.]
        },
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

fn check_workflows(remote: bool) {
    let mut checked = 0;
    for swapped in [false, true] {
        for reversed in [false, true] {
            for copy in [false, true] {
                for postselected in [false, true] {
                    eprintln!(
                        "JOIN_CHART remote={remote} swapped={swapped} reversed={reversed} copy={copy} post={postselected}"
                    );
                    let mut document = if remote {
                        Document::new(Tolerance::try_new(0.001, 1e-12, 1e-10).unwrap())
                    } else {
                        Document::default()
                    };
                    let t = Tolerance::DEFAULT;
                    let sources = [surface(false, remote), surface(true, remote)].map(|surface| {
                        let surface = if remote {
                            surface
                                .try_reparameterized(1e12..=1e12 + 1., -2e12..=-2e12 + 2.)
                                .unwrap()
                        } else {
                            surface
                        };
                        let surface = if swapped {
                            surface.try_swapped_uv().unwrap()
                        } else {
                            surface
                        };
                        let brep = Brep::try_surface_face(surface, t).unwrap();
                        if reversed { brep.reversed() } else { brep }
                    });
                    let mut ids = Vec::new();
                    for (i, source) in sources.iter().enumerate() {
                        ids.push(
                            document
                                .add_geometry_with_attributes(
                                    Geometry::Brep(source.clone()),
                                    ObjectAttributes::on_layer(document.current_layer_id())
                                        .with_name(format!("source-{i}"))
                                        .with_object_color(ColorRgb::new(10 + i as u8, 30, 50)),
                                )
                                .unwrap(),
                        );
                    }
                    let seed_attributes = document.object(ids[0]).unwrap().attributes().clone();
                    let peer = document
                        .add_geometry(Geometry::Point(Point3::try_new(100., 100., 100.).unwrap()))
                        .unwrap();
                    let group = document
                        .add_group(Some("Unclamped sources".into()), [ids[0], ids[1], peer])
                        .unwrap();
                    document
                        .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                        .unwrap();
                    document.clear_history().unwrap();
                    let before = document.objects().cloned().collect::<Vec<_>>();
                    let memberships = document.group(group).unwrap().members().collect::<Vec<_>>();
                    let registry = CommandRegistry::with_builtins();
                    let command = if copy { "JoinCopy" } else { "Join" };
                    let result = if postselected {
                        registry.execute_postselected(&mut document, command, Default::default())
                    } else {
                        registry.execute(&mut document, command)
                    };
                    result.unwrap_or_else(|e| panic!("remote={remote} swapped={swapped} reversed={reversed} copy={copy} post={postselected}: {e}"));
                    let outputs = document
                        .objects()
                        .filter(|o| !ids.contains(&o.id()) && o.id() != peer)
                        .collect::<Vec<_>>();
                    assert_eq!(outputs.len(), 1);
                    let output = outputs[0];
                    assert_eq!(output.attributes(), &seed_attributes);
                    assert_eq!(output.group_ids(), [group]);
                    assert_eq!(document.is_selected(output.id()), !postselected);
                    assert!(!document.is_selected(peer));
                    let Geometry::Brep(brep) = output.geometry() else {
                        panic!("B-rep output");
                    };
                    assert!(brep.is_manifold());
                    assert!(!brep.is_closed());
                    assert_eq!(brep.faces().len(), 2);
                    assert_eq!(brep.edges().len(), 7);
                    assert_eq!(
                        brep.edge_use_counts()
                            .into_iter()
                            .filter(|&n| n == 2)
                            .count(),
                        1
                    );
                    // This is a complete coefficient comparison against two
                    // independently authored sheets, allowing the native
                    // orientation operation's documented chart transpose.
                    let expected = sources
                        .iter()
                        .map(|b| {
                            b.faces()[0]
                                .surface()
                                .try_clamped_to_active_domain()
                                .unwrap()
                        })
                        .collect::<Vec<_>>();
                    let mut matched = [false; 2];
                    for face in brep.faces() {
                        let index = expected
                            .iter()
                            .position(|s| {
                                face.surface() == s
                                    || face.surface().try_swapped_uv().unwrap() == *s
                            })
                            .unwrap();
                        assert!(!matched[index]);
                        matched[index] = true;
                    }
                    assert_eq!(matched, [true; 2]);
                    for (i, id) in ids.iter().enumerate() {
                        assert_eq!(document.object(*id).is_some(), copy);
                        assert_eq!(document.is_selected(*id), copy);
                        if copy {
                            assert_eq!(
                                document.object(*id).unwrap().geometry(),
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
    assert_eq!(checked, 16);
    eprintln!("UNCLAMPED_JOIN remote={remote} cases={checked}");
}

#[test]
fn unclamped_join_and_copy_preserve_the_active_geometry_metadata_and_history() {
    check_workflows(false);
}

#[test]
fn translated_charts_preserve_join_copy_metadata_and_history() {
    check_workflows(true);
}
