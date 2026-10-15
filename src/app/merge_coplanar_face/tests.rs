use super::*;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn source(app: &mut VibocerosApp, brep: bool) -> ObjectId {
    let t = app.document.tolerance();
    let mesh = viboceros_geometry::TriangleMesh::try_plane_grid(
        viboceros_geometry::Frame3::try_from_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap(),
            t,
        )
        .unwrap(),
        [0., 3.],
        [0., 1.],
        3,
        1,
        t,
    )
    .unwrap();
    app.document
        .add_geometry(if brep {
            Geometry::Brep(viboceros_geometry::Brep::try_from_mesh(&mesh, true, t).unwrap())
        } else {
            Geometry::Mesh(mesh)
        })
        .unwrap()
}
#[test]
fn selected_mesh_face_getter_retries_then_commits_one_history_step() {
    let mut app = super::super::tests::test_app();
    let id = source(&mut app, false);
    let original = app.document.object(id).unwrap().clone();
    enter(&mut app, "MergeCoplanarFace");
    assert!(!app.command_line_idle());
    app.accept_component_face_hit(id, 0, None);
    assert_eq!(app.document.object(id), Some(&original));
    assert_eq!(
        app.merge_face_highlight().unwrap().kind,
        viboceros_command::ComponentSelectionKind::MeshFace
    );
    let before = format!("{:?}", app.document);
    app.accept_component_face_hit(id, 2, None);
    assert!(app.merge_face_prompt.is_some());
    assert_eq!(format!("{:?}", app.document), before);
    app.accept_component_face_hit(id, 1, None);
    assert!(app.merge_face_prompt.is_none());
    assert!(app.command_line_idle());
    let after = app.document.object(id).unwrap().clone();
    assert!(
        matches!(after.geometry(),Geometry::Mesh(m) if m.ngons().len()==1 && m.ngons()[0].faces().len()==2)
    );
    assert_eq!(app.document.undo_label(), Some("MergeCoplanarFace"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.object(id), Some(&original));
    enter(&mut app, "Redo");
    assert_eq!(app.document.object(id), Some(&after));
}
#[test]
fn selected_face_getter_option_action_typed_indices_and_cancel() {
    let mut app = super::super::tests::test_app();
    let id = source(&mut app, false);
    let original = app.document.object(id).unwrap().clone();
    enter(&mut app, "MergeCoplanarFace");
    enter(&mut app, &format!("Object={id} Face=0"));
    enter(&mut app, "MergeAllCoplanar=Maybe");
    assert!(!app.merge_face_prompt.as_ref().unwrap().region);
    assert_eq!(app.document.object(id), Some(&original));
    enter(&mut app, "Cancel");
    assert!(app.merge_face_prompt.is_none());
    assert_eq!(app.document.object(id), Some(&original));
    enter(&mut app, "MergeCoplanarFace");
    enter(&mut app, &format!("Object={id} Face=1"));
    enter(&mut app, "MergeAllCoplanar");
    assert!(app.merge_face_prompt.is_none());
    assert!(
        matches!(app.document.object(id).unwrap().geometry(),Geometry::Mesh(m) if m.ngons()[0].faces().len()==3)
    );
    enter(&mut app, "Undo");
    enter(&mut app, "MergeCoplanarFace MergeAllCoplanar=Yes");
    app.accept_component_face_hit(id, 0, None);
    assert!(app.merge_face_prompt.is_none());
}
#[test]
fn selected_face_getter_rejects_stale_tolerance_and_other_objects() {
    let mut app = super::super::tests::test_app();
    let id = source(&mut app, false);
    let other = source(&mut app, false);
    enter(&mut app, "MergeCoplanarFace");
    app.accept_component_face_hit(id, 0, None);
    let before = format!("{:?}", app.document);
    app.accept_component_face_hit(other, 1, None);
    assert_eq!(format!("{:?}", app.document), before);
    app.document
        .set_tolerance(Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap());
    let before = format!("{:?}", app.document);
    enter(&mut app, "1");
    assert!(app.merge_face_prompt.is_some());
    assert_eq!(format!("{:?}", app.document), before);
    app.cancel_current_prompt_or_selection();
    assert!(app.merge_face_prompt.is_none());
    assert!(app.command_line_idle());
}
#[cfg(feature = "native-smlib")]
#[test]
fn selected_brep_face_clicks_use_original_source_indices() {
    let mut app = super::super::tests::test_app();
    let id = source(&mut app, true);
    enter(&mut app, "MergeCoplanarFace");
    app.accept_component_face_hit(id, 0, None);
    app.accept_component_face_hit(id, 1, None);
    assert!(app.merge_face_prompt.is_none());
    assert!(
        matches!(app.document.object(id).unwrap().geometry(),Geometry::Brep(b) if b.faces().len()==2)
    );
}

#[test]
fn selected_ngon_outline_caches_hole_loops_and_disappears_when_source_changes() {
    use viboceros_geometry::{MeshFace, TriangleMesh};
    let mut app = super::super::tests::test_app();
    let t = app.document.tolerance();
    let points = [
        [0., 0., 0.],
        [3., 0., 0.],
        [3., 3., 0.],
        [0., 3., 0.],
        [1., 1., 0.],
        [2., 1., 0.],
        [2., 2., 0.],
        [1., 2., 0.],
    ]
    .into_iter()
    .map(|p| Point3::try_from(p).unwrap())
    .collect();
    let faces = [[0, 1, 5, 4], [1, 2, 6, 5], [2, 3, 7, 6], [3, 0, 4, 7]]
        .into_iter()
        .map(MeshFace::Quad)
        .collect();
    let mesh = TriangleMesh::try_new_faces(points, faces, t).unwrap();
    let (ring, removed) = mesh.merge_coplanar_faces(t).unwrap();
    assert_eq!(removed, 3);
    let id = app
        .document
        .add_geometry(Geometry::Mesh(ring.clone()))
        .unwrap();
    enter(&mut app, "MergeCoplanarFace");
    app.accept_component_face_hit(id, 2, None);
    let wires = app.merge_face_mesh_wires();
    assert_eq!(wires.len(), 8);
    let inner = wires
        .iter()
        .filter(|segment| {
            segment
                .iter()
                .all(|p| p.x() >= 1. && p.x() <= 2. && p.y() >= 1. && p.y() <= 2.)
        })
        .count();
    assert_eq!(inner, 4);
    // Replacing with a different snapshot hides the cached outline immediately.
    app.document
        .replace_object_geometries(vec![(id, Geometry::Mesh(mesh))])
        .unwrap();
    assert!(app.merge_face_mesh_wires().is_empty());
    let before = format!("{:?}", app.document);
    app.accept_component_face_hit(id, 1, None);
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.merge_face_prompt.is_some());
}
