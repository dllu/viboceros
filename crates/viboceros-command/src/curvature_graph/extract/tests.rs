use super::*;
fn scene() -> (CommandRegistry, Document, ObjectId) {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    r.execute(&mut d, "Circle 0,0,0 2").unwrap();
    let id = d.objects().next().unwrap().id();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut d, "CurvatureGraph").unwrap();
    (r, d, id)
}
#[test]
fn extract_curvature_graph_uses_displayed_outline_current_layer_and_one_undo_edit() {
    let (r, mut d, id) = scene();
    let original = d.object(id).unwrap().geometry().clone();
    let view = r.curvature_graph_view(&d).unwrap().unwrap();
    let layer = d.add_layer("GraphOutput", ColorRgb::new(8, 9, 10)).unwrap();
    d.set_current_layer(layer).unwrap();
    r.execute(&mut d, "ExtractCurvatureGraph").unwrap();
    let output = d.objects().find(|o| o.id() != id).unwrap();
    assert_eq!(output.attributes().layer_id(), layer);
    assert!(output.attributes().name().is_none());
    assert!(output.attributes().user_text().is_empty());
    assert!(!d.is_selected(output.id()));
    assert!(d.is_selected(id));
    let Geometry::Polyline(poly) = output.geometry() else {
        panic!("expected outline polyline")
    };
    assert_eq!(poly.vertices().len(), 281);
    assert_eq!(poly.vertices().first(), poly.vertices().last());
    for point in poly.vertices() {
        assert!(
            (point
                .distance_to(Point3::try_new(0., 0., 0.).unwrap())
                .unwrap()
                - 2.5)
                .abs()
                < 1e-12
        );
    }
    assert_eq!(d.object(id).unwrap().geometry(), &original);
    assert_eq!(d.undo_label(), Some("ExtractCurvatureGraph"));
    assert!(Arc::ptr_eq(
        &view.graphs,
        &r.curvature_graph_view(&d).unwrap().unwrap().graphs
    ));
    d.undo().unwrap();
    assert_eq!(d.objects().len(), 1);
    d.redo().unwrap();
    assert_eq!(d.objects().len(), 2);
}
#[test]
fn extract_curvature_graph_filters_ungraphed_sources_and_obeys_postselection_cleanup() {
    let (r, mut d, id) = scene();
    r.execute(&mut d, "Circle 5,0,0 2").unwrap();
    let other = d.objects().last().unwrap().id();
    d.select_objects_direct([other], SelectionMode::Replace)
        .unwrap();
    let count = d.objects().len();
    let undo = d.undo_label().map(str::to_owned);
    assert!(matches!(
        r.execute(&mut d, "ExtractCurvatureGraph"),
        Err(CommandError::OperationDeclined)
    ));
    assert_eq!(d.objects().len(), count);
    assert_eq!(d.undo_label(), undo.as_deref());
    assert_eq!(d.selected_object_count(), 0);
    d.select_objects_direct([id, other], SelectionMode::Replace)
        .unwrap();
    r.execute_postselected(&mut d, "ExtractCurvatureGraph", CommandContext::default())
        .unwrap();
    assert_eq!(d.objects().len(), count + 1);
    assert_eq!(d.selected_object_count(), 0);
    r.execute(&mut d, "CurvatureGraphOff").unwrap();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    assert!(r.execute(&mut d, "ExtractCurvatureGraph").is_err());
}
#[test]
fn extract_curvature_graph_flat_curves_preserve_redo_and_invalid_arguments_do_not_mutate() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    r.execute(&mut d, "Line 0,0,0 3,0,0").unwrap();
    r.execute(&mut d, "SelAll").unwrap();
    r.execute(&mut d, "CurvatureGraph").unwrap();
    r.execute(&mut d, "Point 5,5,5").unwrap();
    d.undo().unwrap();
    let before = format!("{d:?}");
    r.execute(&mut d, "ExtractCurvatureGraph").unwrap();
    assert_eq!(format!("{d:?}"), before);
    assert!(r.execute(&mut d, "ExtractCurvatureGraph junk").is_err());
    assert_eq!(format!("{d:?}"), before);
    d.redo().unwrap();
    assert_eq!(d.objects().len(), 2);
}
