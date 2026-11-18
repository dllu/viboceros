use super::*;
use viboceros_geometry::MeshFace;

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn assert_mesh_matches_oracle(mesh: &TriangleMesh, expected: &serde_json::Value, id: &str) {
    let expected_vertices = expected["vertices"].as_array().unwrap();
    let expected_faces = expected["faces"].as_array().unwrap();
    assert_eq!(
        mesh.vertices().len(),
        expected_vertices.len(),
        "{id}: vertices"
    );
    assert_eq!(mesh.faces().len(), expected_faces.len(), "{id}: faces");
    for (vertex_index, (vertex, expected_vertex)) in
        mesh.vertices().iter().zip(expected_vertices).enumerate()
    {
        for (coordinate, expected_coordinate) in vertex
            .to_array()
            .into_iter()
            .zip(expected_vertex.as_array().unwrap())
        {
            assert!(
                (coordinate - expected_coordinate.as_f64().unwrap()).abs() <= 1e-6,
                "{id}: vertex {vertex_index} mismatch: native {coordinate}, Rhino {}",
                expected_coordinate.as_f64().unwrap()
            );
        }
    }
    for (face, expected_face) in mesh.faces().iter().zip(expected_faces) {
        let indices = face.indices();
        let expected_indices = expected_face.as_array().unwrap();
        assert_eq!(indices.len(), expected_indices.len(), "{id}: face size");
        for (&index, expected_index) in indices.iter().zip(expected_indices) {
            assert_eq!(index as u64, expected_index.as_u64().unwrap(), "{id}: face");
        }
    }
}

fn selected_triangle(document: &mut Document) -> ObjectId {
    let mesh = TriangleMesh::try_new_faces(
        vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
        ],
        vec![MeshFace::Triangle([0, 1, 2])],
        document.tolerance(),
    )
    .unwrap();
    let id = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    id
}

#[test]
fn solid_offset_copies_group_and_undoes_deletion() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let source = selected_triangle(&mut document);
    let group = document.add_group(None, [source]).unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "OffsetMesh 2 Solid=Yes DeleteInput=Yes")
            .unwrap(),
        "Offset 1 mesh(es)"
    );
    assert!(document.object(source).is_none());
    let result = document.selected_objects().next().unwrap();
    assert!(result.group_ids().contains(&group));
    let Geometry::Mesh(shell) = result.geometry() else {
        panic!("mesh expected")
    };
    assert!(shell.topology().is_solid());
    assert!((shell.signed_volume().unwrap() - 1.0).abs() < 1e-12);
    document.undo().unwrap();
    assert!(document.object(source).is_some());
    assert_eq!(document.objects().count(), 1);
}

#[test]
fn both_sides_and_disjoint_option_control_output_count() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let source = selected_triangle(&mut document);
    registry
        .execute(&mut document, "OffsetMesh 1 BothSides=Yes")
        .unwrap();
    assert_eq!(document.selected_objects().count(), 2);
    document.undo().unwrap();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(
            &mut document,
            "OffsetMesh 1 BothSides=Yes AllowDisjoint=Yes",
        )
        .unwrap();
    let result = document.selected_objects().next().unwrap();
    let Geometry::Mesh(mesh) = result.geometry() else {
        panic!("mesh expected")
    };
    assert_eq!(mesh.disjoint_pieces().len(), 2);
    assert_eq!(mesh.vertices()[0].z(), 1.0);
    assert_eq!(mesh.vertices()[3].z(), -1.0);
    assert_eq!(document.selected_objects().count(), 1);
    document.undo().unwrap();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "OffsetMesh 1 AllowDisjoint=Yes")
        .unwrap();
    let Geometry::Mesh(mesh) = document.selected_objects().next().unwrap().geometry() else {
        panic!("mesh expected")
    };
    assert_eq!(mesh.vertices()[0].z(), 1.0);
    assert_eq!(mesh.vertices()[3].z(), 0.0);
}

#[test]
fn invalid_options_and_nonmesh_selection_do_not_edit_document() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    selected_triangle(&mut document);
    for input in [
        "OffsetMesh 0",
        "OffsetMesh 1 DirectionMethod=UserSelectedDirection",
        "OffsetMesh 1 Direction=0,0,1",
        "OffsetMesh 1 Solid=Yes Solid=No",
        "OffsetMesh 1 DirectionMethod=UserSelectedDirection Direction=0,0,0",
    ] {
        assert!(registry.execute(&mut document, input).is_err(), "{input}");
        assert_eq!(document.objects().count(), 1);
    }
    let point_id = document
        .add_geometry(Geometry::Point(point(4.0, 0.0, 0.0)))
        .unwrap();
    document
        .select_objects_direct([point_id], SelectionMode::Add)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "OffsetMesh 1"),
        Err(CommandError::UnsupportedOffsetMeshGeometry)
    ));
    assert_eq!(document.objects().count(), 2);
}

#[test]
fn average_normal_fallback_uses_command_construction_plane() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let triangle = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
        ],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap();
    let opposed = TriangleMesh::try_append(&[&triangle, &triangle.reversed()]).unwrap();
    let id = document.add_geometry(Geometry::Mesh(opposed)).unwrap();
    document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let plane = Frame3::try_from_directions(
        point(0.0, 0.0, 0.0),
        Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    registry
        .execute_in_context(
            &mut document,
            "OffsetMesh 2 AverageNormals=Yes AllowDisjoint=Yes",
            CommandContext {
                construction_plane: plane,
            },
        )
        .unwrap();
    let Geometry::Mesh(result) = document.selected_objects().next().unwrap().geometry() else {
        panic!("mesh expected")
    };
    let offset_count = result.vertices().len() / 2;
    assert!(
        result.vertices()[..offset_count]
            .iter()
            .all(|point| point.x() <= -1.0)
    );
    assert!(
        result.vertices()[offset_count..]
            .iter()
            .all(|point| point.x() <= 1.0)
    );
    assert!(result.vertices().iter().all(|point| point.z() == 0.0));
}

#[test]
fn typed_direction_and_flip_all_move_mesh_to_opposite_side() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let source = selected_triangle(&mut document);
    registry
        .execute(
            &mut document,
            "OffsetMesh 1 DirectionMethod=UserSelectedDirection Direction=0,0,1 FlipAll=Yes",
        )
        .unwrap();
    let Geometry::Mesh(result) = document.selected_objects().next().unwrap().geometry() else {
        panic!("mesh expected")
    };
    assert!(result.vertices().iter().all(|point| point.z() == -1.0));
    assert_eq!(result.faces()[0], MeshFace::Triangle([0, 2, 1]));
    assert!(document.object(source).is_some());
}

#[test]
fn collapsed_solid_offset_keeps_rhino_two_skin_result_in_one_object() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let mesh = TriangleMesh::try_new_faces(
        vec![
            point(0., 0., 0.),
            point(1., 0., 0.),
            point(0., 1., 0.),
            point(0., 0., 1.),
        ],
        vec![MeshFace::Triangle([0, 1, 2]), MeshFace::Triangle([0, 3, 1])],
        document.tolerance(),
    )
    .unwrap();
    let source = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "OffsetMesh 1 Solid=Yes")
        .unwrap();
    let selected = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(selected.len(), 1);
    let Geometry::Mesh(result) = selected[0].geometry() else {
        panic!("offset result must be a mesh");
    };
    assert_eq!(result.vertices().len(), 8);
    assert_eq!(result.faces().len(), 4);
}

#[test]
fn command_mesh_storage_matches_rhino_offset_fixtures() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/mesh_offset.json"
    ))
    .unwrap();
    let observed: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/mesh_offset.json"
    ))
    .unwrap();
    let operations = fixture["operations"].as_array().unwrap();
    let results = observed["results"].as_array().unwrap();
    assert_eq!(operations.len(), results.len());
    let registry = CommandRegistry::with_builtins();
    for (operation, observation) in operations.iter().zip(results) {
        let id = operation["id"].as_str().unwrap();
        assert_eq!(id, observation["id"].as_str().unwrap());
        let mut document = Document::default();
        let vertices = operation["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|coordinates| {
                let coordinates = coordinates.as_array().unwrap();
                point(
                    coordinates[0].as_f64().unwrap(),
                    coordinates[1].as_f64().unwrap(),
                    coordinates[2].as_f64().unwrap(),
                )
            })
            .collect();
        let faces = operation["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|indices| {
                let indices = indices.as_array().unwrap();
                let indices = indices
                    .iter()
                    .map(|index| index.as_u64().unwrap() as u32)
                    .collect::<Vec<_>>();
                match *indices.as_slice() {
                    [a, b, c] => MeshFace::Triangle([a, b, c]),
                    [a, b, c, d] => MeshFace::Quad([a, b, c, d]),
                    _ => panic!("unsupported fixture face"),
                }
            })
            .collect();
        let mesh = TriangleMesh::try_new_faces(vertices, faces, document.tolerance()).unwrap();
        let api = &observation["value"]["api"];
        if !api.is_null() {
            let api_mesh = mesh
                .offset_mesh_rhinocommon_normal(
                    operation["distance"].as_f64().unwrap(),
                    operation["solid"].as_bool().unwrap_or(false),
                    document.tolerance(),
                )
                .unwrap();
            assert_mesh_matches_oracle(&api_mesh, api, id);
        }
        let source = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        let mut command = format!("OffsetMesh {}", operation["distance"].as_f64().unwrap());
        if operation["solid"].as_bool().unwrap_or(false) {
            command.push_str(" Solid=Yes");
        }
        if operation["macro"]
            .as_str()
            .unwrap()
            .contains("_BothSides=_Yes")
        {
            command.push_str(" BothSides=Yes");
        }
        if operation["macro"]
            .as_str()
            .unwrap()
            .contains("_AllowDisjoint=_Yes")
        {
            command.push_str(" AllowDisjoint=Yes");
        }
        if operation["macro"]
            .as_str()
            .unwrap()
            .contains("_AverageNormals=_Yes")
        {
            command.push_str(" AverageNormals=Yes");
        }
        if operation["macro"].as_str().unwrap().contains("_FlipAll") {
            command.push_str(" FlipAll=Yes");
        }
        registry.execute(&mut document, &command).unwrap();
        let expected = observation["value"]["output"].as_array().unwrap();
        let actual = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(actual.len(), expected.len(), "{id}: output count");
        for (mesh_object, expected_mesh) in actual.into_iter().zip(expected) {
            let Geometry::Mesh(mesh) = mesh_object.geometry() else {
                panic!("{id}: output must be a mesh");
            };
            assert_mesh_matches_oracle(mesh, expected_mesh, id);
        }
    }
}
