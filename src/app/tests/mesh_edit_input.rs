use super::*;

#[test]
fn interactive_extract_mesh_faces_uses_one_face_pick_and_copy_option() {
    let mut app = test_app();
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(4.0, 4.0, 0.0),
            point(0.0, 4.0, 0.0),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
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

    assert!(app.try_start_interactive_command("ExtractMeshFaces MakeCopy=Yes"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::ExtractMeshFaces { make_copy: true })
    );
    assert!(app.command_log.back().unwrap().contains("pick a face"));
    app.accept_drafting_point(point(0.5, 3.5, 2.0));

    assert_eq!(app.active_command, None);
    assert_eq!(app.document.objects().len(), 2);
    assert_eq!(
        app.document.object(source).unwrap().geometry(),
        &Geometry::Mesh(mesh)
    );
    assert!(!app.document.is_selected(source));
    assert!(matches!(
        app.document.selected_objects().next().unwrap().geometry(),
        Geometry::Mesh(extracted) if extracted.triangles() == [[0, 1, 2]]
    ));
    assert_eq!(app.document.undo_label(), Some("ExtractMeshFaces"));
    assert!(!app.try_start_interactive_command("ExtractMeshFaces MakeCopy=Maybe"));
    assert!(!app.try_start_interactive_command("ExtractMeshFaces MakeCopy=No MakeCopy=Yes"));
}

#[test]
fn interactive_delete_faces_uses_one_face_pick() {
    let mut app = test_app();
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(4.0, 4.0, 0.0),
            point(0.0, 4.0, 0.0),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
        app.document.tolerance(),
    )
    .unwrap();
    let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    app.document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("DeleteFaces"));
    assert_eq!(app.active_command, Some(InteractiveCommand::DeleteFaces));
    assert!(app.command_log.back().unwrap().contains("pick a face"));
    app.accept_drafting_point(point(0.5, 3.5, 2.0));

    assert_eq!(app.active_command, None);
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.is_selected(source));
    assert!(matches!(
        app.document.object(source).unwrap().geometry(),
        Geometry::Mesh(remainder) if remainder.triangles() == [[0, 1, 2]]
    ));
    assert_eq!(app.document.undo_label(), Some("DeleteFaces"));
    assert!(!app.try_start_interactive_command("DeleteFaces extra"));
}

#[test]
fn interactive_swap_mesh_edge_uses_one_topology_edge_pick() {
    let mut app = test_app();
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(4.0, 4.0, 0.0),
            point(0.0, 4.0, 0.0),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
        app.document.tolerance(),
    )
    .unwrap();
    let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    app.document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("SwapMeshEdge"));
    assert_eq!(app.active_command, Some(InteractiveCommand::SwapMeshEdge));
    assert!(app.command_log.back().unwrap().contains("interior edge"));
    app.accept_drafting_point(point(2.0, 2.0, 0.0));

    assert_eq!(app.active_command, None);
    assert!(app.document.is_selected(source));
    let Geometry::Mesh(swapped) = app.document.object(source).unwrap().geometry() else {
        panic!("expected edge-swapped mesh")
    };
    assert_eq!(swapped.triangles(), &[[0, 1, 3], [2, 3, 1]]);
    assert_eq!(app.document.undo_label(), Some("SwapMeshEdge"));
    assert!(!app.try_start_interactive_command("SwapMeshEdge Edge=1"));
}

#[test]
fn interactive_collapse_mesh_edge_uses_one_topology_edge_pick() {
    let mut app = test_app();
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 4.0, 0.0),
            point(0.0, 0.0, 4.0),
        ],
        vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        app.document.tolerance(),
    )
    .unwrap();
    let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    app.document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("CollapseMeshEdge"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::CollapseMeshEdge)
    );
    assert!(app.command_log.back().unwrap().contains("topology edge"));
    app.accept_drafting_point(point(2.0, 0.0, 0.0));

    assert_eq!(app.active_command, None);
    assert!(app.document.is_selected(source));
    let Geometry::Mesh(collapsed) = app.document.object(source).unwrap().geometry() else {
        panic!("expected edge-collapsed mesh")
    };
    assert_eq!(
        collapsed.vertices(),
        &[
            point(2.0, 0.0, 0.0),
            point(0.0, 4.0, 0.0),
            point(0.0, 0.0, 4.0),
        ]
    );
    assert_eq!(collapsed.triangles(), &[[0, 1, 2], [1, 0, 2]]);
    assert_eq!(app.document.undo_label(), Some("CollapseMeshEdge"));
    assert!(!app.try_start_interactive_command("CollapseMeshEdge Edge=0"));
}

#[test]
fn interactive_split_mesh_edge_uses_edge_and_location_picks() {
    let mut app = test_app();
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 4.0, 0.0),
            point(0.0, 0.0, 4.0),
        ],
        vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        app.document.tolerance(),
    )
    .unwrap();
    let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    app.document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("SplitMeshEdge"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::SplitMeshEdge { edge_point: None })
    );
    app.accept_drafting_point(point(2.0, 0.0, 0.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::SplitMeshEdge {
            edge_point: Some(point(2.0, 0.0, 0.0)),
        })
    );
    assert!(app.command_log.back().unwrap().contains("split location"));
    app.accept_drafting_point(point(1.0, 0.0, 0.0));

    assert_eq!(app.active_command, None);
    assert!(app.document.is_selected(source));
    let Geometry::Mesh(split) = app.document.object(source).unwrap().geometry() else {
        panic!("expected edge-split mesh")
    };
    assert_eq!(split.vertices()[4], point(1.0, 0.0, 0.0));
    assert_eq!(
        split.triangles(),
        &[
            [1, 2, 3],
            [2, 0, 3],
            [2, 4, 0],
            [2, 1, 4],
            [3, 0, 4],
            [3, 4, 1],
        ]
    );
    assert_eq!(app.document.undo_label(), Some("SplitMeshEdge"));
    assert!(!app.try_start_interactive_command("SplitMeshEdge Edge=0 Parameter=0.5"));
}

#[test]
fn interactive_fill_mesh_hole_uses_one_boundary_pick_and_options() {
    let mut app = test_app();
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 4.0, 0.0),
            point(0.0, 0.0, 4.0),
        ],
        vec![[0, 1, 3], [1, 2, 3], [2, 0, 3]],
        app.document.tolerance(),
    )
    .unwrap();
    let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    app.document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("FillMeshHole"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::FillMeshHole { join_mesh: true })
    );
    assert!(app.command_log.back().unwrap().contains("naked boundary"));
    app.accept_drafting_point(point(2.0, 0.0, 0.0));

    assert_eq!(app.active_command, None);
    assert!(app.document.is_selected(source));
    let Geometry::Mesh(filled) = app.document.object(source).unwrap().geometry() else {
        panic!("expected hole-filled mesh")
    };
    assert!(filled.topology().is_solid());
    assert_eq!(app.document.undo_label(), Some("FillMeshHole"));

    assert!(app.try_start_interactive_command("FillMeshHole JoinMesh=No"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::FillMeshHole { join_mesh: false })
    );
    app.cancel_interactive_command(false);
    assert!(!app.try_start_interactive_command("FillMeshHole Edge=0"));
    assert!(!app.try_start_interactive_command("FillMeshHole JoinMesh=Maybe"));
}

#[test]
fn interactive_weld_edge_uses_one_topology_edge_pick() {
    let mut app = test_app();
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, -3.0, 0.0),
        ],
        vec![[0, 1, 2], [3, 4, 5]],
        app.document.tolerance(),
    )
    .unwrap();
    let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    app.document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("WeldMeshEdge"));
    assert_eq!(app.active_command, Some(InteractiveCommand::WeldEdge));
    assert!(app.command_log.back().unwrap().contains("topology edge"));
    app.accept_drafting_point(point(2.0, -0.1, 0.0));

    assert_eq!(app.active_command, None);
    assert!(app.document.is_selected(source));
    let Geometry::Mesh(welded) = app.document.object(source).unwrap().geometry() else {
        panic!("expected edge-welded mesh")
    };
    assert_eq!(welded.vertices().len(), 4);
    assert_eq!(welded.triangles(), &[[0, 1, 2], [1, 0, 3]]);
    assert_eq!(app.document.undo_label(), Some("WeldEdge"));
    assert!(!app.try_start_interactive_command("WeldEdge Unknown=0"));
}

#[test]
fn interactive_weld_vertices_uses_one_topology_vertex_pick() {
    let mut app = test_app();
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, -3.0, 0.0),
        ],
        vec![[0, 1, 2], [3, 4, 5]],
        app.document.tolerance(),
    )
    .unwrap();
    let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    app.document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("WeldMeshVertex"));
    assert_eq!(app.active_command, Some(InteractiveCommand::WeldVertices));
    assert!(app.command_log.back().unwrap().contains("topology vertex"));
    app.accept_drafting_point(point(0.1, 0.1, 0.0));

    assert_eq!(app.active_command, None);
    assert!(app.document.is_selected(source));
    let Geometry::Mesh(welded) = app.document.object(source).unwrap().geometry() else {
        panic!("expected vertex-welded mesh")
    };
    assert_eq!(welded.vertices().len(), 4);
    assert_eq!(welded.triangles(), &[[2, 1, 0], [1, 2, 3]]);
    assert_eq!(app.document.undo_label(), Some("WeldVertices"));
    assert!(!app.try_start_interactive_command("WeldVertices Unknown=0"));
}

#[test]
fn interactive_unweld_edge_uses_one_topology_edge_pick() {
    let mut app = test_app();
    let stored_normal = viboceros_geometry::Vector3::try_new(1., 0., 0.).unwrap();
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(0.0, -3.0, 0.0),
        ],
        vec![[0, 1, 2], [1, 0, 3]],
        app.document.tolerance(),
    )
    .unwrap()
    .try_with_vertex_normals(Some(vec![stored_normal; 4]))
    .unwrap();
    let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    app.document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("UnweldMeshEdge ModifyNormals=No"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::UnweldEdge {
            modify_normals: false,
        })
    );
    assert!(app.command_log.back().unwrap().contains("topology edge"));
    app.accept_drafting_point(point(2.0, -0.1, 0.0));

    assert_eq!(app.active_command, None);
    assert!(app.document.is_selected(source));
    let Geometry::Mesh(unwelded) = app.document.object(source).unwrap().geometry() else {
        panic!("expected unwelded mesh")
    };
    assert_eq!(unwelded.vertices().len(), 6);
    assert!(
        unwelded
            .vertex_normals()
            .unwrap()
            .iter()
            .all(|n| *n == stored_normal)
    );
    assert_eq!(app.document.undo_label(), Some("UnweldEdge"));
    assert!(!app.try_start_interactive_command("UnweldEdge ModifyNormals=Maybe"));
    assert!(!app.try_start_interactive_command("UnweldEdge ModifyNormals=No ModifyNormals=Yes"));
}

#[test]
fn interactive_unweld_vertex_uses_one_topology_vertex_pick() {
    let mut app = test_app();
    let stored_normal = viboceros_geometry::Vector3::try_new(1., 0., 0.).unwrap();
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(0.0, -3.0, 0.0),
        ],
        vec![[0, 1, 2], [1, 0, 3]],
        app.document.tolerance(),
    )
    .unwrap()
    .try_with_vertex_normals(Some(vec![stored_normal; 4]))
    .unwrap();
    let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    app.document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("UnweldVertices ModifyNormals=No"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::UnweldVertex {
            modify_normals: false,
        })
    );
    assert!(app.command_log.back().unwrap().contains("topology vertex"));
    app.accept_drafting_point(point(0.1, 0.1, 0.0));

    assert_eq!(app.active_command, None);
    assert!(app.document.is_selected(source));
    let Geometry::Mesh(unwelded) = app.document.object(source).unwrap().geometry() else {
        panic!("expected vertex-unwelded mesh")
    };
    assert_eq!(unwelded.vertices().len(), 5);
    assert!(
        unwelded
            .vertex_normals()
            .unwrap()
            .iter()
            .all(|n| *n == stored_normal)
    );
    assert_eq!(app.document.undo_label(), Some("UnweldVertex"));
    assert!(!app.try_start_interactive_command("UnweldVertex ModifyNormals=Maybe"));
    assert!(!app.try_start_interactive_command("UnweldVertex ModifyNormals=No ModifyNormals=Yes"));
}

#[test]
fn interactive_duplicate_mesh_edge_uses_one_logical_edge_pick() {
    let mut app = test_app();
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(4.0, 3.0, 0.0),
            point(0.0, 3.0, 0.0),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
        app.document.tolerance(),
    )
    .unwrap();
    let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    app.document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("DuplicateMeshEdge BreakAngle=45"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::DupMeshEdge {
            break_angle_degrees: 45.0,
        })
    );
    assert!(app.command_log.back().unwrap().contains("logical edge"));
    app.accept_drafting_point(point(2.0, -0.1, 0.0));

    assert_eq!(app.active_command, None);
    assert!(app.document.object(source).is_some());
    assert!(!app.document.is_selected(source));
    assert_eq!(app.document.objects().len(), 2);
    let Geometry::Polyline(boundary) = app.document.selected_objects().next().unwrap().geometry()
    else {
        panic!("DupMeshEdge must create a polyline")
    };
    assert!(boundary.is_closed());
    assert_eq!(boundary.segment_count(), 4);
    assert_eq!(app.document.undo_label(), Some("DupMeshEdge"));
    assert!(!app.try_start_interactive_command("DupMeshEdge All"));
    assert!(!app.try_start_interactive_command("DupMeshEdge BreakAngle=-1"));
    assert!(!app.try_start_interactive_command("DupMeshEdge BreakAngle=20 BreakAngle=30"));
}

#[test]
fn interactive_duplicate_mesh_hole_boundary_uses_one_boundary_pick() {
    let mut app = test_app();
    let mesh = rectangular_annulus_mesh(app.document.tolerance());
    let expected = mesh
        .boundary_polylines(app.document.tolerance())
        .unwrap()
        .into_iter()
        .find(|boundary| {
            app.document
                .tolerance()
                .approx_eq(boundary.length().unwrap(), 16.0)
        })
        .unwrap();
    let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    app.document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("DuplicateMeshHoleBoundary"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::DupMeshHoleBoundary)
    );
    assert!(app.command_log.back().unwrap().contains("closed naked"));
    app.accept_drafting_point(point(3.1, 5.0, 0.0));

    assert_eq!(app.active_command, None);
    assert!(app.document.object(source).is_some());
    assert!(!app.document.is_selected(source));
    assert_eq!(app.document.objects().len(), 2);
    let output = app.document.selected_objects().next().unwrap();
    assert!(matches!(output.geometry(), Geometry::Polyline(boundary) if boundary == &expected));
    assert_eq!(app.document.undo_label(), Some("DupMeshHoleBoundary"));
    assert!(!app.try_start_interactive_command("DupMeshHoleBoundary Boundaries=All"));
}
