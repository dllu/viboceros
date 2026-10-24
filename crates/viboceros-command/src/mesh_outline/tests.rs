use super::*;
use viboceros_io::{ThreeDmNamedView, ThreeDmProjection};

#[test]
fn later_unbounded_mesh_rolls_back_selection_geometry_and_redo_history() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let mut ids = Vec::new();
    for points in [
        [p(0., 0., 2.), p(2., 0., 2.), p(0., 2., 2.)],
        [p(0., 0., 9.), p(2., 0., 12.), p(0., 2., 9.)],
    ] {
        ids.push(
            doc.add_geometry(Geometry::Mesh(
                TriangleMesh::try_new(points.to_vec(), vec![[0, 1, 2]], Tolerance::MESH_VALIDATION)
                    .unwrap(),
            ))
            .unwrap(),
        );
    }
    registry.execute(&mut doc, "Point 8,8,8").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    doc.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    let plane = CommandContext::default().construction_plane;
    let view = ThreeDmNamedView {
        name: "Atomic outline".into(),
        projection: ThreeDmProjection::Perspective,
        camera_location: p(0., 0., 10.),
        camera_direction: Vector3::try_new(0., 0., -1.).unwrap(),
        camera_up: Vector3::try_new(0., 1., 0.).unwrap(),
        target: Some(p(0., 0., 0.)),
        construction_plane: plane,
        frustum: [-1., 1., -1., 1., 1., 20.],
        screen_port: [0, 800, 0, 600],
    };
    let projection = picking::Projection::from_view(&view).unwrap();
    let before = format!("{doc:?}");
    assert!(matches!(
        registry.execute_with_pick_projection(
            &mut doc,
            "MeshOutline",
            Default::default(),
            false,
            projection
        ),
        Err(CommandError::Geometry(
            GeometryError::MeshOutlineCrossesEyePlane
        ))
    ));
    assert_eq!(format!("{doc:?}"), before);
    assert_eq!(doc.redo_label(), Some("Point"));
    for input in ["MeshOutline Unexpected", "MeshOutline Direction=0,0,1"] {
        assert!(registry.execute(&mut doc, input).is_err());
        assert_eq!(format!("{doc:?}"), before);
    }
}
