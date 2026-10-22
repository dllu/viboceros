use viboceros_command::{
    CommandError, CommandRegistry,
    crease::{CreaseMode, CreaseSelection, CreaseTarget},
};
use viboceros_document::{ControlPointId, Document, Geometry, SelectionMode};
use viboceros_geometry::{MeshFace, Point3, SubD, TriangleMesh};

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn setup(mesh: bool) -> (Document, viboceros_document::ObjectId) {
    let points = vec![
        point(0., 0., 0.),
        point(4., 0., 0.),
        point(4., 3., 0.),
        point(0., 3., 0.),
    ];
    let geometry = if mesh {
        Geometry::Mesh(
            TriangleMesh::try_from_face_records(points, vec![MeshFace::Quad([0, 1, 2, 3])])
                .unwrap(),
        )
    } else {
        Geometry::SubD(SubD::try_from_polygons(points, vec![vec![0, 1, 2, 3]]).unwrap())
    };
    let mut doc = Document::default();
    let id = doc.add_geometry(geometry).unwrap();
    doc.clear_history().unwrap();
    (doc, id)
}

#[test]
fn invalid_component_batches_cannot_partially_admit_geometry() {
    let (mut doc, id) = setup(false);
    let second = doc
        .add_geometry(Geometry::Point(point(9., 0., 0.)))
        .unwrap();
    doc.clear_history().unwrap();
    let before = doc
        .objects()
        .map(|o| (o.id(), o.geometry().clone()))
        .collect::<Vec<_>>();
    assert!(matches!(
        CreaseSelection::prepare(
            &doc,
            CreaseMode::Add,
            [CreaseTarget::Vertex(id, 0), CreaseTarget::Edge(second, 0)]
        ),
        Err(CommandError::CreaseUnavailable)
    ));
    assert!(matches!(
        CreaseSelection::prepare(
            &doc,
            CreaseMode::Add,
            [
                CreaseTarget::Vertex(id, 0),
                CreaseTarget::Vertex(id, usize::MAX)
            ]
        ),
        Err(CommandError::CreaseComponentOutOfRange { .. })
    ));
    assert_eq!(
        doc.objects()
            .map(|o| (o.id(), o.geometry().clone()))
            .collect::<Vec<_>>(),
        before
    );
    assert!(!doc.can_undo());
}

#[test]
fn prepared_crease_rejects_changed_geometry_tolerance_and_permissions() {
    let registry = CommandRegistry::with_builtins();
    for edit in ["geometry", "tolerance", "lock"] {
        let (mut doc, id) = setup(false);
        let prepared =
            CreaseSelection::prepare(&doc, CreaseMode::Add, [CreaseTarget::Vertex(id, 0)]).unwrap();
        match edit {
            "geometry" => {
                doc.replace_object_geometries([(id, Geometry::Point(point(7., 8., 9.)))])
                    .unwrap();
            }
            "tolerance" => {
                registry
                    .execute(&mut doc, "Tolerance Absolute=0.02")
                    .unwrap();
            }
            _ => {
                doc.select_object(id, SelectionMode::Replace).unwrap();
                registry.execute(&mut doc, "Lock").unwrap();
            }
        }
        let geometry = doc.object(id).unwrap().geometry().clone();
        let history = doc.undo_label().map(str::to_owned);
        assert!(
            matches!(prepared.commit(&mut doc), Err(CommandError::CreaseStale)),
            "{edit}"
        );
        assert_eq!(doc.object(id).unwrap().geometry(), &geometry);
        assert_eq!(doc.undo_label(), history.as_deref());
    }
}

#[test]
fn metadata_changes_survive_a_prepared_crease_commit() {
    let (mut doc, id) = setup(false);
    let prepared =
        CreaseSelection::prepare(&doc, CreaseMode::Add, [CreaseTarget::Vertex(id, 0)]).unwrap();
    doc.set_object_names([(id, Some("Renamed".into()))])
        .unwrap();
    let group = doc.add_group(Some("Assembly".into()), [id]).unwrap();
    prepared.commit(&mut doc).unwrap();
    assert_eq!(doc.object(id).unwrap().attributes().name(), Some("Renamed"));
    assert!(doc.object(id).unwrap().group_ids().contains(&group));
}

#[test]
fn unchanged_subd_edits_preserve_redo_while_mesh_edits_consume_a_history_step() {
    for mesh in [false, true] {
        let (mut doc, id) = setup(mesh);
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut doc, "Point 7,8,9").unwrap();
        doc.undo().unwrap();
        assert!(doc.can_redo());
        let original = doc.object(id).unwrap().geometry().clone();
        let prepared =
            CreaseSelection::prepare(&doc, CreaseMode::Add, [CreaseTarget::Edge(id, 0)]).unwrap();
        let outcome = prepared.commit(&mut doc).unwrap();
        assert_eq!(doc.object(id).unwrap().geometry(), &original);
        assert_eq!(outcome.processed_objects.len(), usize::from(mesh));
        if mesh {
            assert_eq!(doc.undo_label(), Some("Crease"));
            assert!(!doc.can_redo());
            doc.undo().unwrap();
            assert_eq!(doc.object(id).unwrap().geometry(), &original);
        } else {
            assert!(!doc.can_undo());
            assert!(doc.can_redo());
        }
    }
}

#[test]
fn bare_commands_apply_to_selected_subd_control_grips_and_keep_them_selected() {
    let (mut doc, id) = setup(false);
    let registry = CommandRegistry::with_builtins();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    registry.execute(&mut doc, "PointsOn").unwrap();
    let grip = ControlPointId {
        object: id,
        index: 0,
    };
    doc.select_control_points([grip], SelectionMode::Replace)
        .unwrap();
    let original = doc.object(id).unwrap().geometry().clone();
    registry.execute(&mut doc, "Crease").unwrap();
    assert_ne!(doc.object(id).unwrap().geometry(), &original);
    assert_eq!(
        doc.selected_control_points()
            .map(|(g, _)| g)
            .collect::<Vec<_>>(),
        vec![grip]
    );
    registry.execute(&mut doc, "RemoveCrease").unwrap();
    let Geometry::SubD(subd) = doc.object(id).unwrap().geometry() else {
        panic!("SubD")
    };
    assert_eq!(subd.vertex_tags()[0], 2);
    assert_eq!(
        doc.selected_control_points()
            .map(|(g, _)| g)
            .collect::<Vec<_>>(),
        vec![grip]
    );
}

#[test]
fn malformed_index_helpers_fail_before_geometry_or_history_changes() {
    let (mut doc, id) = setup(false);
    let registry = CommandRegistry::with_builtins();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    let original = doc.object(id).unwrap().geometry().clone();
    for args in [
        "Edges=",
        "Edges=-1",
        "Edges=0,0",
        "Vertices=0 Vertices=1",
        "Edges=0 Mystery=Yes",
        "Object=not-a-uuid",
        "Edges=999999",
    ] {
        assert!(
            registry
                .execute(&mut doc, &format!("Crease {args}"))
                .is_err(),
            "{args}"
        );
        assert_eq!(doc.object(id).unwrap().geometry(), &original);
        assert!(!doc.can_undo());
    }
}
