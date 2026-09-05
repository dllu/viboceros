use super::*;

#[test]
fn only_the_nearest_selected_object_is_differentially_evaluated() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Sphere 0,0,0 2").unwrap();
    registry.execute(&mut doc, "Line -1,0,3 1,0,3").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    // The farther sphere's closest point is a singular NURBS pole. It must
    // not prevent evaluation of the closer, regular line.
    let report = registry.execute(&mut doc, "Curvature 0,0,3").unwrap();
    assert!(report.starts_with("Curve curvature"));
    assert!(report.contains("radius infinite"));
    assert_eq!(doc.objects().len(), 2);
    assert_eq!(doc.selected_object_count(), 2);
}

#[test]
fn curve_measurement_is_read_only_and_markers_have_atomic_fresh_state() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Circle 0,0,0 2").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    registry.execute(&mut document, "Group Source").unwrap();
    registry
        .execute(&mut document, "Layer New Markers")
        .unwrap();
    registry
        .execute(&mut document, "Layer Current Markers")
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let selection = document.selected_object_ids().collect::<Vec<_>>();
    let undo = document.undo_label().map(str::to_owned);
    let report = registry
        .execute(&mut document, "Curvature MarkCurvature=No 2,0,0")
        .unwrap();
    assert!(report.contains("curvature 0.500000000000000"));
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(document.undo_label(), undo.as_deref());
    registry
        .execute(&mut document, "Curvature MarkCurvature=Yes 2,0,0")
        .unwrap();
    let after = document.objects().cloned().collect::<Vec<_>>();
    assert_eq!(after.len(), 3);
    assert_eq!(after[0], before[0]);
    assert!(matches!(after[1].geometry(), Geometry::Point(_)));
    let Geometry::Circle(circle) = after[2].geometry() else {
        panic!("circle expected")
    };
    assert_eq!(circle.center(), Point3::try_new(0.0, 0.0, 0.0).unwrap());
    assert_eq!(circle.radius(), 2.0);
    for object in &after[1..] {
        assert_eq!(
            object.attributes(),
            &ObjectAttributes::on_layer(document.current_layer_id())
        );
        assert!(!document.is_selected(object.id()));
        assert!(
            document
                .groups()
                .all(|g| !g.members().any(|id| id == object.id()))
        );
    }
    assert_eq!(
        document.selected_object_ids().collect::<Vec<_>>(),
        selection
    );
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
}

#[test]
fn flat_curve_marking_produces_only_a_point_and_failures_are_atomic() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Line 0,0,0 5,0,0").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let report = registry
        .execute(&mut doc, "Curvature 2,0,0 MarkCurvature=Yes")
        .unwrap();
    assert!(report.contains("radius infinite"));
    assert_eq!(doc.objects().len(), 2);
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let undo = doc.undo_label().map(str::to_owned);
    for command in [
        "Curvature",
        "Curvature MarkCurvature=Maybe 1,0,0",
        "Curvature 1,0,0 extra",
        "Curvature 1,0,NaN",
    ] {
        assert!(registry.execute(&mut doc, command).is_err());
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.undo_label(), undo.as_deref());
    }
}

#[test]
fn surface_markers_are_normal_section_half_circles() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry
        .execute(&mut doc, "Cylinder 0,0,0 2 5 Solid=No")
        .unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let report = registry
        .execute(&mut doc, "Curvature MarkCurvature=Yes 2,0,2.5")
        .unwrap();
    assert!(report.contains("maximum-absolute principal -0.500000000000000"));
    assert_eq!(doc.objects().len(), 4);
    let arc = doc
        .objects()
        .find_map(|o| {
            if let Geometry::Arc(a) = o.geometry() {
                Some(a)
            } else {
                None
            }
        })
        .unwrap();
    let line = doc
        .objects()
        .find_map(|o| {
            if let Geometry::Line(a) = o.geometry() {
                Some(a)
            } else {
                None
            }
        })
        .unwrap();
    assert!((line.length().unwrap() - 0.2 * 57.0_f64.sqrt()).abs() < 1e-12);
    assert!((arc.sweep_radians() - std::f64::consts::PI).abs() < 1e-14);
    assert!(
        arc.point_at(0.5)
            .unwrap()
            .is_near(Point3::try_new(2.0, 0.0, 2.5).unwrap(), doc.tolerance())
    );
}
