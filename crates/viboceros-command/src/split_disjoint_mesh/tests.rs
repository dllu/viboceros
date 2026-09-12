use super::*;

#[test]
fn staging_enforces_a_cumulative_piece_budget_without_counting_connected_meshes() {
    let mut document = Document::default();
    let ids = [2, 1, 3].map(|count| {
        let vertices = (0..count)
            .flat_map(|index| {
                let x = f64::from(index) * 3.;
                [[x, 0., 0.], [x + 1., 0., 0.], [x, 1., 0.]]
            })
            .map(|p| Point3::try_from(p).unwrap())
            .collect();
        let faces = (0..count as u32)
            .map(|index| [index * 3, index * 3 + 1, index * 3 + 2])
            .collect();
        document
            .add_geometry(Geometry::Mesh(
                TriangleMesh::try_new(vertices, faces, document.tolerance()).unwrap(),
            ))
            .unwrap()
    });
    document
        .add_geometry(Geometry::Point(Point3::try_new(99., 0., 0.).unwrap()))
        .unwrap();
    document.undo().unwrap();
    for id in ids {
        document.select_object(id, SelectionMode::Add).unwrap();
    }
    let before = format!("{document:?}");
    assert!(matches!(
        stage_disjoint_meshes(&document, 4),
        Err(CommandError::TooManySpanOutputObjects {
            command: "SplitDisjointMesh",
            ..
        })
    ));
    assert_eq!(format!("{document:?}"), before);
    let (staged, count) = stage_disjoint_meshes(&document, 5).unwrap();
    assert_eq!(count, 5);
    assert_eq!(
        staged
            .iter()
            .map(|input| input.pieces.len())
            .collect::<Vec<_>>(),
        [2, 1, 3]
    );
    assert_eq!(format!("{document:?}"), before);
    document
        .select_object(ids[1], SelectionMode::Replace)
        .unwrap();
    let (staged, count) = stage_disjoint_meshes(&document, 0).unwrap();
    assert_eq!(count, 0);
    assert_eq!(staged.len(), 1);
    assert_eq!(staged[0].pieces.len(), 1);
    assert!(document.can_redo());
}

#[test]
fn split_disjoint_mesh_reselects_restricted_group_peers_and_replays_exactly() {
    let registry = CommandRegistry::with_builtins();
    for (hidden, locked_index) in [(false, 0), (false, 1), (true, 0), (true, 1)] {
        let mut document = Document::default();
        let mesh = TriangleMesh::try_new(
            [
                [0., 0., 0.],
                [1., 0., 0.],
                [0., 1., 0.],
                [3., 0., 0.],
                [4., 0., 0.],
                [3., 1., 0.],
            ]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
            vec![[0, 1, 2], [3, 4, 5]],
            document.tolerance(),
        )
        .unwrap();
        let ids = [0, 1].map(|_| document.add_geometry(Geometry::Mesh(mesh.clone())).unwrap());
        document.add_group(None, ids).unwrap();
        if hidden {
            document
                .set_objects_visibility([ids[locked_index]], false)
                .unwrap();
        } else {
            document
                .set_objects_locked([ids[locked_index]], true)
                .unwrap();
        }
        document
            .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
            .unwrap();
        document.undo().unwrap();
        document
            .select_object(ids[1 - locked_index], SelectionMode::Replace)
            .unwrap();
        assert_eq!(document.selected_object_count(), 2);
        assert!(document.can_redo());
        let before = format!("{document:?}");
        assert!(
            registry
                .execute(&mut document, "SplitDisjointMesh unexpected")
                .is_err()
        );
        assert_eq!(format!("{document:?}"), before);
        let before_objects = document.objects().cloned().collect::<Vec<_>>();
        let before_groups = document.groups().cloned().collect::<Vec<_>>();
        assert_eq!(
            registry
                .execute(&mut document, "SplitDisjointMesh")
                .unwrap(),
            "Split 2 mesh(es) into 4 piece(s); 0 mesh(es) unchanged"
        );
        assert!(!document.can_redo());
        assert_eq!(document.objects().len(), 5);
        assert_eq!(document.selected_object_count(), 5);
        assert_eq!(
            document.object(ids[locked_index]),
            Some(&before_objects[locked_index])
        );
        assert!(document.object(ids[1 - locked_index]).is_none());
        assert_eq!(document.selectable_objects().count(), 2);
        for object in document.objects() {
            let Geometry::Mesh(piece) = object.geometry() else {
                panic!("mesh expected")
            };
            assert_eq!(
                piece.face_count(),
                if object.id() == ids[locked_index] {
                    2
                } else {
                    1
                }
            );
            assert_eq!(object.group_ids(), before_objects[0].group_ids());
        }
        let after = document.objects().cloned().collect::<Vec<_>>();
        let after_groups = document.groups().cloned().collect::<Vec<_>>();
        document.undo().unwrap();
        assert_eq!(
            document.objects().cloned().collect::<Vec<_>>(),
            before_objects
        );
        assert_eq!(
            document.groups().cloned().collect::<Vec<_>>(),
            before_groups
        );
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            ids.into_iter().collect()
        );
        document.redo().unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
        assert_eq!(document.groups().cloned().collect::<Vec<_>>(), after_groups);
        assert_eq!(document.selected_object_count(), 5);
    }
}

#[test]
fn split_disjoint_mesh_creates_fresh_piece_ids_in_face_order() {
    let mut document = Document::default();
    let registry = CommandRegistry::with_builtins();
    let vertices = [30., 0., 20.]
        .into_iter()
        .flat_map(|x| [[x, 0., 0.], [x + 1., 0., 0.], [x, 1., 0.]])
        .map(|p| Point3::try_from(p).unwrap())
        .collect();
    let mesh = TriangleMesh::try_new(
        vertices,
        vec![[0, 1, 2], [3, 4, 5], [6, 7, 8]],
        document.tolerance(),
    )
    .unwrap();
    let id = document.add_geometry(Geometry::Mesh(mesh.clone())).unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    registry
        .execute(&mut document, "SplitDisjointMesh")
        .unwrap();
    assert!(document.object(id).is_none());
    assert_eq!(document.objects().len(), 3);
    for (object, expected_x) in document.objects().zip([30., 0., 20.]) {
        let Geometry::Mesh(piece) = object.geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(piece.vertices()[0].x(), expected_x);
        assert_eq!(piece.face_count(), 1);
    }
    let after = document.objects().cloned().collect::<Vec<_>>();
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 1);
    assert_eq!(
        document.object(id).unwrap().geometry(),
        &Geometry::Mesh(mesh)
    );
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
}

#[test]
fn splits_disjoint_meshes_with_fresh_ids_attributes_groups_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New Components")
        .unwrap();
    let disjoint = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 1.0, 0.0).unwrap(),
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
            Point3::try_new(11.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 1.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2], [3, 4, 5]],
        document.tolerance(),
    )
    .unwrap();
    let connected = TriangleMesh::try_new(
        vec![
            Point3::try_new(20.0, 0.0, 0.0).unwrap(),
            Point3::try_new(21.0, 0.0, 0.0).unwrap(),
            Point3::try_new(21.0, 1.0, 0.0).unwrap(),
            Point3::try_new(20.0, 1.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
        document.tolerance(),
    )
    .unwrap();
    let first = document
        .add_geometry(Geometry::Mesh(disjoint.clone()))
        .unwrap();
    let second = document
        .add_geometry(Geometry::Mesh(connected.clone()))
        .unwrap();
    let attributes = document.object(first).unwrap().attributes().clone();
    registry.execute(&mut document, "SelMesh").unwrap();
    registry.execute(&mut document, "Group Assembly").unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "SplitDisjointMesh")
            .unwrap(),
        "Split 1 mesh(es) into 2 piece(s); 1 mesh(es) unchanged"
    );
    assert_eq!(document.undo_label(), Some("SplitDisjointMesh"));
    assert_eq!(document.objects().len(), 3);
    assert_eq!(document.selected_object_count(), 3);
    assert!(document.object(first).is_none());
    let pieces = document
        .objects()
        .map(|object| object.id())
        .filter(|id| *id != second)
        .collect::<Vec<_>>();
    assert_eq!(pieces.len(), 2);
    assert_eq!(
        document
            .group_by_name("Assembly")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([pieces[0], second, pieces[1]])
    );
    for id in pieces {
        assert_eq!(document.object(id).unwrap().attributes(), &attributes);
        let Geometry::Mesh(piece) = document.object(id).unwrap().geometry() else {
            panic!("expected split mesh piece")
        };
        assert_eq!(piece.triangles().len(), 1);
        assert_eq!(piece.vertices().len(), 3);
    }
    assert_eq!(
        document.object(second).unwrap().geometry(),
        &Geometry::Mesh(connected)
    );

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.selected_object_count(), 2);
    assert_eq!(
        document.object(first).unwrap().geometry(),
        &Geometry::Mesh(disjoint.clone())
    );
    assert_eq!(
        document
            .group_by_name("Assembly")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([first, second])
    );
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.objects().len(), 3);
    assert_eq!(
        document
            .group_by_name("Assembly")
            .unwrap()
            .members()
            .count(),
        3
    );

    let mut connected_only = Document::default();
    let connected_id = connected_only
        .add_geometry(Geometry::Mesh(disjoint.disjoint_pieces()[0].clone()))
        .unwrap();
    connected_only
        .select_object(connected_id, SelectionMode::Replace)
        .unwrap();
    let history = connected_only.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut connected_only, "SplitDisjointMesh"),
        Err(CommandError::NoDisjointMeshes)
    ));
    assert_eq!(connected_only.undo_label(), history.as_deref());

    registry.execute(&mut connected_only, "Point 9,9").unwrap();
    registry.execute(&mut connected_only, "SelAll").unwrap();
    let before = connected_only.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        registry.execute(&mut connected_only, "SplitDisjointMesh"),
        Err(CommandError::UnsupportedSplitDisjointMeshGeometry)
    ));
    assert_eq!(
        connected_only.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
}
