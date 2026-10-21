use super::*;

#[test]
fn fills_mesh_holes_in_place_with_identity_groups_selection_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 4.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 4.0).unwrap(),
        ],
        vec![[0, 1, 3], [1, 2, 3], [2, 0, 3]],
        document.tolerance(),
    )
    .unwrap();
    let attributes = ObjectAttributes::on_layer(document.current_layer_id())
        .with_name("Hole target")
        .with_object_color(ColorRgb::new(31, 117, 211));
    let source = document
        .add_geometry_with_attributes(Geometry::Mesh(mesh.clone()), attributes.clone())
        .unwrap();
    let group = document
        .add_group(Some("Repair group".to_owned()), [source])
        .unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "FillMeshHole Edge=0")
            .unwrap(),
        "Filled 1 mesh hole(s) with 1 triangle(s) in 1 joined mesh(es)"
    );
    assert!(document.is_selected(source));
    assert_eq!(document.object(source).unwrap().attributes(), &attributes);
    let Geometry::Mesh(filled) = document.object(source).unwrap().geometry() else {
        panic!("expected hole-filled mesh")
    };
    assert_eq!(filled.face_count(), 4);
    assert_eq!(filled.vertices().len(), 8);
    assert!(filled.topology().is_solid());
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![source]
    );
    assert_eq!(document.undo_label(), Some("FillMeshHole"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(mesh)
    );
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![source]
    );
}

#[test]
fn fill_mesh_hole_point_pick_can_create_an_independent_patch() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let make_open_tetrahedron = |offset: Real| {
        TriangleMesh::try_new(
            vec![
                Point3::try_new(offset, 0.0, 0.0).unwrap(),
                Point3::try_new(offset + 4.0, 0.0, 0.0).unwrap(),
                Point3::try_new(offset, 4.0, 0.0).unwrap(),
                Point3::try_new(offset, 0.0, 4.0).unwrap(),
            ],
            vec![[0, 1, 3], [1, 2, 3], [2, 0, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap()
    };
    let first_mesh = make_open_tetrahedron(0.0);
    let second_mesh = make_open_tetrahedron(10.0);
    let first = document
        .add_geometry(Geometry::Mesh(first_mesh.clone()))
        .unwrap();
    let attributes = ObjectAttributes::on_layer(document.current_layer_id())
        .with_name("Separate cap")
        .with_object_color(ColorRgb::new(17, 29, 43));
    let second = document
        .add_geometry_with_attributes(Geometry::Mesh(second_mesh.clone()), attributes.clone())
        .unwrap();
    let group = document
        .add_group(Some("Source only".to_owned()), [second])
        .unwrap();
    document
        .select_objects_direct([first, second], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "FillMeshHole 11,0,0 JoinMesh=No")
            .unwrap(),
        "Filled 1 mesh hole(s) with 1 triangle(s) in separate patch mesh(es)"
    );
    assert_eq!(document.objects().len(), 3);
    assert_eq!(
        document.object(first).unwrap().geometry(),
        &Geometry::Mesh(first_mesh.clone())
    );
    assert_eq!(
        document.object(second).unwrap().geometry(),
        &Geometry::Mesh(second_mesh.clone())
    );
    let patch = document
        .objects()
        .find(|object| object.id() != first && object.id() != second)
        .unwrap();
    assert_eq!(patch.attributes(), &attributes);
    assert!(document.is_selected(patch.id()));
    assert_eq!(document.selected_object_count(), 1);
    assert!(matches!(patch.geometry(), Geometry::Mesh(mesh) if mesh.face_count() == 1));
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![second]
    );

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 2);
    assert_eq!(
        document.object(first).unwrap().geometry(),
        &Geometry::Mesh(first_mesh)
    );
    assert_eq!(
        document.object(second).unwrap().geometry(),
        &Geometry::Mesh(second_mesh)
    );
}

#[test]
fn fill_mesh_hole_rejects_invalid_non_naked_and_unsupported_input_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert!(matches!(
        registry.execute(&mut document, "FillMeshHole Edge=0"),
        Err(CommandError::NoObjectsSelected)
    ));
    let square = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 4.0, 0.0).unwrap(),
            Point3::try_new(0.0, 4.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
        document.tolerance(),
    )
    .unwrap();
    let source = document
        .add_geometry(Geometry::Mesh(square.clone()))
        .unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "FillMeshHole 2,2,0"),
        Err(CommandError::NoFillableMeshHoles)
    ));
    assert!(matches!(
        registry.execute(&mut document, "FillMeshHole Edge=5"),
        Err(CommandError::FillMeshHoleEdgeIndexOutOfRange {
            edge: 5,
            edge_count: 5,
        })
    ));
    for invalid in [
        "FillMeshHole",
        "FillMeshHole Edge=",
        "FillMeshHole Edge=-1",
        "FillMeshHole Edge=0 Edge=1",
        "FillMeshHole Edge=0 1,2,3",
        "FillMeshHole Edge=0 JoinMesh=Maybe",
        "FillMeshHole Edge=0 JoinMesh=Yes JoinMesh=No",
        "FillMeshHole Unknown=0",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
    }
    assert_eq!(
        document.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    assert_eq!(document.undo_label(), history.as_deref());

    let point = document
        .add_geometry(Geometry::Point(Point3::try_new(9.0, 9.0, 9.0).unwrap()))
        .unwrap();
    document
        .select_objects_direct([source, point], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "FillMeshHole Edge=0"),
        Err(CommandError::UnsupportedFillMeshHoleGeometry)
    ));
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(square)
    );
}

#[test]
fn fill_mesh_holes_repairs_every_selected_boundary_and_preserves_document_state() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let open = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 4.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 4.0).unwrap(),
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
            Point3::try_new(14.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 4.0, 0.0).unwrap(),
            Point3::try_new(10.0, 0.0, 4.0).unwrap(),
        ],
        vec![
            [0, 1, 3],
            [1, 2, 3],
            [2, 0, 3],
            [4, 5, 7],
            [5, 6, 7],
            [6, 4, 7],
        ],
        document.tolerance(),
    )
    .unwrap();
    let closed = TriangleMesh::try_new(
        vec![
            Point3::try_new(20.0, 0.0, 0.0).unwrap(),
            Point3::try_new(24.0, 0.0, 0.0).unwrap(),
            Point3::try_new(20.0, 4.0, 0.0).unwrap(),
            Point3::try_new(20.0, 0.0, 4.0).unwrap(),
        ],
        vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        document.tolerance(),
    )
    .unwrap();
    let attributes = ObjectAttributes::on_layer(document.current_layer_id())
        .with_name("Two repairs")
        .with_object_color(ColorRgb::new(23, 61, 149));
    let open_id = document
        .add_geometry_with_attributes(Geometry::Mesh(open.clone()), attributes.clone())
        .unwrap();
    let closed_id = document
        .add_geometry(Geometry::Mesh(closed.clone()))
        .unwrap();
    let group = document
        .add_group(Some("Repair candidates".to_owned()), [open_id, closed_id])
        .unwrap();
    document
        .select_objects_direct([open_id, closed_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "FillMeshHoles").unwrap(),
        "Filled 2 mesh hole(s) with 2 triangle(s) in 1 mesh(es); 1 mesh(es) unchanged"
    );
    let Geometry::Mesh(filled) = document.object(open_id).unwrap().geometry() else {
        panic!("expected repaired mesh")
    };
    assert_eq!(filled.face_count(), 8);
    assert_eq!(filled.vertices().len(), 14);
    assert!(filled.topology().is_solid());
    assert_eq!(document.object(open_id).unwrap().attributes(), &attributes);
    assert_eq!(
        document.object(closed_id).unwrap().geometry(),
        &Geometry::Mesh(closed)
    );
    assert!(document.is_selected(open_id));
    assert!(document.is_selected(closed_id));
    assert_eq!(
        document
            .group(group)
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([open_id, closed_id])
    );
    assert_eq!(document.undo_label(), Some("FillMeshHoles"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(open_id).unwrap().geometry(),
        &Geometry::Mesh(open)
    );
    assert_eq!(
        document
            .group(group)
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([open_id, closed_id])
    );
}

#[test]
fn fill_mesh_holes_rejects_noop_arguments_and_mixed_geometry_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert!(matches!(
        registry.execute(&mut document, "FillMeshHoles"),
        Err(CommandError::NoObjectsSelected)
    ));
    let closed = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 4.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 4.0).unwrap(),
        ],
        vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        document.tolerance(),
    )
    .unwrap();
    let mesh_id = document
        .add_geometry(Geometry::Mesh(closed.clone()))
        .unwrap();
    document
        .select_object(mesh_id, SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "FillMeshHoles"),
        Err(CommandError::NoFillableMeshHoles)
    ));
    assert!(matches!(
        registry.execute(&mut document, "FillMeshHoles Unexpected"),
        Err(CommandError::Usage("FillMeshHoles"))
    ));

    let point_id = document
        .add_geometry(Geometry::Point(Point3::try_new(9.0, 9.0, 9.0).unwrap()))
        .unwrap();
    document
        .select_objects_direct([mesh_id, point_id], SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "FillMeshHoles"),
        Err(CommandError::UnsupportedFillMeshHolesGeometry)
    ));
    assert_eq!(
        document.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    assert_eq!(document.undo_label(), history.as_deref());
    assert_eq!(
        document.object(mesh_id).unwrap().geometry(),
        &Geometry::Mesh(closed)
    );
}
