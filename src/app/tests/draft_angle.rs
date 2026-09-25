use super::*;

fn facing_pair(app: &mut VibocerosApp) -> ObjectId {
    let point = |x, y| Point3::try_new(x, y, 0.0).unwrap();
    let mesh = TriangleMesh::try_new_faces(
        vec![
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(0.0, 1.0),
            point(2.0, 0.0),
            point(3.0, 0.0),
            point(2.0, 1.0),
        ],
        vec![MeshFace::Triangle([0, 1, 2]), MeshFace::Triangle([3, 5, 4])],
        app.document.tolerance(),
    )
    .unwrap();
    let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    source
}

#[test]
fn draft_angle_uses_active_view_and_logs_the_direction_used() {
    let mut app = test_app();
    let source = facing_pair(&mut app);
    assert!(app.try_execute_command("ExtractMeshFacesByDraftAngle StartAngle=0 EndAngle=0"));
    let selected = app.document.selected_objects().next().unwrap();
    let Geometry::Mesh(top_face) = selected.geometry() else {
        panic!("mesh expected")
    };
    assert_eq!(top_face.face_count(), 1);
    assert_eq!(top_face.polygon_face_normals().unwrap()[0].z(), 1.0);
    assert!(
        app.command_log
            .iter()
            .any(|entry| entry.contains("ViewDirection=0,0,1"))
    );

    app.document.undo().unwrap();
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.viewports[0].set_world_view(ViewKind::Bottom);
    assert!(app.try_execute_command("ExtractMeshFacesByDraftAngle StartAngle=0 EndAngle=0"));
    let selected = app.document.selected_objects().next().unwrap();
    let Geometry::Mesh(bottom_face) = selected.geometry() else {
        panic!("mesh expected")
    };
    assert_eq!(bottom_face.polygon_face_normals().unwrap()[0].z(), -1.0);
    assert!(
        app.command_log
            .iter()
            .any(|entry| entry.contains("ViewDirection=0,0,-1"))
    );
}

#[test]
fn viewward_directions_follow_front_back_and_perspective_cameras() {
    let front = Viewport::new(ViewKind::Front).viewward_direction();
    let back = Viewport::new(ViewKind::Back).viewward_direction();
    assert_eq!(front.to_array(), [0.0, -1.0, 0.0]);
    assert_eq!(back.to_array(), [0.0, 1.0, 0.0]);
    let perspective = Viewport::new(ViewKind::Perspective).viewward_direction();
    assert!(perspective.x() > 0.0 && perspective.y() < 0.0 && perspective.z() > 0.0);
    assert!((perspective.length().unwrap() - 1.0).abs() < 1.0e-12);
}
