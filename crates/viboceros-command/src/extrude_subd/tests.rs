use super::*;
fn fixture() -> (Document, ObjectId) {
    let mut doc = Document::default();
    let s = viboceros_geometry::SubD::try_from_polygons(
        [[0., 0., 0.], [4., 0., 0.], [4., 3., 0.], [0., 3., 0.]]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        vec![vec![0, 1, 2, 3]],
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::SubD(s)).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    (doc, id)
}
#[test]
fn extrude_subd_invalid_inputs_preserve_redo_and_preferences() {
    let (mut doc, id) = fixture();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 10,20,30").unwrap();
    doc.undo().unwrap();
    let before = doc.object(id).unwrap().geometry_snapshot().clone();
    for input in [
        "ExtrudeSubD 0",
        "ExtrudeSubD NaN",
        "ExtrudeSubD 1 Faces=9",
        "ExtrudeSubD 1 Edges=9",
        "ExtrudeSubD 1 Basis=UVN Direction=Z",
        "ExtrudeSubD 1 Direction=V",
        "ExtrudeSubD 1 Distance=2",
        "ExtrudeSubD 1 Basis=WCS Direction=0,0,0",
        "ExtrudeSubD 1 Faces=-1",
        "ExtrudeSubD 1 Faces=0 Faces=0",
    ] {
        let options = registry.extrude_subd_options();
        assert!(registry.execute(&mut doc, input).is_err(), "{input}");
        assert_eq!(registry.extrude_subd_options(), options);
        assert_eq!(doc.redo_label(), Some("Point"));
        assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &before);
    }
    assert!(
        Plan::prepare(
            &doc,
            [Target::Face(id, 0), Target::Edge(id, 0)],
            Options {
                direction: Direction::V,
                ..Default::default()
            }
        )
        .is_err()
    );
}
#[test]
fn extrude_subd_stale_sources_attributes_groups_selection_and_tolerance_are_rejected() {
    for change in 0..5 {
        let (mut doc, id) = fixture();
        let plan = Plan::prepare(&doc, [Target::Whole(id)], Options::default()).unwrap();
        let before = doc.object(id).unwrap().geometry_snapshot().clone();
        match change {
            0 => {
                doc.clear_selection();
            }
            1 => {
                doc.set_object_names([(id, Some("Changed".into()))])
                    .unwrap();
            }
            2 => {
                doc.add_group(Some("Changed".into()), [id]).unwrap();
            }
            3 => {
                doc.set_tolerance(Tolerance::try_new(0.002, 1e-6, 0.01).unwrap());
            }
            _ => {
                doc.replace_object_geometries([(
                    id,
                    Geometry::Point(Point3::try_new(1., 2., 3.).unwrap()),
                )])
                .unwrap();
            }
        }
        let snapshot = doc.object(id).unwrap().geometry_snapshot().clone();
        let label = doc.undo_label().map(str::to_owned);
        assert!(matches!(
            plan.commit(&mut doc),
            Err(CommandError::ExtrudeSubDStale)
        ));
        assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &snapshot);
        assert_eq!(doc.undo_label(), label.as_deref());
        if change < 4 {
            assert_eq!(snapshot, before);
        }
    }
}
#[test]
fn extrude_subd_batches_validate_all_sources_before_editing() {
    let (mut doc, id) = fixture();
    let point = doc
        .add_geometry(Geometry::Point(Point3::try_new(1., 2., 3.).unwrap()))
        .unwrap();
    doc.select_objects_direct([id, point], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    let before = doc.object(id).unwrap().geometry_snapshot().clone();
    assert!(
        CommandRegistry::with_builtins()
            .execute(&mut doc, "ExtrudeSubD 2")
            .is_err()
    );
    assert!(!doc.can_undo());
    assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &before);
}
