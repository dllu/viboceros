use super::*;

#[test]
fn extracts_non_manifold_faces_with_options_groups_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Layer New Repair").unwrap();
    let vertices = vec![
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(1.0, 0.0, 0.0).unwrap(),
        Point3::try_new(0.0, 1.0, 0.0).unwrap(),
        Point3::try_new(0.0, 0.0, 1.0).unwrap(),
        Point3::try_new(0.0, -1.0, 1.0).unwrap(),
    ];
    let faces = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3], [0, 1, 4]];
    let non_manifold = TriangleMesh::try_new(vertices, faces, document.tolerance()).unwrap();
    let clean = TriangleMesh::try_new(
        vec![
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
            Point3::try_new(11.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 1.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap();
    let source = document
        .add_geometry(Geometry::Mesh(non_manifold.clone()))
        .unwrap();
    let clean_id = document
        .add_geometry(Geometry::Mesh(clean.clone()))
        .unwrap();
    let attributes = document.object(source).unwrap().attributes().clone();
    registry.execute(&mut document, "SelMesh").unwrap();
    registry.execute(&mut document, "Group RepairSet").unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "ExtractNonManifoldMeshEdges")
            .unwrap(),
        "Extracted 3 face(s) from 1 mesh(es); 1 mesh(es) unchanged"
    );
    assert_eq!(document.undo_label(), Some("ExtractNonManifoldMeshEdges"));
    assert_eq!(document.objects().len(), 3);
    assert_eq!(document.selected_object_count(), 3);
    let extracted_id = document
        .objects()
        .map(|object| object.id())
        .find(|id| *id != source && *id != clean_id)
        .unwrap();
    assert_eq!(
        document.object(extracted_id).unwrap().attributes(),
        &attributes
    );
    let Geometry::Mesh(remainder) = document.object(source).unwrap().geometry() else {
        panic!("expected remainder mesh")
    };
    assert_eq!(remainder.triangles(), &[[0, 3, 2], [1, 2, 3]]);
    let Geometry::Mesh(extracted) = document.object(extracted_id).unwrap().geometry() else {
        panic!("expected extracted mesh")
    };
    assert_eq!(extracted.triangles().len(), 3);
    assert_eq!(
        document
            .group_by_name("RepairSet")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([source, clean_id, extracted_id])
    );
    assert_eq!(
        document.object(clean_id).unwrap().geometry(),
        &Geometry::Mesh(clean)
    );

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 2);
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(non_manifold.clone())
    );
    assert_eq!(
        document
            .group_by_name("RepairSet")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([source, clean_id])
    );

    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(
            &mut document,
            "ExtractNonManifoldMeshEdges MinimumFaceCount=4"
        ),
        Err(CommandError::NoNonManifoldMeshFaces)
    ));
    assert_eq!(document.undo_label(), history.as_deref());
    assert!(matches!(
        registry.execute(
            &mut document,
            "ExtractNonManifoldMeshEdges MinimumFaceCount=2"
        ),
        Err(CommandError::Geometry(
            GeometryError::InvalidNonManifoldMinimumFaceCount(2)
        ))
    ));

    let mut selective = Document::default();
    let selective_source = selective
        .add_geometry(Geometry::Mesh(non_manifold))
        .unwrap();
    selective
        .select_object(selective_source, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(
                &mut selective,
                "ExtractNonManifoldMeshEdges ExtractHangingFacesOnly Yes MinimumFaceCount 3"
            )
            .unwrap(),
        "Extracted 1 face(s) from 1 mesh(es); 0 mesh(es) unchanged"
    );
    assert_eq!(selective.objects().len(), 2);
    let Geometry::Mesh(selective_remainder) =
        selective.object(selective_source).unwrap().geometry()
    else {
        panic!("expected selective remainder mesh")
    };
    assert_eq!(selective_remainder.triangles().len(), 4);
    let selective_extracted = selective
        .objects()
        .find(|object| object.id() != selective_source)
        .unwrap();
    assert!(matches!(
        selective_extracted.geometry(),
        Geometry::Mesh(mesh) if mesh.triangles().len() == 1
    ));

    registry.execute(&mut selective, "Point 9,9").unwrap();
    registry.execute(&mut selective, "SelAll").unwrap();
    assert!(matches!(
        registry.execute(&mut selective, "ExtractNonManifoldMeshEdges"),
        Err(CommandError::UnsupportedExtractNonManifoldGeometry)
    ));
}

#[test]
fn extracts_duplicate_mesh_faces_with_identity_groups_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Layer New Repair").unwrap();
    let vertices = vec![
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(2.0, 0.0, 0.0).unwrap(),
        Point3::try_new(0.0, 2.0, 0.0).unwrap(),
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(2.0, 0.0, 0.0).unwrap(),
        Point3::try_new(0.0, 2.0, 0.0).unwrap(),
        Point3::try_new(0.0, 0.0, 1.0).unwrap(),
    ];
    let faces = vec![[0, 1, 2], [0, 1, 6], [3, 5, 4]];
    let duplicate = TriangleMesh::try_new(vertices, faces, document.tolerance()).unwrap();
    let clean = TriangleMesh::try_new(
        vec![
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
            Point3::try_new(11.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 1.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap();
    let source = document
        .add_geometry(Geometry::Mesh(duplicate.clone()))
        .unwrap();
    let clean_id = document
        .add_geometry(Geometry::Mesh(clean.clone()))
        .unwrap();
    let attributes = document.object(source).unwrap().attributes().clone();
    registry.execute(&mut document, "SelMesh").unwrap();
    registry
        .execute(&mut document, "Group DuplicateSet")
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "ExtractDuplicateMeshFaces")
            .unwrap(),
        "Extracted 1 duplicate face(s) from 1 mesh(es); 1 mesh(es) unchanged"
    );
    assert_eq!(document.undo_label(), Some("ExtractDuplicateMeshFaces"));
    assert_eq!(document.objects().len(), 3);
    assert_eq!(document.selected_object_count(), 3);
    let extracted_id = document
        .objects()
        .map(|object| object.id())
        .find(|id| *id != source && *id != clean_id)
        .unwrap();
    assert_eq!(
        document.object(extracted_id).unwrap().attributes(),
        &attributes
    );
    let Geometry::Mesh(remainder) = document.object(source).unwrap().geometry() else {
        panic!("expected duplicate-free remainder")
    };
    assert_eq!(remainder.triangles(), &[[0, 1, 2], [0, 1, 3]]);
    let Geometry::Mesh(extracted) = document.object(extracted_id).unwrap().geometry() else {
        panic!("expected extracted duplicate mesh")
    };
    assert_eq!(extracted.triangles(), &[[0, 2, 1]]);
    assert_eq!(
        document
            .group_by_name("DuplicateSet")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([source, clean_id, extracted_id])
    );
    assert_eq!(
        document.object(clean_id).unwrap().geometry(),
        &Geometry::Mesh(clean.clone())
    );

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 2);
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(duplicate)
    );
    assert_eq!(
        document
            .group_by_name("DuplicateSet")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([source, clean_id])
    );

    let mut clean_only = Document::default();
    let clean_only_id = clean_only.add_geometry(Geometry::Mesh(clean)).unwrap();
    clean_only
        .select_object(clean_only_id, SelectionMode::Replace)
        .unwrap();
    let history = clean_only.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut clean_only, "ExtractDuplicateMeshFaces"),
        Err(CommandError::NoDuplicateMeshFaces)
    ));
    assert_eq!(clean_only.undo_label(), history.as_deref());

    registry.execute(&mut clean_only, "Point 9,9").unwrap();
    registry.execute(&mut clean_only, "SelAll").unwrap();
    let before = clean_only.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        registry.execute(&mut clean_only, "ExtractDuplicateMeshFaces"),
        Err(CommandError::UnsupportedExtractDuplicateMeshFacesGeometry)
    ));
    assert_eq!(
        clean_only.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
}
