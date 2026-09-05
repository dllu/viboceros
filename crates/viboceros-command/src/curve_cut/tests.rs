use super::*;

#[test]
fn native_polyline_cuts_retain_station_parameters_and_undo() {
    let registry = CommandRegistry::with_builtins();
    for (command, expected) in [
        (
            "Split CuttingObjects=4,3",
            vec![[-7.0, 0.5], [0.5, 8.0], [8.0, 13.0]],
        ),
        (
            "Trim 4,3 ApparentIntersections=No",
            vec![[-7.0, 0.5], [8.0, 13.0]],
        ),
    ] {
        let mut document = Document::default();
        registry
            .execute(&mut document, "Polyline 0,0 4,3 10,0")
            .unwrap();
        let id = document.objects().next().unwrap().id();
        document.select_object(id, SelectionMode::Replace).unwrap();
        registry
            .execute(&mut document, "Reparameterize -7 13")
            .unwrap();
        registry.execute(&mut document, "Group NativeCuts").unwrap();
        registry
            .execute(&mut document, "SetObjectName NativePolyline")
            .unwrap();
        let before = document.object(id).unwrap().clone();
        let view = before.geometry().curve_ref().unwrap();
        registry.execute(&mut document, "Line 3,-10 3,10").unwrap();
        registry.execute(&mut document, "Line 7,-10 7,10").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry.execute(&mut document, command).unwrap();
        let outputs = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(outputs.len(), expected.len());
        for (object, [a, b]) in outputs.iter().zip(expected) {
            let Geometry::Polyline(curve) = object.geometry() else {
                panic!("native polyline lost");
            };
            assert!((*curve.domain().start() - a).abs() < 1e-12);
            assert!((*curve.domain().end() - b).abs() < 1e-12);
            assert_eq!(object.attributes(), before.attributes());
            assert!(
                document
                    .group_by_name("NativeCuts")
                    .unwrap()
                    .members()
                    .any(|id| id == object.id())
            );
            for i in 0..=16 {
                let t = CurveRef::Polyline(curve)
                    .parameter_at(i as Real / 16.0)
                    .unwrap();
                assert!(
                    curve
                        .evaluate(t)
                        .unwrap()
                        .distance_to(view.evaluate(t).unwrap())
                        .unwrap()
                        < 1e-12
                );
            }
        }
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.object(id).unwrap(), &before);
        assert_eq!(document.objects().count(), 3);
    }
}

#[test]
fn closed_seam_is_a_cut_and_wrapped_command_pieces_stay_arcs() {
    let registry = CommandRegistry::with_builtins();
    for (cutter, command, count) in [
        ("Line -10,0 10,0", "Split CuttingObjects=0,5", 2),
        ("Line -10,0 10,0", "Trim 0,5 ApparentIntersections=No", 1),
        ("Line 2,-10 2,10", "Split CuttingObjects=-5,0", 2),
        ("Line 2,-10 2,10", "Trim -5,0 ApparentIntersections=No", 1),
    ] {
        let mut document = Document::default();
        registry.execute(&mut document, "Circle 0,0 5").unwrap();
        let id = document.objects().next().unwrap().id();
        document.select_object(id, SelectionMode::Replace).unwrap();
        registry
            .execute(&mut document, "Reparameterize -7 13")
            .unwrap();
        let before = document.object(id).unwrap().clone();
        registry.execute(&mut document, cutter).unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry.execute(&mut document, command).unwrap();
        assert_eq!(document.selected_object_count(), count);
        for object in document.selected_objects() {
            let Geometry::Arc(arc) = object.geometry() else {
                panic!("wrapped analytic result lost its arc representation");
            };
            for i in 0..=16 {
                let t = CurveRef::Arc(arc).parameter_at(i as Real / 16.0).unwrap();
                let old_t = if t > 13.0 { t - 20.0 } else { t };
                assert!(
                    arc.evaluate(t)
                        .unwrap()
                        .distance_to(
                            before
                                .geometry()
                                .curve_ref()
                                .unwrap()
                                .evaluate(old_t)
                                .unwrap()
                        )
                        .unwrap()
                        < 1e-11
                );
            }
        }
        if cutter == "Line -10,0 10,0" && command.starts_with("Trim") {
            assert_eq!(
                document
                    .object(id)
                    .unwrap()
                    .geometry()
                    .curve_ref()
                    .unwrap()
                    .domain(),
                3.0..=13.0
            );
        }
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.object(id).unwrap(), &before);
    }
}

#[test]
fn projected_circle_trim_maps_parameters_back_to_the_unprojected_source() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Circle 0,0,1 5").unwrap();
    let id = document.objects().next().unwrap().id();
    document.select_object(id, SelectionMode::Replace).unwrap();
    registry
        .execute(&mut document, "Reparameterize -7 13")
        .unwrap();
    registry
        .execute(&mut document, "Line 2,-10,0 2,10,0")
        .unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    registry.execute(&mut document, "Trim -5,0,1").unwrap();
    let Geometry::Arc(arc) = document.object(id).unwrap().geometry() else {
        panic!("expected an arc");
    };
    assert!((*arc.domain().start() - 9.309898804344545).abs() < 1e-12);
    assert!((*arc.domain().end() - 16.690101195655455).abs() < 1e-12);
    for i in 0..=32 {
        let point = arc
            .evaluate(CurveRef::Arc(arc).parameter_at(i as Real / 32.0).unwrap())
            .unwrap();
        assert_eq!(point.z(), 1.0);
        assert!((point.x().hypot(point.y()) - 5.0).abs() < 1e-12);
    }
}

#[test]
fn cut_deduplication_is_invariant_under_parameter_translation() {
    let point = Point3::try_new(0.0, 0.0, 0.0).unwrap();
    for origin in [0.0, -1e12, 1e12] {
        assert!(!trim_intersections_near(
            (origin + 2.0, point),
            (origin + 12.0, point),
            10.0,
            Tolerance::DEFAULT
        ));
        assert!(trim_intersections_near(
            (origin + 2.0, point),
            (origin + 2.0, point),
            10.0,
            Tolerance::DEFAULT
        ));
    }
    assert!(!trim_intersections_near(
        (-1e308, point),
        (1e308, point),
        1e308,
        Tolerance::DEFAULT
    ));
}

#[test]
fn one_closed_cut_does_not_relocate_the_seam_or_replace_the_source() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Circle 0,0 5").unwrap();
    let id = document.objects().next().unwrap().id();
    let before = document.object(id).unwrap().clone();
    registry
        .execute(&mut document, "Line -5,-10 -5,10")
        .unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(&mut document, "Split CuttingObjects=0,5")
        .unwrap();
    assert_eq!(document.object(id).unwrap(), &before);
    assert_eq!(document.objects().count(), 2);
    assert_eq!(document.selected_object_count(), 1);
    assert!(document.is_selected(id));
}

#[test]
fn quadratic_tangent_trim_retains_the_exact_double_root() {
    let point = |x, y| Point3::try_new(x, y, 0.0).unwrap();
    let mut document = Document::default();
    let curve = NurbsCurve::try_clamped_uniform(
        2,
        vec![point(0.0, 0.0), point(5.0, 8.0), point(10.0, 0.0)],
    )
    .unwrap();
    let id = document.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut document, "Line 0,4 10,4").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(&mut document, "Trim 2,2.56 ApparentIntersections=No")
        .unwrap();
    let Geometry::NurbsCurve(curve) = document.object(id).unwrap().geometry() else {
        panic!("expected a NURBS curve");
    };
    // y(t) = 16*t*(1-t), so the exact y=4 contact is t=0.5.
    assert_eq!(*curve.domain().start(), 0.5);
    assert_eq!(curve.evaluate(0.5).unwrap(), point(5.0, 4.0));
}
