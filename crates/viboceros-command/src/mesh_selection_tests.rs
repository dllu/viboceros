use super::*;

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
        assert_eq!(document.objects().len(), 4);
        assert_eq!(document.selected_object_count(), 4);
        assert_eq!(document.selectable_objects().count(), 2);
        for object in document.objects() {
            let Geometry::Mesh(piece) = object.geometry() else {
                panic!("mesh expected")
            };
            assert_eq!(piece.face_count(), 1);
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
        assert_eq!(document.selected_object_count(), 4);
    }
}

#[test]
fn split_disjoint_mesh_moves_first_piece_and_keeps_remaining_face_order() {
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
    assert_eq!(document.objects().next().unwrap().id(), id);
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
fn collapse_locked_group_peers_follow_explicit_deletion_policy_and_restore_on_undo() {
    let registry = CommandRegistry::with_builtins();
    for locked_index in [0, 2] {
        let mut document = Document::default();
        let vertices = [[0., 0., 0.], [2., 0., 0.], [0., 2., 0.], [0., 0., 2.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec();
        let ids = [false, true, false].map(|solid| {
            let faces = if solid {
                vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]]
            } else {
                vec![[0, 1, 2]]
            };
            let mesh =
                TriangleMesh::try_new(vertices.clone(), faces, document.tolerance()).unwrap();
            document.add_geometry(Geometry::Mesh(mesh)).unwrap()
        });
        document.add_group(None, ids).unwrap();
        document
            .set_objects_locked([ids[locked_index]], true)
            .unwrap();
        document
            .add_geometry(Geometry::Point(Point3::try_new(99., 0., 0.).unwrap()))
            .unwrap();
        document.undo().unwrap();
        document
            .select_object(ids[1], SelectionMode::Replace)
            .unwrap();
        assert_eq!(document.selected_object_count(), 3);
        assert!(document.can_redo());
        let before = document.objects().cloned().collect::<Vec<_>>();
        let groups = document.groups().cloned().collect::<Vec<_>>();
        let selection = document.selected_object_ids().collect::<BTreeSet<_>>();
        let result = registry.execute(&mut document, "CollapseMeshEdge Edge=0");
        assert_eq!(
            result.unwrap(),
            "Collapsed 3 mesh edge(s) in 3 mesh(es); deleted 2 empty mesh(es)"
        );
        assert!(!document.can_redo());
        assert!(document.object(ids[locked_index]).is_none());
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [ids[1]]);
        let after = document.objects().cloned().collect::<Vec<_>>();
        let after_groups = document.groups().cloned().collect::<Vec<_>>();
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(document.groups().cloned().collect::<Vec<_>>(), groups);
        // History exchanges selected membership; it does not rewind the
        // action order of surviving versus restored objects.
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            selection
        );
        assert!(
            document
                .object(ids[locked_index])
                .unwrap()
                .attributes()
                .is_locked()
        );
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
        assert_eq!(document.groups().cloned().collect::<Vec<_>>(), after_groups);
    }
}

#[test]
fn collapse_mixed_replacements_and_deletions_replays_exactly() {
    let mut document = Document::default();
    let registry = CommandRegistry::with_builtins();
    let vertices = [[0., 0., 0.], [2., 0., 0.], [0., 2., 0.], [0., 0., 2.]]
        .map(|p| Point3::try_from(p).unwrap())
        .to_vec();
    let mut ids = Vec::new();
    for solid in [false, true, false] {
        let faces = if solid {
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]]
        } else {
            vec![[0, 1, 2]]
        };
        let mesh = TriangleMesh::try_new(vertices.clone(), faces, document.tolerance()).unwrap();
        ids.push(document.add_geometry(Geometry::Mesh(mesh)).unwrap());
        document
            .add_geometry(Geometry::Point(Point3::try_new(99., 99., 99.).unwrap()))
            .unwrap();
    }
    let group = document.add_group(None, ids.iter().copied()).unwrap();
    document
        .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let groups = document.groups().cloned().collect::<Vec<_>>();
    assert_eq!(
        registry
            .execute(&mut document, "CollapseMeshEdge Edge=0")
            .unwrap(),
        "Collapsed 3 mesh edge(s) in 3 mesh(es); deleted 2 empty mesh(es)"
    );
    assert!(document.object(ids[0]).is_none());
    assert!(document.object(ids[2]).is_none());
    let Geometry::Mesh(mesh) = document.object(ids[1]).unwrap().geometry() else {
        panic!("mesh expected")
    };
    assert_eq!(mesh.face_count(), 2);
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        [ids[1]]
    );
    assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [ids[1]]);
    let after = document.objects().cloned().collect::<Vec<_>>();
    let after_groups = document.groups().cloned().collect::<Vec<_>>();
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(document.groups().cloned().collect::<Vec<_>>(), groups);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
    assert_eq!(document.groups().cloned().collect::<Vec<_>>(), after_groups);
    assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [ids[1]]);
}

#[test]
fn mesh_staging_retains_action_order_group_peers_and_read_only_failures() {
    let mut document = Document::default();
    let ids = (0..20)
        .map(|i| {
            let x = i as f64;
            let mesh = TriangleMesh::try_new(
                [[x, 0., 0.], [x + 1., 0., 0.], [x, 1., 0.]]
                    .map(|p| Point3::try_from(p).unwrap())
                    .to_vec(),
                vec![[0, 1, 2]],
                Tolerance::DEFAULT,
            )
            .unwrap();
            document.add_geometry(Geometry::Mesh(mesh)).unwrap()
        })
        .collect::<Vec<_>>();
    document.add_group(None, [ids[0], ids[1]]).unwrap();
    document.set_objects_visibility([ids[1]], false).unwrap();
    for &id in ids[2..].iter().rev() {
        document.select_object(id, SelectionMode::Add).unwrap();
    }
    document.select_object(ids[0], SelectionMode::Add).unwrap();
    let expected = document.selected_object_ids().collect::<Vec<_>>();
    assert_eq!(expected.len(), 20);
    assert_eq!(expected[0], ids[19]);
    assert!(expected.contains(&ids[1]));
    let before = format!("{document:?}");
    let unsupported = || CommandError::Usage("expected mesh");
    let sources = selected_mesh_face_sources(&document, unsupported).unwrap();
    let topology = selected_mesh_topology_sources(&document, unsupported).unwrap();
    assert_eq!(
        topology.iter().map(|source| source.id).collect::<Vec<_>>(),
        expected
    );
    for source in topology {
        let Geometry::Mesh(mesh) = document.object(source.id).unwrap().geometry() else {
            unreachable!()
        };
        assert!(std::ptr::eq(source.mesh, mesh));
    }
    assert_eq!(
        sources.iter().map(|source| source.id).collect::<Vec<_>>(),
        expected
    );
    let inputs =
        stage_selected_mesh_face_extractions(&document, unsupported, |_| Ok(None)).unwrap();
    assert_eq!(
        inputs.iter().map(|input| input.id).collect::<Vec<_>>(),
        expected
    );
    for (source, input) in sources.iter().zip(&inputs) {
        let object = document.object(source.id).unwrap();
        let Geometry::Mesh(mesh) = object.geometry() else {
            unreachable!()
        };
        assert!(std::ptr::eq(source.mesh, mesh));
        assert!(std::ptr::eq(source.attributes, object.attributes()));
        assert!(std::ptr::eq(source.group_ids, object.group_ids()));
        assert_eq!(Geometry::Mesh(source.mesh.clone()), *object.geometry());
        assert_eq!(source.attributes, object.attributes());
        assert_eq!(source.group_ids, object.group_ids());
        assert_eq!(input.attributes, *source.attributes);
        assert_eq!(input.group_ids, source.group_ids);
        assert!(input.extraction.is_none());
    }
    let mut calls = 0;
    assert!(
        stage_selected_mesh_face_extractions(&document, unsupported, |_| {
            calls += 1;
            if calls == 3 {
                Err(GeometryError::NumericalIntegrationDidNotConverge)
            } else {
                Ok(None)
            }
        })
        .is_err()
    );
    assert_eq!(calls, 3);
    assert_eq!(format!("{document:?}"), before);
    let point = document
        .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
        .unwrap();
    document.select_object(point, SelectionMode::Add).unwrap();
    let before = format!("{document:?}");
    assert!(matches!(
        selected_mesh_topology_sources(&document, unsupported),
        Err(CommandError::Usage("expected mesh"))
    ));
    assert_eq!(format!("{document:?}"), before);
    document.clear_selection();
    assert!(matches!(
        selected_mesh_topology_sources(&document, unsupported),
        Err(CommandError::NoObjectsSelected)
    ));
}
