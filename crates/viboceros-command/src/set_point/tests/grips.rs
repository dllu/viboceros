use super::*;
use viboceros_document::{ControlPointId, GripMode};
use viboceros_geometry::{Frame3, NurbsCurve, Vector3, WeightedPoint3};

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn source() -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        [p(0., 0., 1.), p(1., 3., 2.), p(3., -2., 3.), p(4., 0., 4.)]
            .into_iter()
            .zip([1., 0.5, 2., 1.])
            .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
            .collect(),
        vec![2., 2., 2., 3., 7., 7., 7.],
    )
    .unwrap()
}
fn setup(mode: GripMode) -> (Document, ObjectId, ControlPointId) {
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::NurbsCurve(source())).unwrap();
    match mode {
        GripMode::Control => doc.enable_control_points([id]),
        GripMode::Edit => doc.enable_edit_points([id]),
    }
    .unwrap();
    let grip = ControlPointId {
        object: id,
        index: 1,
    };
    doc.select_control_points([grip], SelectionMode::Add)
        .unwrap();
    (doc, id, grip)
}

#[test]
fn set_point_grips_preserve_original_basis_and_unselected_locations_through_history() {
    let registry = CommandRegistry::with_builtins();
    for mode in [GripMode::Control, GripMode::Edit] {
        let (mut doc, id, grip) = setup(mode);
        let original = doc.object(id).unwrap().geometry().clone();
        let before = doc.control_point_locations(id).unwrap().to_vec();
        // The parent is also selected; selected grips still define the edit.
        doc.select_objects_direct([id], SelectionMode::Add).unwrap();
        registry
            .execute(&mut doc, "SetPt 0,0,9 XSet=No YSet=No ZSet=Yes")
            .unwrap();
        assert_eq!(doc.undo_label(), Some("SetPt"));
        let Geometry::NurbsCurve(result) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        assert_eq!(result.knots(), source().knots());
        assert_eq!(result.degree(), source().degree());
        assert_eq!(
            result
                .control_points()
                .iter()
                .map(|c| c.weight())
                .collect::<Vec<_>>(),
            vec![1., 0.5, 2., 1.]
        );
        let expected = before
            .iter()
            .enumerate()
            .map(|(i, q)| if i == 1 { p(q.x(), q.y(), 9.) } else { *q })
            .collect::<Vec<_>>();
        for (a, b) in doc
            .control_point_locations(id)
            .unwrap()
            .iter()
            .zip(expected)
        {
            assert!(a.distance_to(b).unwrap() < 1e-8);
        }
        let changed = doc.object(id).unwrap().geometry().clone();
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.object(id).unwrap().geometry(), &original);
        assert_eq!(doc.grip_mode(id), Some(mode));
        assert_eq!(
            doc.selected_control_points()
                .map(|(p, _)| p)
                .collect::<Vec<_>>(),
            [grip]
        );
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.object(id).unwrap().geometry(), &changed);
    }
}

#[test]
fn copied_cplane_grips_and_ordinary_points_use_the_same_coordinate_constraint() {
    let registry = CommandRegistry::with_builtins();
    for mode in [GripMode::Control, GripMode::Edit] {
        let (mut doc, id, _) = setup(mode);
        let original = doc.object(id).unwrap().geometry().clone();
        let before = doc.control_point_locations(id).unwrap().to_vec();
        let peer = doc.add_geometry(Geometry::Point(p(6., 7., 8.))).unwrap();
        doc.select_objects_direct([peer], SelectionMode::Add)
            .unwrap();
        let frame = Frame3::try_from_directions(
            p(20., 30., 40.),
            Vector3::try_new(0., 1., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            doc.tolerance(),
        )
        .unwrap();
        registry
            .execute_in_context(
                &mut doc,
                "SetPt 10,11,12 XSet=Yes YSet=No ZSet=No Alignment=CPlane Copy=Yes",
                CommandContext {
                    construction_plane: frame,
                },
            )
            .unwrap();
        assert_eq!(doc.objects().count(), 4);
        assert_eq!(doc.object(id).unwrap().geometry(), &original);
        let copy = doc
            .objects()
            .find(|o| o.id() != id && matches!(o.geometry(), Geometry::NurbsCurve(_)))
            .unwrap()
            .id();
        assert_eq!(doc.grip_mode(copy), Some(mode));
        for (i, q) in doc
            .control_point_locations(copy)
            .unwrap()
            .iter()
            .enumerate()
        {
            let expected = if i == 1 {
                p(before[i].x(), 11., before[i].z())
            } else {
                before[i]
            };
            assert!(q.distance_to(expected).unwrap() < 1e-8);
        }
        assert!(
            doc.objects()
                .any(|o| o.id() != peer && o.geometry() == &Geometry::Point(p(6., 11., 8.)))
        );
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().count(), 2);
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.grip_mode(copy), Some(mode));
    }
}

#[test]
fn invalid_frozen_grips_and_geometry_failures_preserve_peers_selection_and_redo() {
    let registry = CommandRegistry::with_builtins();
    let (mut doc, id, _) = setup(GripMode::Control);
    registry.execute(&mut doc, "Line 10,0,0 11,0,0").unwrap();
    let line = doc
        .objects()
        .find(|o| matches!(o.geometry(), Geometry::Line(_)))
        .unwrap()
        .id();
    registry.execute(&mut doc, "Point 9,9,9").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    doc.select_objects_direct([line], SelectionMode::Add)
        .unwrap();
    let before = format!("{doc:?}");
    // A collapsed whole line fails even though the grip edit itself is valid.
    assert!(registry.execute(&mut doc, "SetPt 0,0,0").is_err());
    assert_eq!(format!("{doc:?}"), before);
    assert!(
        registry
            .execute(
                &mut doc,
                &format!("SetPt 0,0,0 Sources={line} GripSources={id}:99")
            )
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
    assert!(doc.can_redo());
}
