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
fn unsupported_inputs_and_colors_preserve_geometry_selection_and_redo() {
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
        "PointCloud UsePointColors=Yes",
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
