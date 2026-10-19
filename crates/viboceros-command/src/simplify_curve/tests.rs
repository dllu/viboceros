use super::*;
#[test]
fn failed_later_curve_does_not_publish_earlier_simplification_or_destroy_redo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let line = NurbsCurve::try_new(
        3,
        [0., 1., 4., 6.]
            .into_iter()
            .map(|x| Point3::try_new(x, 0., 0.).unwrap())
            .collect(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let first = doc.add_geometry(Geometry::NurbsCurve(line)).unwrap();
    let invalid = doc
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_new(
                1,
                vec![
                    Point3::try_new(0., 3., 0.).unwrap(),
                    Point3::try_new(1., 3., 0.).unwrap(),
                    Point3::try_new(2., 4., 0.).unwrap(),
                    Point3::try_new(3., 4., 0.).unwrap(),
                ],
                vec![0., 0., 0.5, 0.5, 1., 1.],
            )
            .unwrap(),
        ))
        .unwrap();
    doc.select_objects_direct([first, invalid], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    registry.execute(&mut doc, "SetObjectName Future").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    assert!(doc.can_redo());
    assert!(registry.execute(&mut doc, "SimplifyCrv").is_err());
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(doc.can_redo());
    assert!(!doc.can_undo());
}
#[test]
fn noop_invalid_input_and_selection_description_do_not_change_model_history() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Line 0,0,0 6,0,0").unwrap();
    let id = doc.objects().next().unwrap().id();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    doc.clear_history().unwrap();
    registry.execute(&mut doc, "SetObjectName Future").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let before = doc.object(id).unwrap().clone();
    registry.execute(&mut doc, "_simplifycrv").unwrap();
    for input in ["SimplifyCrv Unknown", "SimplifyCrv Tolerance=0"] {
        assert!(registry.execute(&mut doc, input).is_err());
    }
    assert_eq!(doc.object(id), Some(&before));
    assert!(doc.can_redo());
    assert!(!doc.can_undo());
    assert_eq!(
        registry
            .object_selection_prompt("SimplifyCrv")
            .unwrap()
            .unwrap()
            .filter,
        ObjectSelectionFilter::Curves
    );
}
