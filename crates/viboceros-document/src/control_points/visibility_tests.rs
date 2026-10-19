use super::*;
fn source() -> (Document, ObjectId, ObjectId, ObjectId) {
    let mut doc = Document::default();
    let curve = viboceros_geometry::NurbsCurve::try_clamped_uniform(
        3,
        (0..4)
            .map(|i| Point3::try_new(i as f64, (i % 2) as f64, 0.).unwrap())
            .collect(),
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
    let peer = doc
        .add_geometry(Geometry::Point(Point3::try_new(8., 0., 0.).unwrap()))
        .unwrap();
    doc.clear_history().unwrap();
    let marker = doc
        .add_geometry(Geometry::Point(Point3::try_new(30., 0., 0.).unwrap()))
        .unwrap();
    doc.undo().unwrap();
    doc.enable_control_points([id]).unwrap();
    doc.select_objects_direct([id, peer], SelectionMode::Add)
        .unwrap();
    (doc, id, peer, marker)
}
fn grip(object: ObjectId, index: usize) -> ControlPointId {
    ControlPointId { object, index }
}
#[test]
fn hiding_keeps_logical_indices_geometry_and_redo_and_show_keeps_other_picks() {
    let (mut doc, id, peer, marker) = source();
    let original = doc.object(id).unwrap().clone();
    let points = doc.control_point_locations(id).unwrap().to_vec();
    doc.select_control_points([grip(id, 0), grip(id, 2)], SelectionMode::Add)
        .unwrap();
    assert_eq!(doc.hide_selected_control_points(), 2);
    assert_eq!(doc.all_control_points().count(), 4);
    assert_eq!(
        doc.control_points()
            .map(|(g, _, _)| g.index)
            .collect::<Vec<_>>(),
        [1, 3]
    );
    assert_eq!(doc.selected_control_points().count(), 0);
    assert_eq!(doc.control_point_locations(id), Some(points.as_slice()));
    assert_eq!(doc.is_control_point_hidden(grip(id, 0)), Some(true));
    assert!(!doc.is_control_point_selectable(grip(id, 0)));
    doc.select_control_points([grip(id, 1)], SelectionMode::Add)
        .unwrap();
    assert_eq!(doc.show_control_points(), 2);
    assert_eq!(
        doc.selected_control_points()
            .map(|(g, _)| g.index)
            .collect::<Vec<_>>(),
        [1]
    );
    assert!(doc.is_selected(id) && doc.is_selected(peer));
    assert_eq!(doc.object(id), Some(&original));
    assert!(!doc.can_undo());
    assert!(doc.can_redo());
    doc.redo().unwrap();
    assert!(doc.object(marker).is_some());
}
#[test]
fn invalid_visibility_selection_and_edit_batches_are_atomic() {
    let (mut doc, id, peer, _) = source();
    let original = doc.object(id).unwrap().clone();
    assert!(
        doc.set_control_points_hidden([grip(id, 0), grip(id, 4)], true)
            .is_err()
    );
    assert_eq!(doc.control_points().count(), 4);
    doc.set_control_points_hidden([grip(id, 0)], true).unwrap();
    doc.select_control_points([grip(id, 2)], SelectionMode::Add)
        .unwrap();
    let transform = viboceros_geometry::AffineTransform3::from_translation(
        viboceros_geometry::Vector3::try_new(1., 2., 3.).unwrap(),
    );
    assert!(
        doc.select_control_points([grip(id, 4), grip(id, 1)], SelectionMode::Replace)
            .is_err()
    );
    assert_eq!(
        doc.selected_control_points()
            .map(|(g, _)| g.index)
            .collect::<Vec<_>>(),
        [2]
    );
    assert!(doc.is_selected(peer));
    assert!(
        doc.transform_control_point_display([grip(id, 1), grip(id, 4)], transform)
            .is_err()
    );
    assert!(
        doc.transform_objects_and_grips(
            [],
            [grip(id, 1), grip(id, 4)],
            transform,
            false,
            CopyGroupPolicy::Omit
        )
        .is_err()
    );
    assert_eq!(doc.object(id), Some(&original));
    assert!(doc.can_redo());
    doc.select_control_points([grip(id, 0)], SelectionMode::Add)
        .unwrap();
    assert!(
        doc.selected_logical_control_points()
            .any(|(g, _)| g == grip(id, 0))
    );
    assert!(!doc.selected_control_points().any(|(g, _)| g == grip(id, 0)));
    assert!(!doc.is_control_point_selectable(grip(id, 0)));
    doc.disable_control_points();
    assert!(doc.is_control_point_hidden(grip(id, 0)).is_none());
    doc.enable_control_points([id]).unwrap();
    assert_eq!(doc.control_points().count(), 4);
}
#[test]
fn visible_grip_edits_and_history_preserve_hidden_layout_without_moving_hidden_controls() {
    let (mut doc, id, _, _) = source();
    doc.clear_selection();
    doc.clear_history().unwrap();
    doc.set_control_points_hidden([grip(id, 0), grip(id, 2)], true)
        .unwrap();
    let points = doc.control_point_locations(id).unwrap().to_vec();
    doc.select_control_points([grip(id, 1)], SelectionMode::Add)
        .unwrap();
    let transform = viboceros_geometry::AffineTransform3::from_translation(
        viboceros_geometry::Vector3::try_new(0., 0., 2.).unwrap(),
    );
    doc.transform_objects_and_grips([], [grip(id, 1)], transform, false, CopyGroupPolicy::Omit)
        .unwrap();
    assert_eq!(doc.control_point_locations(id).unwrap()[0], points[0]);
    assert_eq!(doc.control_point_locations(id).unwrap()[2], points[2]);
    assert_eq!(
        doc.control_point_locations(id).unwrap()[1].z(),
        points[1].z() + 2.
    );
    assert_eq!(doc.is_control_point_hidden(grip(id, 0)), Some(true));
    doc.undo().unwrap();
    assert_eq!(doc.control_point_locations(id), Some(points.as_slice()));
    assert_eq!(doc.is_control_point_hidden(grip(id, 0)), Some(true));
    doc.redo().unwrap();
    assert_eq!(doc.is_control_point_hidden(grip(id, 2)), Some(true));
}

#[test]
fn history_restores_logical_picks_and_shows_the_owner_if_a_restored_pick_was_hidden() {
    let (mut doc, id, _, _) = source();
    doc.clear_selection();
    doc.clear_history().unwrap();
    doc.set_control_points_hidden([grip(id, 2)], true).unwrap();
    doc.select_control_points([grip(id, 1), grip(id, 2)], SelectionMode::Add)
        .unwrap();
    let points = doc.control_point_locations(id).unwrap().to_vec();
    let transform = viboceros_geometry::AffineTransform3::from_translation(
        viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap(),
    );
    doc.transform_objects_and_grips([], [grip(id, 1)], transform, false, CopyGroupPolicy::Omit)
        .unwrap();
    assert_eq!(doc.is_control_point_hidden(grip(id, 2)), Some(true));
    assert_eq!(doc.selected_logical_control_points().count(), 1);
    assert_eq!(doc.control_point_locations(id).unwrap()[2], points[2]);
    doc.undo().unwrap();
    assert_eq!(doc.control_point_locations(id), Some(points.as_slice()));
    assert_eq!(doc.selected_control_points().count(), 2);
    assert_eq!(doc.is_control_point_hidden(grip(id, 2)), Some(false));
    doc.redo().unwrap();
    assert_eq!(doc.selected_control_points().count(), 1);
    assert_eq!(doc.is_control_point_hidden(grip(id, 2)), Some(false));
}
