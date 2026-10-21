use super::*;

#[test]
fn triangulates_mesh_quads_with_rhino_order_identity_groups_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New Triangulation")
        .unwrap();
    let vertices = vec![
        Point3::try_new(-3.0, 0.0, 0.0).unwrap(),
        Point3::try_new(-2.0, 0.0, 0.0).unwrap(),
        Point3::try_new(-3.0, 1.0, 0.0).unwrap(),
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(4.0, 0.0, 0.0).unwrap(),
        Point3::try_new(1.0, 1.0, 0.0).unwrap(),
        Point3::try_new(0.0, 2.0, 0.0).unwrap(),
        Point3::try_new(10.0, 0.0, 0.0).unwrap(),
        Point3::try_new(11.0, 0.0, 0.0).unwrap(),
        Point3::try_new(12.0, 2.0, 0.0).unwrap(),
        Point3::try_new(10.0, 1.0, 0.0).unwrap(),
        Point3::try_new(99.0, 99.0, 99.0).unwrap(),
    ];
    let mesh = TriangleMesh::try_new_faces(
        vertices.clone(),
        vec![
            viboceros_geometry::MeshFace::Triangle([0, 1, 2]),
            viboceros_geometry::MeshFace::Quad([3, 4, 5, 6]),
            viboceros_geometry::MeshFace::Quad([7, 8, 9, 10]),
        ],
        document.tolerance(),
    )
    .unwrap();
    let triangles = TriangleMesh::try_new(
        vec![
            Point3::try_new(20.0, 0.0, 0.0).unwrap(),
            Point3::try_new(21.0, 0.0, 0.0).unwrap(),
            Point3::try_new(20.0, 1.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap();
    let attributes = ObjectAttributes::on_layer(document.current_layer_id())
        .with_name("Mixed polygon mesh")
        .with_object_color(ColorRgb::new(31, 101, 173));
    let source = document
        .add_geometry_with_attributes(Geometry::Mesh(mesh.clone()), attributes.clone())
        .unwrap();
    let unchanged = document
        .add_geometry(Geometry::Mesh(triangles.clone()))
        .unwrap();
    let group = document
        .add_group(Some("Triangulation group".to_owned()), [source])
        .unwrap();
    document
        .select_objects_direct([source, unchanged], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "TriangulateMesh").unwrap(),
        "Triangulated 2 quadrilateral face(s) in 1 mesh(es); 1 mesh(es) unchanged"
    );
    assert!(document.is_selected(source));
    assert!(document.is_selected(unchanged));
    let result = document.object(source).unwrap();
    assert_eq!(result.attributes(), &attributes);
    let Geometry::Mesh(result) = result.geometry() else {
        panic!("expected a triangulated mesh")
    };
    assert_eq!(result.vertices(), vertices);
    assert_eq!(
        result.faces(),
        &[
            viboceros_geometry::MeshFace::Triangle([0, 1, 2]),
            viboceros_geometry::MeshFace::Triangle([3, 4, 5]),
            viboceros_geometry::MeshFace::Triangle([7, 8, 10]),
            viboceros_geometry::MeshFace::Triangle([3, 5, 6]),
            viboceros_geometry::MeshFace::Triangle([8, 9, 10]),
        ]
    );
    assert_eq!(
        document.object(unchanged).unwrap().geometry(),
        &Geometry::Mesh(triangles.clone())
    );
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![source]
    );
    assert_eq!(document.undo_label(), Some("TriangulateMesh"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(mesh)
    );
    document
        .select_object(unchanged, SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert_eq!(
        registry.execute(&mut document, "TriangulateMesh").unwrap(),
        "Triangulated 0 quadrilateral face(s) in 0 mesh(es); 1 mesh(es) unchanged"
    );
    assert_eq!(document.undo_label(), history.as_deref());
    assert_eq!(
        document.object(unchanged).unwrap().geometry(),
        &Geometry::Mesh(triangles)
    );
}

#[test]
fn triangulate_mesh_rejects_empty_mixed_and_invalid_input_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert!(matches!(
        registry.execute(&mut document, "TriangulateMesh"),
        Err(CommandError::NoObjectsSelected)
    ));
    let mesh = TriangleMesh::try_new_faces(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 1.0, 0.0).unwrap(),
            Point3::try_new(0.0, 1.0, 0.0).unwrap(),
        ],
        vec![viboceros_geometry::MeshFace::Quad([0, 1, 2, 3])],
        document.tolerance(),
    )
    .unwrap();
    let mesh_id = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    let point_id = document
        .add_geometry(Geometry::Point(Point3::try_new(5.0, 5.0, 5.0).unwrap()))
        .unwrap();
    document
        .select_objects_direct([mesh_id, point_id], SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "TriangulateMesh"),
        Err(CommandError::UnsupportedTriangulateMeshGeometry)
    ));
    assert!(
        registry
            .execute(&mut document, "TriangulateMesh extra")
            .is_err()
    );
    assert_eq!(
        document.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    assert_eq!(document.undo_label(), history.as_deref());
}
