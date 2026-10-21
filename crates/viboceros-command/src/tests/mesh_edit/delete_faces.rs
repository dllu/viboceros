use super::*;

#[test]
fn deletes_mesh_faces_with_rhino_identity_attributes_groups_selection_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New MeshEdit")
        .unwrap();
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(99.0, 99.0, 99.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(88.0, 88.0, 88.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
            Point3::try_new(2.0, 2.0, 0.0).unwrap(),
            Point3::try_new(1.0, 1.0, 1.0).unwrap(),
        ],
        vec![[4, 1, 6], [1, 2, 6], [2, 5, 6], [5, 4, 6]],
        document.tolerance(),
    )
    .unwrap();
    let attributes = ObjectAttributes::on_layer(document.current_layer_id())
        .with_name("Editable mesh")
        .with_object_color(ColorRgb::new(19, 83, 157));
    let source = document
        .add_geometry_with_attributes(Geometry::Mesh(mesh.clone()), attributes.clone())
        .unwrap();
    let group = document
        .add_group(Some("Mesh repair".to_owned()), [source])
        .unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "DeleteFaces Faces=1")
            .unwrap(),
        "Deleted 1 face(s) from 1 object(s)"
    );
    assert_eq!(document.objects().len(), 1);
    assert!(!document.is_selected(source));
    let result = document.object(source).unwrap();
    assert_eq!(result.attributes(), &attributes);
    let Geometry::Mesh(result) = result.geometry() else {
        panic!("expected a mesh face-deletion remainder")
    };
    assert_eq!(
        result.vertices(),
        &[
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
            Point3::try_new(2.0, 2.0, 0.0).unwrap(),
            Point3::try_new(1.0, 1.0, 1.0).unwrap(),
        ]
    );
    assert_eq!(result.triangles(), &[[2, 0, 4], [1, 3, 4], [3, 2, 4]]);
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![source]
    );
    assert_eq!(document.undo_label(), Some("DeleteFaces"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(mesh.clone())
    );
    assert!(!document.is_selected(source));
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "DeleteFaces Faces=3,2,1,0")
            .unwrap(),
        "Deleted 4 face(s) from 1 object(s)"
    );
    assert!(document.object(source).is_none());
    assert_eq!(document.selected_object_count(), 0);
    assert_eq!(document.undo_label(), Some("DeleteFaces"));

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
fn deletes_brep_faces_in_source_order_with_rhino_document_behavior() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New PolysurfaceEdit")
        .unwrap();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let brep = Brep::try_box(
        frame,
        [[0.0, 2.0], [0.0, 3.0], [0.0, 5.0]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let attributes = ObjectAttributes::on_layer(document.current_layer_id())
        .with_name("Editable polysurface")
        .with_object_color(ColorRgb::new(23, 91, 161));
    let source = document
        .add_geometry_with_attributes(Geometry::Brep(brep.clone()), attributes.clone())
        .unwrap();
    let group = document
        .add_group(Some("Polysurface repair".to_owned()), [source])
        .unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "DeleteFaces Faces=1")
            .unwrap(),
        "Deleted 1 face(s) from 1 object(s)"
    );
    assert_eq!(document.objects().len(), 1);
    assert!(!document.is_selected(source));
    let result = document.object(source).unwrap();
    assert_eq!(result.attributes(), &attributes);
    assert_eq!(
        result.geometry(),
        &Geometry::Brep(
            brep.sub_brep(&[0, 2, 3, 4, 5], document.tolerance())
                .unwrap()
        )
    );
    let Geometry::Brep(result) = result.geometry() else {
        panic!("expected a B-rep face-deletion remainder")
    };
    assert!(!result.is_solid());
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![source]
    );
    assert_eq!(document.undo_label(), Some("DeleteFaces"));

    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "DeleteFaces Faces=All")
            .unwrap(),
        "Deleted 6 face(s) from 1 object(s)"
    );
    assert!(document.object(source).is_none());
    assert_eq!(document.selected_object_count(), 0);

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Brep(brep)
    );
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![source]
    );
}

#[test]
fn delete_faces_point_pick_targets_only_the_nearest_selected_mesh() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let near = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap();
    let far = TriangleMesh::try_new(
        vec![
            Point3::try_new(100.0, 0.0, 2.0).unwrap(),
            Point3::try_new(104.0, 0.0, 2.0).unwrap(),
            Point3::try_new(104.0, 4.0, 2.0).unwrap(),
            Point3::try_new(100.0, 4.0, 2.0).unwrap(),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
        document.tolerance(),
    )
    .unwrap();
    let near_id = document.add_geometry(Geometry::Mesh(near.clone())).unwrap();
    let far_id = document.add_geometry(Geometry::Mesh(far)).unwrap();
    document
        .select_objects([near_id, far_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "DeleteFaces 100.5,3.5,8")
            .unwrap(),
        "Deleted 1 face(s) from 1 object(s)"
    );
    assert_eq!(document.objects().len(), 2);
    assert_eq!(
        document.object(near_id).unwrap().geometry(),
        &Geometry::Mesh(near)
    );
    assert!(matches!(
        document.object(far_id).unwrap().geometry(),
        Geometry::Mesh(remainder) if remainder.triangles() == [[0, 1, 2]]
    ));
    assert_eq!(document.selected_object_count(), 0);
}

#[test]
fn delete_faces_point_pick_compares_mesh_and_brep_faces() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap();
    let frame = Frame3::try_from_normal(
        Point3::try_new(100.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let brep = Brep::try_box(
        frame,
        [[0.0, 2.0], [0.0, 3.0], [0.0, 5.0]],
        document.tolerance(),
    )
    .unwrap();
    let mesh_id = document.add_geometry(Geometry::Mesh(mesh.clone())).unwrap();
    let brep_id = document.add_geometry(Geometry::Brep(brep)).unwrap();
    document
        .select_objects([mesh_id, brep_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "DeleteFaces 102,1.5,2.5")
            .unwrap(),
        "Deleted 1 face(s) from 1 object(s)"
    );
    assert_eq!(
        document.object(mesh_id).unwrap().geometry(),
        &Geometry::Mesh(mesh)
    );
    assert!(matches!(
        document.object(brep_id).unwrap().geometry(),
        Geometry::Brep(remainder) if remainder.faces().len() == 5 && !remainder.is_solid()
    ));
    assert_eq!(document.selected_object_count(), 0);
}

#[test]
fn delete_faces_rejects_empty_unsupported_and_invalid_input_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert!(matches!(
        registry.execute(&mut document, "DeleteFaces Faces=All"),
        Err(CommandError::NoObjectsSelected)
    ));
    registry.execute(&mut document, "Point 0,0").unwrap();
    let point_id = document.objects().next().unwrap().id();
    document
        .select_object(point_id, SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "DeleteFaces Faces=All"),
        Err(CommandError::UnsupportedDeleteFacesGeometry)
    ));

    let first = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 1.0, 0.0).unwrap(),
            Point3::try_new(1.0, 1.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2], [1, 3, 2]],
        document.tolerance(),
    )
    .unwrap();
    let second = TriangleMesh::try_new(
        vec![
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
            Point3::try_new(11.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 1.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap();
    let first_id = document.add_geometry(Geometry::Mesh(first)).unwrap();
    let second_id = document.add_geometry(Geometry::Mesh(second)).unwrap();
    document
        .select_objects([first_id, second_id], SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "DeleteFaces Faces=1"),
        Err(CommandError::DeleteFacesFaceIndexOutOfRange {
            face: 1,
            face_count: 1,
        })
    ));
    for invalid in [
        "DeleteFaces",
        "DeleteFaces Faces=",
        "DeleteFaces Faces=0,0",
        "DeleteFaces Faces=0 1,2,3",
        "DeleteFaces Faces=0 Faces=1",
        "DeleteFaces Unknown=0",
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
}
