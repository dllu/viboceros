use viboceros_command::{CommandError, CommandRegistry, soft_crease::SoftCreaseSession};
use viboceros_document::{ControlPointId, Document, Geometry, ObjectId, SelectionMode};
use viboceros_geometry::{Point3, SubD};

fn setup() -> (Document, ObjectId, Vec<usize>) {
    let subd = SubD::try_from_polygons(
        [
            [0., 0., 0.],
            [4., 0., 0.],
            [4., 3., 0.],
            [0., 3., 0.],
            [0., 0., 2.],
            [4., 0., 2.],
            [4., 3., 2.],
            [0., 3., 2.],
        ]
        .map(|p| Point3::try_from(p).unwrap())
        .to_vec(),
        vec![
            vec![0, 3, 2, 1],
            vec![0, 1, 5, 4],
            vec![1, 2, 6, 5],
            vec![2, 3, 7, 6],
            vec![3, 0, 4, 7],
            vec![4, 5, 6, 7],
        ],
    )
    .unwrap();
    let edges = [[0, 1], [1, 2], [2, 3]]
        .map(|[a, b]| {
            subd.edges()
                .iter()
                .position(|e| *e == [a, b] || *e == [b, a])
                .unwrap()
        })
        .to_vec();
    let mut document = Document::default();
    let id = document.add_geometry(Geometry::SubD(subd)).unwrap();
    document.clear_history().unwrap();
    (document, id, edges)
}

#[test]
fn preview_is_immutable_invalid_actions_are_atomic_and_labels_follow_the_limit_surface() {
    let (document, id, edges) = setup();
    let source = document.object(id).unwrap().geometry().clone();
    let mut editor = SoftCreaseSession::prepare(&document, edges.iter().map(|&e| (id, e))).unwrap();
    let before = editor.labels();
    editor.vertex(0, 0, 80.).unwrap();
    assert_eq!(document.object(id).unwrap().geometry(), &source);
    let candidate = editor.candidates().next().unwrap().1.clone();
    assert!(
        editor
            .labels()
            .iter()
            .any(|l| before.iter().find(|b| b.vertex == l.vertex).unwrap().point != l.point)
    );
    for label in editor.labels() {
        assert_eq!(label.point, candidate.surface_points()[label.vertex]);
    }
    for (chain, vertex, weight) in [
        (usize::MAX, 0, 50.),
        (0, usize::MAX, 50.),
        (0, 0, -1.),
        (0, 0, 101.),
        (0, 0, f64::NAN),
    ] {
        assert!(editor.vertex(chain, vertex, weight).is_err());
        assert_eq!(editor.candidates().next().unwrap().1, &candidate);
    }
    assert!(editor.constant(None, f64::INFINITY).is_err());
    assert_eq!(editor.candidates().next().unwrap().1, &candidate);
    assert!(!document.can_undo());
}

#[test]
fn commit_releases_grips_before_undo_and_preserves_metadata_and_source_order() {
    let (mut document, id, edges) = setup();
    let registry = CommandRegistry::with_builtins();
    let second = document
        .add_geometry(document.object(id).unwrap().geometry().clone())
        .unwrap();
    registry.execute(&mut document, "Point 99,0,0").unwrap();
    let witness = document.objects().last().unwrap().id();
    document.enable_control_points([id]).unwrap();
    document
        .select_control_points(
            [ControlPointId {
                object: id,
                index: 0,
            }],
            SelectionMode::Replace,
        )
        .unwrap();
    let original = document.object(id).unwrap().geometry().clone();
    let mut editor = SoftCreaseSession::prepare(&document, edges.iter().map(|&e| (id, e))).unwrap();
    document
        .set_object_names([(id, Some("Renamed".into()))])
        .unwrap();
    let group = document.add_group(Some("Assembly".into()), [id]).unwrap();
    document.clear_history().unwrap();
    editor.vertex(0, 0, 80.).unwrap();
    assert_eq!(editor.commit(&mut document).unwrap(), vec![id]);
    assert_eq!(
        document.objects().map(|o| o.id()).collect::<Vec<_>>(),
        vec![second, witness, id]
    );
    assert_eq!(
        document.object(id).unwrap().attributes().name(),
        Some("Renamed")
    );
    assert!(document.object(id).unwrap().group_ids().contains(&group));
    assert!(document.grip_mode(id).is_some());
    assert_eq!(document.selected_control_points().count(), 0);
    assert_eq!(document.undo_label(), Some("SubDCrease"));
    document.undo().unwrap();
    assert_eq!(document.object(id).unwrap().geometry(), &original);
    assert_eq!(
        document.objects().map(|o| o.id()).collect::<Vec<_>>(),
        vec![id, second, witness]
    );
    assert!(document.grip_mode(id).is_some());
    assert_eq!(document.selected_control_points().count(), 0);
    document.redo().unwrap();
    assert_eq!(document.selected_control_points().count(), 0);
}

#[test]
fn no_op_commit_preserves_redo_and_stale_sources_cannot_be_admitted() {
    let registry = CommandRegistry::with_builtins();
    for change in ["none", "geometry", "tolerance", "lock"] {
        let (mut document, id, edges) = setup();
        registry.execute(&mut document, "Point 9,8,7").unwrap();
        document.undo().unwrap();
        let mut editor =
            SoftCreaseSession::prepare(&document, edges.iter().map(|&e| (id, e))).unwrap();
        editor.constant(None, 0.).unwrap();
        match change {
            "geometry" => {
                document
                    .replace_object_geometries([(
                        id,
                        Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()),
                    )])
                    .unwrap();
            }
            "tolerance" => {
                registry
                    .execute(&mut document, "Tolerance Absolute=0.02")
                    .unwrap();
            }
            "lock" => {
                document.select_object(id, SelectionMode::Replace).unwrap();
                registry.execute(&mut document, "Lock").unwrap();
            }
            _ => {}
        }
        let before = document.object(id).unwrap().geometry().clone();
        let history = document.undo_label().map(str::to_owned);
        if change == "none" {
            assert!(editor.commit(&mut document).unwrap().is_empty());
            assert!(document.can_redo());
            assert!(!document.can_undo());
        } else {
            assert!(matches!(
                editor.commit(&mut document),
                Err(CommandError::CreaseStale)
            ));
        }
        assert_eq!(document.object(id).unwrap().geometry(), &before);
        assert_eq!(document.undo_label(), history.as_deref());
    }
}

#[test]
fn typed_command_uses_the_same_profile_and_admits_only_a_complete_edit() {
    let registry = CommandRegistry::with_builtins();
    let (mut document, id, edges) = setup();
    let mut expected =
        SoftCreaseSession::prepare(&document, edges.iter().map(|&e| (id, e))).unwrap();
    expected.vertex(0, 0, 80.).unwrap();
    expected.vertex(0, 1, 20.).unwrap();
    let indices = edges
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",");
    document.select_object(id, SelectionMode::Replace).unwrap();
    registry
        .execute(
            &mut document,
            &format!("_SubDCrease Edges={indices} Vertex=0 Weight=80 Vertex=1 Weight=20"),
        )
        .unwrap();
    let Geometry::SubD(actual) = document.object(id).unwrap().geometry() else {
        panic!()
    };
    // Independently serialized native objects may have different archive IDs.
    let expected = expected.candidates().next().unwrap().1;
    assert_eq!(actual.edge_sharpness(), expected.edge_sharpness());
    assert_eq!(actual.edge_tags(), expected.edge_tags());
    assert_eq!(actual.vertex_tags(), expected.vertex_tags());
    assert_eq!(actual.surface_points(), expected.surface_points());
    assert_eq!(
        actual.tessellate(2).unwrap(),
        expected.tessellate(2).unwrap()
    );
    document.undo().unwrap();
    let original = document.object(id).unwrap().geometry().clone();
    document.select_object(id, SelectionMode::Replace).unwrap();
    assert!(
        registry
            .execute(
                &mut document,
                &format!("SubDCrease Edges={indices} Weight=50 Vertex=0 Weight=101")
            )
            .is_err()
    );
    assert_eq!(document.object(id).unwrap().geometry(), &original);
    assert!(document.can_redo());
    registry
        .execute(
            &mut document,
            &format!("SubDCrease Edges={indices} Weight=50 Cancel"),
        )
        .unwrap();
    assert_eq!(document.object(id).unwrap().geometry(), &original);
    assert!(document.can_redo());
}

#[test]
fn boundary_only_empty_and_mixed_invalid_targets_do_not_create_sessions() {
    let (document, id, edges) = setup();
    assert!(SoftCreaseSession::prepare(&document, []).is_err());
    assert!(SoftCreaseSession::prepare(&document, [(id, edges[0]), (id, usize::MAX)]).is_err());
    let mut document = document;
    let plane = SubD::try_from_polygons(
        [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![vec![0, 1, 2, 3]],
    )
    .unwrap();
    let boundary = document.add_geometry(Geometry::SubD(plane)).unwrap();
    assert!(SoftCreaseSession::prepare(&document, [(boundary, 0)]).is_err());
}
