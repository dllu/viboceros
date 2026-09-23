use super::*;

#[test]
#[ignore = "manual large point-object conversion timing"]
fn benchmark_large_point_conversion() {
    let mut doc = Document::default();
    doc.begin_transaction("fixture").unwrap();
    for i in 0..10_000 {
        doc.add_geometry(Geometry::Point(p(i as Real))).unwrap();
    }
    doc.commit_transaction().unwrap();
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    doc.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    let registry = CommandRegistry::with_builtins();
    let start = std::time::Instant::now();
    registry.execute(&mut doc, "PointCloud").unwrap();
    let convert = start.elapsed();
    let start = std::time::Instant::now();
    registry.execute(&mut doc, "Undo").unwrap();
    let undo = start.elapsed();
    let start = std::time::Instant::now();
    registry.execute(&mut doc, "Redo").unwrap();
    eprintln!(
        "10k points: conversion={convert:?}, undo={undo:?}, redo={:?}",
        start.elapsed()
    );
    assert_eq!(doc.objects().len(), 1);
}

fn p(x: Real) -> Point3 {
    Point3::try_new(x, 0.0, 0.0).unwrap()
}

#[test]
fn point_conversion_respects_pre_and_postselection_order_and_undo() {
    for postselected in [false, true] {
        let mut doc = Document::default();
        let ids = [1.0, 2.0, 1.0].map(|x| doc.add_geometry(Geometry::Point(p(x))).unwrap());
        let group = doc.add_group(Some("sources".into()), ids).unwrap();
        for id in [ids[2], ids[0], ids[1]] {
            doc.select_objects_direct([id], SelectionMode::Add).unwrap();
        }
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let registry = CommandRegistry::with_builtins();
        if postselected {
            registry
                .execute_postselected(&mut doc, "PointCloud", CommandContext::default())
                .unwrap();
        } else {
            registry.execute(&mut doc, "PointCloud").unwrap();
        }
        assert_eq!(doc.objects().len(), 1);
        let object = doc.objects().next().unwrap();
        let Geometry::PointCloud(cloud) = object.geometry() else {
            panic!()
        };
        assert_eq!(
            cloud.points(),
            if postselected {
                [p(1.0), p(1.0), p(2.0)]
            } else {
                [p(1.0), p(2.0), p(1.0)]
            }
        );
        assert!(object.group_ids().is_empty());
        assert_eq!(object.attributes().name(), None);
        assert_eq!(doc.selected_object_count(), 0);
        let after = doc.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(doc.group(group).is_some());
        // Batch deletion restores the removed sources' relative pick order.
        // This native invariant does not claim Rhino Undo pick-order parity.
        assert_eq!(
            doc.selected_object_ids().collect::<Vec<_>>(),
            [ids[2], ids[0], ids[1]]
        );
        assert_eq!(
            doc.selected_object_ids().collect::<BTreeSet<_>>(),
            ids.into_iter().collect()
        );
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
    }
}

#[test]
fn mesh_keeps_unused_duplicate_vertices_and_source_object() {
    let mut doc = Document::default();
    let vertices = vec![
        p(0.0),
        p(3.0),
        Point3::try_new(0.0, 4.0, 0.0).unwrap(),
        p(0.0),
    ];
    let mesh = TriangleMesh::try_new(vertices.clone(), vec![[0, 1, 2]], doc.tolerance()).unwrap();
    let id = doc.add_geometry(Geometry::Mesh(mesh)).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut doc, "PointCloud UsePointColors=No")
        .unwrap();
    assert!(doc.is_selected(id));
    assert_eq!(doc.objects().len(), 2);
    let cloud = doc
        .objects()
        .find_map(|o| {
            if let Geometry::PointCloud(c) = o.geometry() {
                Some(c)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(cloud.points(), vertices);
}

#[test]
fn unsupported_inputs_preserve_geometry_selection_and_redo() {
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::PointCloud(
            PointCloud3::try_new(vec![p(1.0)]).unwrap(),
        ))
        .unwrap();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 2,0,0").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for input in [
        "PointCloud",
        "PointCloud Add",
        "PointCloud Remove",
        "PointCloud UsePointColors=No UsePointColors=No",
        "PointCloud 0,0,0",
    ] {
        assert!(registry.execute(&mut doc, input).is_err(), "{input}");
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(doc.is_selected(id));
        assert_eq!(doc.redo_label(), Some("Point"));
    }
}

#[test]
fn single_point_is_a_noop_but_postselection_is_cleared() {
    for postselected in [false, true] {
        let mut doc = Document::default();
        let id = doc.add_geometry(Geometry::Point(p(1.0))).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        if postselected {
            registry
                .execute_postselected(&mut doc, "PointCloud", CommandContext::default())
                .unwrap();
        } else {
            registry.execute(&mut doc, "PointCloud").unwrap();
        }
        assert_eq!(doc.objects().len(), 1);
        assert!(matches!(
            doc.object(id).unwrap().geometry(),
            Geometry::Point(_)
        ));
        assert_eq!(doc.is_selected(id), !postselected);
        assert_eq!(doc.undo_label(), Some("Add object"));
    }
}

#[test]
fn add_points_and_another_cloud_preserves_target_and_undo() {
    let mut doc = Document::default();
    let target = doc
        .add_geometry(Geometry::PointCloud(
            PointCloud3::try_new(vec![p(1.0)]).unwrap(),
        ))
        .unwrap();
    let point = doc.add_geometry(Geometry::Point(p(2.0))).unwrap();
    let normal = viboceros_geometry::Vector3::try_new(0.0, 0.0, 1.0).unwrap();
    let zero = viboceros_geometry::Vector3::try_new(0.0, 0.0, 0.0).unwrap();
    let source = doc
        .add_geometry(Geometry::PointCloud(
            PointCloud3::try_with_channels(
                vec![p(3.0), p(3.0)],
                viboceros_geometry::PointCloudChannels {
                    normals: Some(vec![normal, normal]),
                    values: Some(vec![7.0, 9.0]),
                    ordered: true,
                    ..viboceros_geometry::PointCloudChannels::default()
                },
            )
            .unwrap(),
        ))
        .unwrap();
    let group = doc.add_group(Some("cloud".into()), [target]).unwrap();
    doc.select_objects_direct([target, point, source], SelectionMode::Replace)
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let registry = CommandRegistry::with_builtins();
    assert!(matches!(
        registry.execute(&mut doc, "PointCloud Add"),
        Err(CommandError::PointCloudTargetAmbiguous)
    ));
    registry
        .execute(&mut doc, &format!("PointCloud Add Target={target}"))
        .unwrap();
    assert_eq!(doc.objects().len(), 1);
    let object = doc.object(target).unwrap();
    assert_eq!(object.group_ids(), [group]);
    let Geometry::PointCloud(cloud) = object.geometry() else {
        panic!()
    };
    assert_eq!(cloud.points(), [p(1.0), p(2.0), p(3.0), p(3.0)]);
    assert_eq!(cloud.normals().unwrap(), [zero, zero, normal, normal]);
    assert_eq!(cloud.values().unwrap(), [0.0, 0.0, 7.0, 9.0]);
    assert!(!cloud.is_ordered());
    assert!(doc.is_selected(target));
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().len(), 1);
}

#[test]
fn remove_indices_preserves_order_and_is_atomic_on_invalid_index() {
    for output in ["Points", "PointCloud"] {
        let mut doc = Document::default();
        let target = doc
            .add_geometry(Geometry::PointCloud(
                PointCloud3::try_new(vec![p(1.0), p(2.0), p(2.0), p(4.0)]).unwrap(),
            ))
            .unwrap();
        doc.select_objects_direct([target], SelectionMode::Replace)
            .unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let registry = CommandRegistry::with_builtins();
        assert!(matches!(
            registry.execute(&mut doc, "PointCloud Remove Indices=1,9"),
            Err(CommandError::PointCloudIndexOutOfRange)
        ));
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        registry
            .execute(
                &mut doc,
                &format!("PointCloud Remove Indices=2,0,2 Output={output}"),
            )
            .unwrap();
        let Geometry::PointCloud(cloud) = doc.object(target).unwrap().geometry() else {
            panic!()
        };
        assert_eq!(cloud.points(), [p(2.0), p(4.0)]);
        let outputs = doc
            .objects()
            .filter(|o| o.id() != target)
            .collect::<Vec<_>>();
        if output == "Points" {
            assert_eq!(outputs.len(), 2);
            assert!(matches!(outputs[0].geometry(), Geometry::Point(point) if *point == p(1.0)));
            assert!(matches!(outputs[1].geometry(), Geometry::Point(point) if *point == p(2.0)));
        } else {
            assert_eq!(outputs.len(), 1);
            let Geometry::PointCloud(cloud) = outputs[0].geometry() else {
                panic!()
            };
            assert_eq!(cloud.points(), [p(1.0), p(2.0)]);
        }
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn removing_every_point_deletes_the_empty_source_cloud() {
    let mut doc = Document::default();
    let target = doc
        .add_geometry(Geometry::PointCloud(
            PointCloud3::try_new(vec![p(5.0)]).unwrap(),
        ))
        .unwrap();
    doc.select_objects_direct([target], SelectionMode::Replace)
        .unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut doc, "PointCloud Remove Indices=0")
        .unwrap();
    assert!(doc.object(target).is_none());
    assert!(
        matches!(doc.objects().next().unwrap().geometry(), Geometry::Point(point) if *point == p(5.0))
    );
}

#[test]
fn creation_inherits_source_display_colors_and_undoes() {
    let mut doc = Document::default();
    let red = ColorRgb::new(220, 20, 30);
    let blue = ColorRgb::new(15, 40, 230);
    let layer = doc.add_layer("Source", red).unwrap();
    let first = doc
        .add_geometry_with_attributes(Geometry::Point(p(1.0)), ObjectAttributes::on_layer(layer))
        .unwrap();
    let second = doc
        .add_geometry_with_attributes(
            Geometry::Point(p(2.0)),
            ObjectAttributes::on_layer(layer).with_object_color(blue),
        )
        .unwrap();
    let mesh = TriangleMesh::try_new(
        vec![p(3.0), p(4.0), Point3::try_new(3.0, 1.0, 0.0).unwrap()],
        vec![[0, 1, 2]],
        doc.tolerance(),
    )
    .unwrap();
    let mesh = mesh
        .try_with_vertex_colors(Some(vec![[5, 6, 7, 0], [8, 9, 10, 128], [11, 12, 13, 255]]))
        .unwrap();
    let mesh_id = doc
        .add_geometry_with_attributes(Geometry::Mesh(mesh), ObjectAttributes::on_layer(layer))
        .unwrap();
    doc.select_objects_direct([first, second, mesh_id], SelectionMode::Replace)
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut doc, "PointCloud UsePointColors=Yes")
        .unwrap();
    let cloud = doc
        .objects()
        .find_map(|object| match object.geometry() {
            Geometry::PointCloud(cloud) => Some(cloud),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        cloud.colors(),
        Some(
            &[
                [220, 20, 30, 0],
                [15, 40, 230, 0],
                [5, 6, 7, 0],
                [8, 9, 10, 128],
                [11, 12, 13, 255],
            ][..]
        )
    );
    assert!(doc.object(mesh_id).is_some());
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn cloud_edits_keep_member_colors_aligned_with_stored_points() {
    let mut doc = Document::default();
    let up = viboceros_geometry::Vector3::try_new(0.0, 0.0, 1.0).unwrap();
    let side = viboceros_geometry::Vector3::try_new(1.0, 0.0, 0.0).unwrap();
    let zero = viboceros_geometry::Vector3::try_new(0.0, 0.0, 0.0).unwrap();
    let target = doc
        .add_geometry(Geometry::PointCloud(
            PointCloud3::try_with_channels(
                vec![p(1.0), p(2.0)],
                viboceros_geometry::PointCloudChannels {
                    colors: Some(vec![[10, 20, 30, 0], [40, 50, 60, 128]]),
                    normals: Some(vec![up, side]),
                    values: Some(vec![3.5, 7.25]),
                    ordered: true,
                    plane: Some(
                        viboceros_geometry::PointCloudPlane::try_new(
                            Point3::try_new(0.0, 0.0, 5.0).unwrap(),
                            [
                                viboceros_geometry::Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
                                viboceros_geometry::Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
                                viboceros_geometry::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
                            ],
                        )
                        .unwrap(),
                    ),
                },
            )
            .unwrap(),
        ))
        .unwrap();
    let added = doc.add_geometry(Geometry::Point(p(3.0))).unwrap();
    doc.select_objects_direct([target, added], SelectionMode::Replace)
        .unwrap();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "PointCloud Add").unwrap();
    let Geometry::PointCloud(cloud) = doc.object(target).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(
        cloud.colors().unwrap()[..2],
        [[10, 20, 30, 0], [40, 50, 60, 128]]
    );
    assert_eq!(cloud.normals().unwrap(), [up, side, zero]);
    assert_eq!(cloud.values().unwrap(), [3.5, 7.25, 0.0]);
    assert!(cloud.is_ordered());
    let plane = cloud.plane();
    assert!(plane.is_some());
    let added_color = cloud.colors().unwrap()[2];
    registry
        .execute(&mut doc, "PointCloud Remove Indices=1 Output=PointCloud")
        .unwrap();
    let Geometry::PointCloud(retained) = doc.object(target).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(retained.points(), [p(1.0), p(3.0)]);
    assert_eq!(retained.colors().unwrap(), [[10, 20, 30, 0], added_color]);
    assert_eq!(retained.normals().unwrap(), [up, zero]);
    assert_eq!(retained.values().unwrap(), [3.5, 0.0]);
    assert!(retained.is_ordered());
    assert_eq!(retained.plane(), plane);
    let removed = doc
        .objects()
        .find_map(|object| {
            if object.id() != target
                && let Geometry::PointCloud(cloud) = object.geometry()
            {
                return Some(cloud);
            }
            None
        })
        .unwrap();
    assert_eq!(removed.points(), [p(2.0)]);
    assert_eq!(removed.colors().unwrap(), [[40, 50, 60, 128]]);
    assert_eq!(removed.normals().unwrap(), [side]);
    assert_eq!(removed.values().unwrap(), [7.25]);
    assert!(removed.is_ordered());
    assert_eq!(removed.plane(), plane);
    registry
        .execute(&mut doc, "PointCloud Remove Indices=0 Output=Points")
        .unwrap();
    let output_point = doc
        .objects()
        .find(|object| matches!(object.geometry(), Geometry::Point(_)))
        .unwrap();
    assert_eq!(
        output_point.attributes().object_color(),
        ColorRgb::new(10, 20, 30)
    );
    assert_eq!(
        output_point.attributes().color_source(),
        viboceros_document::ObjectColorSource::Object
    );
}
