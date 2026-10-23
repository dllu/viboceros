use viboceros_command::{CommandError, CommandRegistry, insert_point::InsertSession};
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::{MeshFace, Point3, PolygonEdgePoint, TriangleMesh};
fn setup() -> (Document, viboceros_document::ObjectId) {
    let mesh = TriangleMesh::try_from_face_records(
        [[0., 0., 0.], [4., 0., 0.], [4., 3., 0.], [0., 3., 0.]]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        vec![MeshFace::Quad([0, 1, 2, 3])],
    )
    .unwrap();
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::Mesh(mesh)).unwrap();
    doc.clear_history().unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    (doc, id)
}
#[test]
fn malformed_late_tokens_leave_source_selection_history_and_redo_unchanged() {
    for suffix in [
        "Edge=9999 Parameter=0.5",
        "Edge=0 Parameter=NaN",
        "Enter Undo",
        "Cancel 1,2,3",
        "unknown",
    ] {
        let (mut doc, id) = setup();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut doc, "Point 10,10,10").unwrap();
        doc.undo().unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let original = doc.object(id).unwrap().geometry().clone();
        let redo = doc.redo_label().map(str::to_owned);
        let history = doc.undo_label().map(str::to_owned);
        assert!(
            registry
                .execute(&mut doc, &format!("InsertPoint 0,1.5,0 {suffix}"))
                .is_err()
        );
        assert_eq!(doc.object(id).unwrap().geometry(), &original);
        assert!(doc.is_selected(id));
        assert_eq!(doc.redo_label(), redo.as_deref());
        assert_eq!(doc.undo_label(), history.as_deref());
    }
}
#[test]
fn stale_geometry_tolerance_and_lock_never_admit_preview_geometry() {
    for edit in ["geometry", "tolerance", "lock"] {
        let (mut doc, id) = setup();
        let mut session = InsertSession::new(&doc, id).unwrap();
        let pick = session
            .pick_point(Point3::try_new(0., 1.5, 0.).unwrap())
            .unwrap();
        session.insert(&doc, pick, None).unwrap();
        match edit {
            "geometry" => {
                doc.replace_object_geometries([(
                    id,
                    Geometry::Point(Point3::try_new(8., 9., 10.).unwrap()),
                )])
                .unwrap();
            }
            "tolerance" => {
                CommandRegistry::with_builtins()
                    .execute(&mut doc, "Tolerance Absolute=0.1")
                    .unwrap();
            }
            _ => {
                CommandRegistry::with_builtins()
                    .execute(&mut doc, "Lock")
                    .unwrap();
            }
        }
        let original = doc.object(id).unwrap().geometry().clone();
        let label = doc.undo_label().map(str::to_owned);
        assert!(matches!(
            session.finish(&mut doc),
            Err(CommandError::InsertPointStale)
        ));
        assert_eq!(doc.object(id).unwrap().geometry(), &original);
        assert_eq!(doc.undo_label(), label.as_deref());
    }
}
#[test]
fn tiny_mesh_features_and_both_parameter_syntaxes_survive_model_tolerance() {
    for syntax in ["Edge=1 Parameter=0.000001", "Edge 1 Parameter 0.000001"] {
        let (mut doc, id) = setup();
        let original = doc.object(id).unwrap().geometry().clone();
        CommandRegistry::with_builtins()
            .execute(&mut doc, &format!("InsertPoint {syntax} Enter"))
            .unwrap();
        let Geometry::Mesh(m) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        assert_eq!(m.vertices().len(), 5);
        assert_eq!(m.face_count(), 2);
        doc.undo().unwrap();
        assert_eq!(doc.object(id).unwrap().geometry(), &original);
    }
}
#[test]
fn existing_mesh_edges_below_document_tolerance_remain_pickable() {
    let (mut doc, id) = setup();
    let scale = 1e-5;
    let mesh = TriangleMesh::try_from_face_records(
        [[0., 0., 0.], [4., 0., 0.], [4., 3., 0.], [0., 3., 0.]]
            .into_iter()
            .map(|p| Point3::try_from(p.map(|v| v * scale)).unwrap())
            .collect(),
        vec![MeshFace::Quad([0, 1, 2, 3])],
    )
    .unwrap();
    doc.replace_object_geometries([(id, Geometry::Mesh(mesh))])
        .unwrap();
    CommandRegistry::with_builtins()
        .execute(
            &mut doc,
            "InsertPoint 0,0.000015,0 0.00004,0.000015,0 Enter",
        )
        .unwrap();
    let Geometry::Mesh(mesh) = doc.object(id).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(mesh.vertices().len(), 6);
    assert_eq!(mesh.face_count(), 2);
    assert!(mesh.faces().iter().all(|f| matches!(f, MeshFace::Quad(_))));
}
#[test]
fn insertion_on_one_island_does_not_remesh_an_untouched_concave_quad() {
    use viboceros_geometry::{MeshNgon, Tolerance};
    let (mut doc, id) = setup();
    let mesh = TriangleMesh::try_new_faces(
        [
            [0., 0., 0.],
            [4., 0., 0.],
            [4., 3., 0.],
            [0., 3., 0.],
            [10., 0., 0.],
            [12., 0., 0.],
            [10.5, 0.5, 0.],
            [10., 2., 0.],
        ]
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect(),
        vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Quad([4, 5, 6, 7])],
        Tolerance::MESH_VALIDATION,
    )
    .unwrap()
    .try_with_ngons(vec![MeshNgon::from_parts(vec![4, 5, 6, 7], vec![1])])
    .unwrap();
    doc.replace_object_geometries([(id, Geometry::Mesh(mesh))])
        .unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut doc, "InsertPoint 0,1.5,0 Enter")
        .unwrap();
    let Geometry::Mesh(mesh) = doc.object(id).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(mesh.face_count(), 3);
    assert!(mesh.faces().contains(&MeshFace::Quad([4, 5, 6, 7])));
    let retained = mesh
        .ngons()
        .iter()
        .find(|n| n.vertices() == [4, 5, 6, 7])
        .unwrap();
    assert_eq!(retained.faces().len(), 1);
}
#[test]
fn invalid_session_picks_and_exhausted_point_budget_keep_the_last_valid_preview() {
    let (doc, id) = setup();
    let mut session = InsertSession::new(&doc, id).unwrap();
    let pick = PolygonEdgePoint {
        edge: 1,
        parameter: 0.5,
    };
    session.insert(&doc, pick, None).unwrap();
    let candidate = session.candidate().clone();
    assert!(
        session
            .insert(
                &doc,
                PolygonEdgePoint {
                    edge: usize::MAX,
                    ..pick
                },
                None
            )
            .is_err()
    );
    assert_eq!(session.candidate(), &candidate);
    for _ in 1..256 {
        session.insert(&doc, pick, None).unwrap();
    }
    assert!(matches!(
        session.insert(&doc, pick, None),
        Err(CommandError::InsertPointResourceLimit)
    ));
    assert_eq!(session.candidate(), &candidate);
    assert_eq!(session.point_count(), 256);
}
