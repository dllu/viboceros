use super::*;

#[test]
fn control_point_picks_validate_atomically_and_do_not_consume_redo() {
    let mut document = Document::default();
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let source = document
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_clamped_uniform(2, vec![p(2., 0.), p(0., 2.), p(-2., 0.), p(0., -2.)])
                .unwrap(),
        ))
        .unwrap();
    document.add_geometry(Geometry::Point(p(8., 0.))).unwrap();
    document.undo().unwrap();
    let redo = document.redo_label().map(str::to_owned);
    document.enable_control_points([source]).unwrap();
    let first = ControlPointId {
        object: source,
        index: 0,
    };
    document
        .select_control_points([first], SelectionMode::Add)
        .unwrap();
    let before = format!("{document:?}");
    assert!(
        document
            .select_control_points(
                [
                    first,
                    ControlPointId {
                        object: source,
                        index: 4
                    }
                ],
                SelectionMode::Replace
            )
            .is_err()
    );
    assert_eq!(format!("{document:?}"), before);
    assert!(
        document
            .enable_control_points([source, ObjectId::new()])
            .is_err()
    );
    assert_eq!(format!("{document:?}"), before);
    assert_eq!(document.redo_label(), redo.as_deref());
    document.redo().unwrap();
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.selected_control_points().count(), 1);
    document.clear_selection();
    assert_eq!(document.selected_control_points().count(), 0);
    assert_eq!(document.control_points().count(), 4);
}

#[test]
fn source_geometry_changes_refresh_grips_and_clear_stale_index_picks() {
    let mut document = Document::default();
    let curve = |n| {
        Geometry::NurbsCurve(
            NurbsCurve::try_clamped_uniform(
                2,
                (0..n)
                    .map(|i| Point3::try_new(i as f64, (i % 2) as f64, 0.).unwrap())
                    .collect(),
            )
            .unwrap(),
        )
    };
    let id = document.add_geometry(curve(4)).unwrap();
    document.enable_control_points([id]).unwrap();
    document
        .select_control_points(
            [ControlPointId {
                object: id,
                index: 3,
            }],
            SelectionMode::Add,
        )
        .unwrap();
    document
        .replace_object_geometries([(id, curve(3))])
        .unwrap();
    assert_eq!(document.selected_control_points().count(), 0);
    assert_eq!(document.control_point_locations(id).unwrap().len(), 3);
    document.undo().unwrap();
    assert_eq!(document.control_point_locations(id).unwrap().len(), 4);
    document
        .select_control_points(
            [ControlPointId {
                object: id,
                index: 3,
            }],
            SelectionMode::Add,
        )
        .unwrap();
    let before = format!("{document:?}");
    document.begin_transaction("Rejected edit").unwrap();
    document
        .replace_object_geometries([(id, curve(3))])
        .unwrap();
    document.rollback_transaction().unwrap();
    assert_eq!(format!("{document:?}"), before);
}

#[test]
fn control_point_display_tracks_unit_rescaling_and_history() {
    let mut document = Document::default();
    let id = document
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_clamped_uniform(
                2,
                [(-2., 0.), (0., 2.), (2., 0.)]
                    .map(|(x, y)| Point3::try_new(x, y, 0.).unwrap())
                    .to_vec(),
            )
            .unwrap(),
        ))
        .unwrap();
    document.enable_control_points([id]).unwrap();
    let original = document.control_point_locations(id).unwrap().to_vec();
    document.set_units(LengthUnitSystem::Meters, true).unwrap();
    let Geometry::NurbsCurve(curve) = document.object(id).unwrap().geometry() else {
        panic!("source changed type")
    };
    let scaled = curve.extract_point_locations().unwrap();
    assert_ne!(scaled, original);
    assert_eq!(document.control_point_locations(id).unwrap(), scaled);
    document.undo().unwrap();
    assert_eq!(document.control_point_locations(id).unwrap(), original);
    document.redo().unwrap();
    assert_eq!(document.control_point_locations(id).unwrap(), scaled);
}

#[test]
fn mixed_grip_transform_failure_preserves_geometry_picks_history_and_redo() {
    let mut doc = Document::default();
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let point = doc.add_geometry(Geometry::Point(p(0.25, 0.))).unwrap();
    let mesh = doc
        .add_geometry(Geometry::Mesh(
            TriangleMesh::try_new_faces(
                vec![p(2., 0.), p(0., 2.), p(-2., 0.), p(0., -2.), p(2., 0.)],
                vec![
                    viboceros_geometry::MeshFace::Quad([0, 1, 2, 3]),
                    viboceros_geometry::MeshFace::Triangle([4, 2, 3]),
                ],
                doc.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    doc.add_geometry(Geometry::Point(p(9., 0.))).unwrap();
    doc.undo().unwrap();
    doc.enable_control_points([mesh]).unwrap();
    let grips = [0, 2].map(|index| ControlPointId {
        object: mesh,
        index,
    });
    doc.select_control_points(grips, SelectionMode::Add)
        .unwrap();
    doc.select_objects_direct([point], SelectionMode::Add)
        .unwrap();
    let before = format!("{doc:?}");
    let overflow = AffineTransform3::try_nonuniform_scale(
        Point3::try_new(0., 0., 0.).unwrap(),
        [f64::MAX, 1., 1.],
    )
    .unwrap();
    assert!(
        doc.transform_objects_and_grips([point], grips, overflow, false, CopyGroupPolicy::Preserve)
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
    // Invalid frozen IDs fail before the valid ordinary peer is transformed.
    assert!(
        doc.transform_objects_and_grips(
            [point],
            [ControlPointId {
                object: mesh,
                index: 5
            }],
            AffineTransform3::identity(),
            false,
            CopyGroupPolicy::Preserve
        )
        .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
    doc.redo().unwrap();
    assert_eq!(doc.objects().len(), 3);
}

#[test]
fn partial_grip_edits_rollback_and_exchange_display_state_across_history() {
    let mut doc = Document::default();
    let curve = NurbsCurve::try_clamped_uniform(
        2,
        vec![
            Point3::try_new(2., 0., 0.).unwrap(),
            Point3::try_new(0., 2., 0.).unwrap(),
            Point3::try_new(-2., 0., 0.).unwrap(),
            Point3::try_new(0., -2., 0.).unwrap(),
        ],
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
    doc.enable_control_points([id]).unwrap();
    let pick = ControlPointId {
        object: id,
        index: 0,
    };
    doc.select_control_points([pick], SelectionMode::Add)
        .unwrap();
    let map = AffineTransform3::from_translation(
        viboceros_geometry::Vector3::try_new(1., 2., 3.).unwrap(),
    );
    let before = format!("{doc:?}");
    doc.begin_transaction("Rejected grip edit").unwrap();
    doc.transform_objects_and_grips([], [pick], map, false, CopyGroupPolicy::Preserve)
        .unwrap();
    doc.rollback_transaction().unwrap();
    assert_eq!(format!("{doc:?}"), before);
    doc.transform_objects_and_grips([], [pick], map, false, CopyGroupPolicy::Preserve)
        .unwrap();
    let changed = doc.object(id).unwrap().geometry().clone();
    doc.disable_control_points();
    doc.undo().unwrap();
    assert_eq!(
        doc.selected_control_points()
            .map(|(id, _)| id)
            .collect::<Vec<_>>(),
        [pick]
    );
    doc.redo().unwrap();
    assert_eq!(doc.object(id).unwrap().geometry(), &changed);
    assert!(doc.control_point_locations(id).is_none());
    // Another replay exchanges both states, including the off state on Redo.
    doc.undo().unwrap();
    assert_eq!(doc.selected_control_points().count(), 1);
    doc.redo().unwrap();
    assert!(doc.control_point_locations(id).is_none());
}
