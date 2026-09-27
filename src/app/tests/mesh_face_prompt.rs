use super::*;

#[test]
fn connected_mesh_face_prompt_picks_planar_region_with_options() {
    let mut app = test_app();
    let mesh = TriangleMesh::try_new_faces(
        vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(1.0, 1.0, 0.0),
            point(1.0, 1.0, 1.0),
        ],
        vec![
            MeshFace::Triangle([0, 1, 2]),
            MeshFace::Triangle([1, 3, 2]),
            MeshFace::Triangle([1, 4, 3]),
        ],
        app.document.tolerance(),
    )
    .unwrap();
    let source = app
        .document
        .add_geometry(Geometry::Mesh(mesh.clone()))
        .unwrap();
    app.document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("ExtractConnectedMeshFaces Angle=0.1 MakeCopy=Yes"));
    assert!(app.command_log.back().unwrap().contains("pick a face"));
    app.accept_drafting_point(point(0.2, 0.2, 0.0));

    assert_eq!(app.active_command, None);
    assert_eq!(app.document.undo_label(), Some("ExtractConnectedMeshFaces"));
    assert_eq!(
        app.document.object(source).unwrap().geometry(),
        &Geometry::Mesh(mesh)
    );
    let selected = app.document.selected_objects().collect::<Vec<_>>();
    assert_eq!(selected.len(), 1);
    assert!(matches!(
        selected[0].geometry(),
        Geometry::Mesh(extracted) if extracted.face_count() == 2
    ));
    assert!(!app.try_start_interactive_command("ExtractConnectedMeshFaces Face=0"));
    assert!(!app.try_start_interactive_command("ExtractConnectedMeshFaces Angle=-1"));
    assert!(
        !app.try_start_interactive_command("ExtractConnectedMeshFaces MakeCopy=Yes MakeCopy=No")
    );
}

#[test]
fn mesh_part_prompt_picks_through_unwelded_seam_when_requested() {
    let mut app = test_app();
    let mesh = TriangleMesh::try_new_faces(
        vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(1.0, 1.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(1.0, 1.0, 0.0),
            point(2.0, 0.0, 0.0),
        ],
        vec![
            MeshFace::Triangle([0, 1, 2]),
            MeshFace::Triangle([1, 3, 2]),
            MeshFace::Triangle([4, 6, 5]),
        ],
        app.document.tolerance(),
    )
    .unwrap();
    let source = app
        .document
        .add_geometry(Geometry::Mesh(mesh.clone()))
        .unwrap();
    app.document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command(
        "ExtractMeshPart ExtractWholeDisjointParts=Yes MakeCopy=Yes"
    ));
    assert!(app.command_log.back().unwrap().contains("pick a face"));
    app.accept_drafting_point(point(0.2, 0.2, 0.0));

    assert_eq!(app.active_command, None);
    assert_eq!(app.document.undo_label(), Some("ExtractMeshPart"));
    assert_eq!(
        app.document.object(source).unwrap().geometry(),
        &Geometry::Mesh(mesh)
    );
    let selected = app.document.selected_objects().collect::<Vec<_>>();
    assert_eq!(selected.len(), 1);
    assert!(matches!(
        selected[0].geometry(),
        Geometry::Mesh(extracted) if extracted.face_count() == 3
    ));
    assert!(!app.try_start_interactive_command("ExtractMeshPart Face=0"));
    assert!(!app.try_start_interactive_command("ExtractMeshPart JoinOutput=Maybe"));
    assert!(!app.try_start_interactive_command("ExtractMeshPart MakeCopy=Yes MakeCopy=No"));
}

#[test]
fn viewport_face_hit_runs_each_region_command_on_the_hit_object() {
    for command in ["ExtractConnectedMeshFaces", "ExtractMeshPart"] {
        let mut app = test_app();
        let mesh = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![MeshFace::Triangle([0, 1, 2])],
            app.document.tolerance(),
        )
        .unwrap();
        let target = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        let other = app
            .document
            .add_geometry(Geometry::Point(point(2.0, 2.0, 0.0)))
            .unwrap();
        app.document
            .select_objects_direct([target, other], viboceros_document::SelectionMode::Replace)
            .unwrap();
        assert!(app.try_start_interactive_command(&format!("{command} MakeCopy=Yes")));
        app.accept_mesh_face_click(target, 0);
        assert_eq!(app.active_command, None);
        assert_eq!(app.document.undo_label(), Some(command));
        let selected = app.document.selected_objects().collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        assert!(matches!(selected[0].geometry(), Geometry::Mesh(mesh) if mesh.face_count() == 1));
        assert!(app.document.object(other).is_some());
    }
}

#[test]
fn viewport_face_hit_extracts_or_deletes_the_hit_mesh_face() {
    for command in ["ExtractMeshFaces", "DeleteFaces"] {
        let mut app = test_app();
        let mesh = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![MeshFace::Triangle([0, 1, 2]), MeshFace::Triangle([0, 2, 3])],
            app.document.tolerance(),
        )
        .unwrap();
        let target = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        let other = app
            .document
            .add_geometry(Geometry::Point(point(2.0, 2.0, 0.0)))
            .unwrap();
        app.document
            .select_objects_direct([target, other], viboceros_document::SelectionMode::Replace)
            .unwrap();
        assert!(app.try_start_interactive_command(command));
        app.accept_mesh_face_click(target, 1);
        assert_eq!(app.active_command, None);
        assert_eq!(app.document.undo_label(), Some(command));
        assert!(app.document.object(other).is_some());
        if command == "DeleteFaces" {
            assert!(matches!(
                app.document.object(target).unwrap().geometry(),
                Geometry::Mesh(mesh) if mesh.face_count() == 1
            ));
        } else {
            assert_eq!(app.document.selected_objects().count(), 1);
            assert!(matches!(
                app.document.selected_objects().next().unwrap().geometry(),
                Geometry::Mesh(mesh) if mesh.face_count() == 1
            ));
        }
    }
}
