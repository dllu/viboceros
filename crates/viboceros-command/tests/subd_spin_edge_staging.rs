use viboceros_command::{
    CommandError, CommandRegistry,
    subd_spin_edge::{Direction, SpinSession, Target},
};
use viboceros_document::{ControlPointId, Document, Geometry, GripMode, SelectionMode};
use viboceros_geometry::{Point3, SubD};
fn setup() -> (Document, Target) {
    let points = [
        [0., 0., 0.],
        [2., 0., 0.],
        [4., 0., 0.],
        [0., 3., 0.],
        [2., 3., 0.],
        [4., 3., 0.],
    ]
    .into_iter()
    .map(|p| Point3::try_from(p).unwrap())
    .collect();
    let s = SubD::try_from_polygons(points, vec![vec![0, 1, 4, 3], vec![1, 2, 5, 4]]).unwrap();
    let edge = s
        .edges()
        .iter()
        .position(|&e| e == [1, 4] || e == [4, 1])
        .unwrap();
    let mut doc = Document::default();
    let object = doc.add_geometry(Geometry::SubD(s)).unwrap();
    doc.clear_history().unwrap();
    (doc, Target { object, edge })
}
#[test]
fn stale_invalid_and_locked_inputs_never_admit_a_partial_session() {
    for change in ["geometry", "tolerance", "lock", "index", "type"] {
        let (mut doc, target) = setup();
        let mut session = SpinSession::new(&doc, Direction::default());
        session.spin(&doc, target).unwrap();
        match change {
            "geometry" => {
                doc.replace_object_geometries([(
                    target.object,
                    Geometry::Point(Point3::try_new(8., 9., 10.).unwrap()),
                )])
                .unwrap();
            }
            "tolerance" => {
                CommandRegistry::with_builtins()
                    .execute(&mut doc, "Tolerance Absolute=0.02")
                    .unwrap();
            }
            "lock" => {
                doc.select_objects_direct([target.object], SelectionMode::Replace)
                    .unwrap();
                CommandRegistry::with_builtins()
                    .execute(&mut doc, "Lock")
                    .unwrap();
            }
            "index" => {
                assert!(matches!(
                    session.spin(
                        &doc,
                        Target {
                            edge: usize::MAX,
                            ..target
                        }
                    ),
                    Err(CommandError::SpinEdgeOutOfRange { .. })
                ));
            }
            _ => {
                let id = doc
                    .add_geometry(Geometry::Point(Point3::try_new(5., 6., 7.).unwrap()))
                    .unwrap();
                assert!(matches!(
                    session.spin(
                        &doc,
                        Target {
                            object: id,
                            edge: 0
                        }
                    ),
                    Err(CommandError::SpinEdgeUnavailable)
                ));
            }
        }
        let geometry = doc.object(target.object).unwrap().geometry().clone();
        let label = doc.undo_label().map(str::to_owned);
        if matches!(change, "geometry" | "tolerance" | "lock") {
            assert!(matches!(
                session.finish(&mut doc),
                Err(CommandError::SpinEdgeStale)
            ));
        }
        assert_eq!(doc.object(target.object).unwrap().geometry(), &geometry);
        assert_eq!(doc.undo_label(), label.as_deref());
    }
}
#[test]
fn scripted_points_indices_direction_toggles_repeat_and_undo_are_ordered() {
    let registry = CommandRegistry::with_builtins();
    for method in ["point", "index"] {
        let (mut doc, target) = setup();
        doc.select_objects_direct([target.object], SelectionMode::Replace)
            .unwrap();
        let original = doc.object(target.object).unwrap().geometry().clone();
        let command = if method == "point" {
            "SubDSpinEdge Direction=Clockwise 2,1.5,0 SpinEdgeAgain Direction SpinEdgeAgain Undo"
                .into()
        } else {
            format!(
                "SubDSpinEdge Direction=Clockwise Edge={} SpinEdgeAgain Direction SpinEdgeAgain Undo",
                target.edge
            )
        };
        let mut expected = SpinSession::new(&doc, Direction::Clockwise);
        expected.spin(&doc, target).unwrap();
        expected.spin_again(&doc).unwrap();
        expected.direction = Direction::CounterClockwise;
        expected.spin_again(&doc).unwrap();
        assert!(expected.undo_spin());
        let output = expected.candidates().next().unwrap().1.clone();
        registry.execute(&mut doc, &command).unwrap();
        let Geometry::SubD(actual) = doc.object(target.object).unwrap().geometry() else {
            panic!()
        };
        assert_eq!(actual.control_points(), output.control_points());
        assert_eq!(actual.edges(), output.edges());
        assert_eq!(actual.faces(), output.faces());
        assert_eq!(actual.vertex_tags(), output.vertex_tags());
        assert_eq!(actual.edge_tags(), output.edge_tags());
        assert_eq!(actual.surface_points(), output.surface_points());
        assert_eq!(doc.undo_label(), Some("SubDSpinEdge"));
        doc.undo().unwrap();
        assert_eq!(doc.object(target.object).unwrap().geometry(), &original);
    }
}
#[test]
fn command_local_undo_keeps_one_history_entry_and_control_identity() {
    let (mut doc, target) = setup();
    doc.enable_control_points([target.object]).unwrap();
    let grip = ControlPointId {
        object: target.object,
        index: 0,
    };
    doc.select_control_points([grip], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    let original = doc.object(target.object).unwrap().geometry().clone();
    let mut session = SpinSession::new(&doc, Direction::default());
    session.spin(&doc, target).unwrap();
    assert!(session.undo_spin());
    assert!(session.last_target().is_none());
    session.finish(&mut doc).unwrap();
    assert_eq!(doc.object(target.object).unwrap().geometry(), &original);
    assert_eq!(doc.undo_label(), Some("SubDSpinEdge"));
    assert_eq!(doc.grip_mode(target.object), Some(GripMode::Control));
    assert_eq!(doc.selected_logical_control_points().count(), 0);
    doc.undo().unwrap();
    doc.redo().unwrap();
    assert_eq!(doc.grip_mode(target.object), Some(GripMode::Control));
}
