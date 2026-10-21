use super::*;

#[test]
fn extracts_selected_mesh_faces_with_rhino_identity_attributes_groups_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New MeshEdit")
        .unwrap();
    let vertices = vec![
        Point3::try_new(99.0, 99.0, 99.0).unwrap(),
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(2.0, 0.0, 0.0).unwrap(),
        Point3::try_new(88.0, 88.0, 88.0).unwrap(),
        Point3::try_new(0.0, 2.0, 0.0).unwrap(),
        Point3::try_new(2.0, 2.0, 0.0).unwrap(),
        Point3::try_new(1.0, 1.0, 1.0).unwrap(),
    ];
    let mesh = TriangleMesh::try_new(
        vertices,
        vec![[4, 1, 6], [1, 2, 6], [2, 5, 6], [5, 4, 6]],
        document.tolerance(),
    )
    .unwrap();
    let source_attributes = ObjectAttributes::on_layer(document.current_layer_id())
        .with_name("Editable mesh")
        .with_object_color(ColorRgb::new(17, 83, 149));
    let source = document
        .add_geometry_with_attributes(Geometry::Mesh(mesh.clone()), source_attributes.clone())
        .unwrap();
    let group = document
        .add_group(Some("Mesh parts".to_owned()), [source])
        .unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "ExtractMeshFaces Faces=2,0 MakeCopy=No")
            .unwrap(),
        "Extracted 2 mesh face(s) from 1 mesh(es); source faces removed"
    );
    assert_eq!(document.objects().len(), 2);
    assert!(!document.is_selected(source));
    let extracted = document.selected_objects().next().unwrap();
    let extracted_id = extracted.id();
    assert_ne!(extracted_id, source);
    assert_eq!(extracted.attributes(), &source_attributes);
    let Geometry::Mesh(extracted_mesh) = extracted.geometry() else {
        panic!("expected extracted mesh faces")
    };
    assert_eq!(extracted_mesh.triangles(), &[[1, 3, 4], [2, 0, 4]]);
    let Geometry::Mesh(remainder) = document.object(source).unwrap().geometry() else {
        panic!("expected parent mesh remainder")
    };
    assert_eq!(remainder.triangles(), &[[0, 1, 4], [3, 2, 4]]);
    assert_eq!(
        document
            .group(group)
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([source, extracted_id])
    );
    assert_eq!(document.undo_label(), Some("ExtractMeshFaces"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 1);
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(mesh.clone())
    );
    assert!(!document.is_selected(source));
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![source]
    );
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "ExtractMeshFaces Faces 1 MakeCopy Yes")
            .unwrap(),
        "Extracted 1 mesh face(s) from 1 mesh(es); source faces copied"
    );
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(mesh.clone())
    );
    assert!(!document.is_selected(source));
    let copied = document.selected_objects().next().unwrap();
    assert_eq!(copied.attributes(), &source_attributes);
    assert!(matches!(
        copied.geometry(),
        Geometry::Mesh(copied) if copied.triangles() == [[0, 1, 2]]
    ));

    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "ExtractMeshFaces Faces=3,2,1,0")
            .unwrap(),
        "Extracted 4 mesh face(s) from 1 mesh(es); source faces removed"
    );
    assert_eq!(document.objects().len(), 1);
    assert!(document.is_selected(source));
    let Geometry::Mesh(extracted_all) = document.object(source).unwrap().geometry() else {
        panic!("expected all extracted faces to reuse the source")
    };
    assert_eq!(
        extracted_all.triangles(),
        &[[3, 2, 4], [1, 3, 4], [0, 1, 4], [2, 0, 4]]
    );

    let mut already_compact = Document::default();
    let one_face = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 1.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        already_compact.tolerance(),
    )
    .unwrap();
    let one_face_id = already_compact
        .add_geometry(Geometry::Mesh(one_face.clone()))
        .unwrap();
    already_compact
        .select_object(one_face_id, SelectionMode::Replace)
        .unwrap();
    assert!(
        registry
            .execute(&mut already_compact, "ExtractMeshFaces Faces=All")
            .is_ok()
    );
    assert_eq!(already_compact.objects().len(), 1);
    assert_eq!(
        already_compact.object(one_face_id).unwrap().geometry(),
        &Geometry::Mesh(one_face)
    );
    assert!(already_compact.is_selected(one_face_id));
}

#[test]
fn extract_mesh_faces_point_pick_targets_only_the_nearest_selected_mesh() {
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
            Point3::try_new(102.0, 0.0, 2.0).unwrap(),
            Point3::try_new(100.0, 2.0, 2.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap();
    let near_id = document.add_geometry(Geometry::Mesh(near.clone())).unwrap();
    let far_id = document.add_geometry(Geometry::Mesh(far.clone())).unwrap();
    document
        .select_objects([near_id, far_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "ExtractMeshFaces 100.25,0.25,8 MakeCopy=Yes")
            .unwrap(),
        "Extracted 1 mesh face(s) from 1 mesh(es); source faces copied"
    );
    assert_eq!(document.objects().len(), 3);
    assert!(!document.is_selected(near_id));
    assert!(!document.is_selected(far_id));
    assert_eq!(
        document.object(near_id).unwrap().geometry(),
        &Geometry::Mesh(near)
    );
    assert_eq!(
        document.object(far_id).unwrap().geometry(),
        &Geometry::Mesh(far.clone())
    );
    assert_eq!(
        document.selected_objects().next().unwrap().geometry(),
        &Geometry::Mesh(far)
    );
}

#[test]
fn object_qualified_face_edits_target_one_selected_mesh() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 1.0, 0.0).unwrap(),
            Point3::try_new(0.0, 1.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
        document.tolerance(),
    )
    .unwrap();
    let source = document.add_geometry(Geometry::Mesh(mesh.clone())).unwrap();
    let other = document
        .add_geometry(Geometry::Point(Point3::try_new(9.0, 9.0, 0.0).unwrap()))
        .unwrap();
    document
        .select_objects_direct([source, other], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(
                &mut document,
                &format!("ExtractMeshFaces Face=1 Object={source} MakeCopy=Yes")
            )
            .unwrap(),
        "Extracted 1 mesh face(s) from 1 mesh(es); source faces copied"
    );
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(mesh.clone())
    );
    assert!(document.object(other).is_some());
    document.undo().unwrap();
    document
        .select_objects_direct([source, other], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(
                &mut document,
                &format!("DeleteFaces Face=1 Object={source}")
            )
            .unwrap(),
        "Deleted 1 face(s) from 1 object(s)"
    );
    let Geometry::Mesh(remainder) = document.object(source).unwrap().geometry() else {
        panic!("mesh expected")
    };
    assert_eq!(remainder.face_count(), 1);
    assert!(document.object(other).is_some());

    document.undo().unwrap();
    let box_brep = Brep::try_box(
        CommandContext::default().construction_plane,
        [[0.0, 1.0], [0.0, 1.0], [0.0, 1.0]],
        document.tolerance(),
    )
    .unwrap();
    let brep = document.add_geometry(Geometry::Brep(box_brep)).unwrap();
    document
        .select_objects_direct([source, brep], SelectionMode::Replace)
        .unwrap();
    assert!(
        registry
            .execute(&mut document, &format!("DeleteFaces Face=0 Object={other}"))
            .is_err()
    );
    assert_eq!(
        registry
            .execute(&mut document, &format!("DeleteFaces Face=0 Object={brep}"))
            .unwrap(),
        "Deleted 1 face(s) from 1 object(s)"
    );
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(mesh)
    );
    let Geometry::Brep(remainder) = document.object(brep).unwrap().geometry() else {
        panic!("B-rep expected")
    };
    assert_eq!(remainder.faces().len(), 5);
}

#[test]
fn extract_mesh_faces_rejects_empty_unsupported_and_invalid_input_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert!(matches!(
        registry.execute(&mut document, "ExtractMeshFaces Faces=All"),
        Err(CommandError::NoObjectsSelected)
    ));
    registry.execute(&mut document, "Point 0,0").unwrap();
    let point_id = document.objects().next().unwrap().id();
    document
        .select_object(point_id, SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "ExtractMeshFaces Faces=All"),
        Err(CommandError::UnsupportedExtractMeshFacesGeometry)
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
        registry.execute(&mut document, "ExtractMeshFaces Faces=1"),
        Err(CommandError::ExtractMeshFacesFaceIndexOutOfRange {
            face: 1,
            face_count: 1,
        })
    ));
    for invalid in [
        "ExtractMeshFaces",
        "ExtractMeshFaces Faces=",
        "ExtractMeshFaces Faces=0,0",
        "ExtractMeshFaces Faces=0 1,2,3",
        "ExtractMeshFaces Faces=0 MakeCopy=Maybe",
        "ExtractMeshFaces Faces=0 MakeCopy=No MakeCopy=Yes",
        "ExtractMeshFaces Unknown=0",
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
