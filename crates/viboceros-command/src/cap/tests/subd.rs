use super::*;

fn subd_source(doc: &mut Document) -> ObjectId {
    let subd = viboceros_geometry::SubD::try_from_polygons(
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
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect(),
        vec![
            vec![0, 1, 5, 4],
            vec![1, 2, 6, 5],
            vec![2, 3, 7, 6],
            vec![3, 0, 4, 7],
        ],
    )
    .unwrap();
    doc.add_geometry(Geometry::SubD(subd)).unwrap()
}

#[test]
fn subd_cap_late_unsupported_input_and_options_leave_objects_and_history_unchanged() {
    let mut doc = Document::default();
    let id = subd_source(&mut doc);
    let point = doc
        .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
        .unwrap();
    doc.select_objects_direct([id, point], SelectionMode::Replace)
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let history = doc.undo_label().map(str::to_owned);
    let registry = CommandRegistry::with_builtins();
    assert!(matches!(
        registry.execute(&mut doc, "Cap DeleteInput=No"),
        Err(CommandError::UnsupportedCapGeometry)
    ));
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.undo_label(), history.as_deref());
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    for command in [
        "Cap Triangles=Maybe",
        "Cap Crease=Yes Crease=No",
        "Cap DeleteInput=Maybe",
    ] {
        assert!(registry.execute(&mut doc, command).is_err());
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.undo_label(), history.as_deref());
    }
    registry
        .execute_postselected(&mut doc, "Cap Crease=No Triangles=Yes", Default::default())
        .unwrap();
    assert!(!doc.is_selected(id));
    assert_eq!(doc.undo_label(), Some("Cap"));
    let Geometry::SubD(subd) = doc.object(id).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(subd.faces().len(), 12);
    assert!(subd.edge_tags().iter().all(|&t| t == 1));
    doc.undo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
}
