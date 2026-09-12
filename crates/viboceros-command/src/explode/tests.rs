use super::*;

#[test]
fn oversized_cloud_is_rejected_without_editing_source_or_discarding_redo() {
    let mut document = Document::default();
    let registry = CommandRegistry::with_builtins();
    let point = Point3::try_new(0., 0., 0.).unwrap();
    let points = vec![point; MAX_SPAN_OUTPUT_OBJECTS + 1];
    let source_data = points.as_ptr();
    let source = document
        .add_geometry(Geometry::PointCloud(PointCloud3::try_new(points).unwrap()))
        .unwrap();
    let other = document.add_geometry(Geometry::Point(point)).unwrap();
    document.undo().unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    let label = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "Explode"),
        Err(CommandError::TooManySpanOutputObjects {
            command: "Explode",
            maximum: MAX_SPAN_OUTPUT_OBJECTS
        })
    ));
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [source]);
    assert_eq!(document.undo_label(), label.as_deref());
    let Geometry::PointCloud(cloud) = document.object(source).unwrap().geometry() else {
        panic!("source cloud expected")
    };
    assert_eq!(cloud.points().as_ptr(), source_data);
    assert_eq!(cloud.points().len(), MAX_SPAN_OUTPUT_OBJECTS + 1);
    assert!(document.can_redo());
    document.redo().unwrap();
    assert!(document.object(other).is_some());
}
