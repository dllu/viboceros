use std::collections::BTreeSet;
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::Point3;
fn subd(closed: bool) -> viboceros_geometry::SubD {
    let mut points = [[0., 0., 0.], [4., 0., 0.], [0., 3., 0.], [0., 0., 2.]]
        .map(|p| Point3::try_from(p).unwrap())
        .to_vec();
    if !closed {
        points.truncate(3);
    }
    let faces = if closed {
        vec![vec![0, 2, 1], vec![0, 1, 3], vec![1, 2, 3], vec![2, 0, 3]]
    } else {
        vec![vec![0, 1, 2]]
    };
    viboceros_geometry::SubD::try_from_polygons(points, faces).unwrap()
}
#[test]
fn subd_selectors_classify_boundaries_additively_without_geometry_history() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let open = doc.add_geometry(Geometry::SubD(subd(false))).unwrap();
    let closed = doc.add_geometry(Geometry::SubD(subd(true))).unwrap();
    let hidden = doc.add_geometry(Geometry::SubD(subd(false))).unwrap();
    let locked = doc.add_geometry(Geometry::SubD(subd(true))).unwrap();
    doc.set_objects_visibility([hidden], false).unwrap();
    doc.set_objects_locked([locked], true).unwrap();
    let point = doc
        .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
        .unwrap();
    for (name, expected) in [
        ("SelOpenSubD", vec![open]),
        ("SelClosedSubD", vec![closed]),
        ("_sElSuBd", vec![open, closed]),
    ] {
        doc.clear_selection();
        doc.select_object(point, SelectionMode::Add).unwrap();
        let history = doc.undo_label().map(str::to_owned);
        registry.execute(&mut doc, name).unwrap();
        assert_eq!(
            doc.selected_object_ids().collect::<BTreeSet<_>>(),
            expected.into_iter().chain([point]).collect()
        );
        assert_eq!(doc.undo_label(), history.as_deref());
    }
    let before = doc.selected_object_ids().collect::<Vec<_>>();
    assert!(registry.execute(&mut doc, "SelSubD extra").is_err());
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), before);
}
