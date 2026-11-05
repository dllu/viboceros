use viboceros_command::{CommandRegistry, ObjectSelectionFilter};
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::{Point3, SubD};
fn setup() -> (Document, CommandRegistry, viboceros_document::ObjectId) {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut doc, "Units Millimeters Scale=No")
        .unwrap();
    let subd = SubD::try_from_polygons(
        [[0., 0., 0.], [4., 0., 0.], [4., 3., 0.], [0., 3., 0.]]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        vec![vec![0, 1, 2, 3]],
    )
    .unwrap()
    .try_with_vertex_tags(&[(0, 3), (1, 3), (2, 3), (3, 3)])
    .unwrap();
    let owner = doc.add_geometry(Geometry::SubD(subd)).unwrap();
    doc.clear_history().unwrap();
    registry.execute(&mut doc, "Point 40,50,60").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    doc.select_objects_direct([owner], SelectionMode::Replace)
        .unwrap();
    (doc, registry, owner)
}
#[test]
fn planar_subd_area_units_appearance_and_centroid_obey_analytic_references() {
    let (mut doc, r, owner) = setup();
    let source = doc.object(owner).unwrap().clone();
    assert!(ObjectSelectionFilter::Area.accepts_object(&source));
    assert!(ObjectSelectionFilter::Volume.accepts_object(&source));
    for _ in 0..2 {
        let report = r.execute(&mut doc, "Area Units=Centimeters").unwrap();
        let value = report
            .split_once("total area ")
            .unwrap()
            .1
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<f64>()
            .unwrap();
        assert!((value - 0.12).abs() < 1e-12);
        assert!(report.ends_with("Centimetres²"));
        assert!(doc.can_redo());
        assert!(!doc.can_undo());
        assert!(doc.is_selected(owner));
        r.execute(&mut doc, "SubDDisplayToggle").unwrap();
    }
    r.execute(&mut doc, "AreaCentroid").unwrap();
    let Geometry::Point(p) = doc.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert!(
        p.distance_to(Point3::try_new(2., 1.5, 0.).unwrap())
            .unwrap()
            < 1e-12
    );
    assert_eq!(doc.object(owner), Some(&source));
    assert_eq!(doc.undo_label(), Some("AreaCentroid"));
    assert!(!doc.can_redo());
    r.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().len(), 1);
    r.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().len(), 2);
}
#[test]
fn failed_mixed_area_query_keeps_subd_sources_selection_and_pending_redo() {
    let (mut doc, r, owner) = setup();
    let point = doc
        .add_geometry(Geometry::Point(Point3::try_new(5., 6., 7.).unwrap()))
        .unwrap();
    doc.clear_history().unwrap();
    r.execute(&mut doc, "Point 40,50,60").unwrap();
    r.execute(&mut doc, "Undo").unwrap();
    doc.select_objects_direct([owner, point], SelectionMode::Replace)
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    assert!(r.execute(&mut doc, "Area").is_err());
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.selected_object_count(), 2);
    assert!(!doc.can_undo());
    assert!(doc.can_redo());
}
