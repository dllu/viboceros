use super::*;
fn fixture() -> (Document, ObjectId) {
    let mut doc = Document::default();
    let s = viboceros_geometry::SubD::try_from_polygons(
        [
            [0., 0., 0.],
            [4., 0., 0.],
            [8., 0., 0.],
            [0., 3., 0.],
            [4., 3., 1.],
            [8., 3., 2.],
        ]
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect(),
        vec![vec![0, 1, 4, 3], vec![1, 2, 5, 4]],
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::SubD(s)).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    (doc, id)
}
#[test]
fn offset_subd_rejected_input_and_stale_plans_preserve_model_and_redo() {
    let (mut doc, id) = fixture();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 20,20,20").unwrap();
    doc.undo().unwrap();
    let before = doc.object(id).unwrap().geometry_snapshot().clone();
    let defaults = registry.offset_subd_options();
    for input in [
        "OffsetSubD 0",
        "OffsetSubD NaN",
        "OffsetSubD Solid=Maybe",
        "OffsetSubD 1 Distance=2",
        "OffsetSubD Solid=Yes Solid=No",
    ] {
        assert!(registry.execute(&mut doc, input).is_err());
        assert_eq!(registry.offset_subd_options(), defaults);
        assert_eq!(doc.redo_label(), Some("Point"));
        assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &before);
    }
    let plan = Plan::prepare(&doc, Options::default(), &BTreeSet::new()).unwrap();
    doc.add_group(Some("changed".into()), [id]).unwrap();
    assert!(matches!(
        plan.commit(&mut doc, false),
        Err(CommandError::OffsetSubDStale)
    ));
    assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &before);
}
#[test]
fn offset_subd_mixed_sources_fail_atomically_and_per_object_flips_are_independent() {
    let (mut doc, id) = fixture();
    let other = doc
        .add_geometry(Geometry::Point(Point3::try_new(20., 20., 20.).unwrap()))
        .unwrap();
    doc.select_objects_direct([id, other], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    assert!(Plan::prepare(&doc, Options::default(), &BTreeSet::new()).is_err());
    assert!(!doc.can_undo());
    assert_eq!(doc.objects().len(), 2);
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let plan = Plan::prepare(&doc, Options::default(), &BTreeSet::from([id])).unwrap();
    plan.commit(&mut doc, false).unwrap();
    assert_eq!(doc.objects().len(), 3);
    assert!(!doc.is_selected(doc.objects().last().unwrap().id()));
    let Geometry::SubD(offset) = doc.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(offset.control_points()[0].to_array(), [0., 0., -1.]);
}
