//! refinement insert knots command regressions.
use super::*;

#[test]
fn insert_knot_refines_a_rational_curve_in_place_and_undoes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let layer = document
        .add_layer("Knot source", ColorRgb::new(28, 84, 140))
        .unwrap();
    let source = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(Point3::try_new(0.0, 0.0, 0.0).unwrap(), 0.75).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(2.0, 3.0, 1.0).unwrap(), 1.5).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(5.0, -1.0, 0.5).unwrap(), 0.6).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(8.0, 2.0, 0.0).unwrap(), 2.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 0.4, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let source_id = document
        .add_geometry_with_attributes(
            Geometry::NurbsCurve(source.clone()),
            ObjectAttributes::on_layer(layer).with_name("rational knot source"),
        )
        .unwrap();
    let point_id = document
        .add_geometry(Geometry::Point(Point3::try_new(20.0, 0.0, 0.0).unwrap()))
        .unwrap();
    document
        .select_objects([source_id, point_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(
                &mut document,
                "InsertKnot 0.6 Multiplicity _2 Symmetrical _Yes",
            )
            .unwrap(),
        "Inserted 3 knot value(s) into selected NURBS curve"
    );
    assert_eq!(document.objects().len(), 2);
    let object = document.object(source_id).unwrap();
    assert_eq!(object.attributes().layer_id(), layer);
    assert_eq!(object.attributes().name(), Some("rational knot source"));
    assert!(document.is_selected(source_id));
    assert!(document.is_selected(point_id));
    let Geometry::NurbsCurve(refined) = object.geometry() else {
        panic!("InsertKnot must retain NURBS curve geometry")
    };
    assert_eq!(
        refined.knots(),
        &[0.0, 0.0, 0.0, 0.4, 0.4, 0.6, 0.6, 1.0, 1.0, 1.0]
    );
    assert_eq!(refined.control_points().len(), 7);
    for parameter in [0.0, 0.13, 0.4, 0.6, 0.89, 1.0] {
        assert!(
            refined
                .evaluate(parameter)
                .unwrap()
                .is_near(source.evaluate(parameter).unwrap(), Tolerance::DEFAULT)
        );
    }
    assert_eq!(document.undo_label(), Some("InsertKnot"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(source_id).unwrap().geometry(),
        &Geometry::NurbsCurve(source)
    );
}

#[test]
fn insert_knot_refines_both_surface_directions_and_preserves_locus() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let source = rational_multi_span_surface();
    let source_id = document
        .add_geometry(Geometry::NurbsSurface(source.clone()))
        .unwrap();
    document
        .select_objects([source_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(
                &mut document,
                "InsertKnot 0.5,12 Multiplicity=2 Direction _Both Symmetrical=Yes",
            )
            .unwrap(),
        "Inserted 6 knot value(s) into selected NURBS surface"
    );
    let Geometry::NurbsSurface(refined) = document.object(source_id).unwrap().geometry() else {
        panic!("InsertKnot must retain NURBS surface geometry")
    };
    assert_eq!(refined.control_point_count_u(), 8);
    assert_eq!(refined.control_point_count_v(), 6);
    assert_eq!(
        refined
            .knots_u()
            .iter()
            .filter(|knot| **knot == 0.5)
            .count(),
        2
    );
    assert_eq!(
        refined
            .knots_u()
            .iter()
            .filter(|knot| **knot == 1.5)
            .count(),
        2
    );
    assert_eq!(
        refined
            .knots_v()
            .iter()
            .filter(|knot| **knot == 12.0)
            .count(),
        2
    );
    for u in [0.0, 0.31, 0.5, 1.5, 1.87, 2.0] {
        for v in [10.0, 10.9, 12.0, 13.4, 14.0] {
            assert!(
                refined
                    .evaluate(u, v)
                    .unwrap()
                    .is_near(source.evaluate(u, v).unwrap(), Tolerance::DEFAULT)
            );
        }
    }

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(source_id).unwrap().geometry(),
        &Geometry::NurbsSurface(source)
    );
}

#[test]
fn insert_knot_accepts_scalar_surface_direction_and_analytic_curves() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let surface_id = document
        .add_geometry(Geometry::NurbsSurface(rational_multi_span_surface()))
        .unwrap();
    document
        .select_objects([surface_id], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "InsertKnot 11 Direction V")
            .unwrap(),
        "Inserted 1 knot value(s) into selected NURBS surface"
    );
    let Geometry::NurbsSurface(surface) = document.object(surface_id).unwrap().geometry() else {
        panic!("InsertKnot must retain NURBS surface geometry")
    };
    assert!(surface.knots_v().contains(&11.0));

    let line_id = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(5.0, 0.0, 0.0).unwrap(),
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .select_objects([line_id], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut document, "_InsertKnot 2").unwrap();
    let Geometry::NurbsCurve(line) = document.object(line_id).unwrap().geometry() else {
        panic!("InsertKnot must convert an analytic curve to NURBS")
    };
    assert_eq!(line.degree(), 1);
    assert_eq!(line.knots(), &[0.0, 0.0, 2.0, 5.0, 5.0]);
}

#[test]
fn insert_knot_rejects_invalid_inputs_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let source = NurbsCurve::try_clamped_uniform(
        3,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 2.0, 0.0).unwrap(),
            Point3::try_new(3.0, -1.0, 0.0).unwrap(),
            Point3::try_new(5.0, 0.0, 0.0).unwrap(),
        ],
    )
    .unwrap();
    let curve_id = document
        .add_geometry(Geometry::NurbsCurve(source.clone()))
        .unwrap();
    let second_curve_id = document
        .add_geometry(Geometry::NurbsCurve(source.clone()))
        .unwrap();
    let point_id = document
        .add_geometry(Geometry::Point(Point3::try_new(9.0, 9.0, 9.0).unwrap()))
        .unwrap();

    document
        .select_objects([point_id], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "InsertKnot 0.5"),
        Err(CommandError::InsertKnotRequiresOneObject { actual: 0 })
    ));
    document
        .select_objects([curve_id, second_curve_id], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "InsertKnot 0.5"),
        Err(CommandError::InsertKnotRequiresOneObject { actual: 2 })
    ));

    document
        .select_objects([curve_id], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "InsertKnot",
        "InsertKnot 0.5,0.5",
        "InsertKnot 0.5 Multiplicity=0",
        "InsertKnot 0.5 Multiplicity=4",
        "InsertKnot 0 Multiplicity=2",
        "InsertKnot -1",
        "InsertKnot 0.5 Direction=Other",
        "InsertKnot 0.5 Direction=U Direction=V",
        "InsertKnot 0.5 Symmetrical=Maybe",
        "InsertKnot 0.5 Multiplicity=one",
        "InsertKnot 0.5 Other=Yes",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(
            document.object(curve_id).unwrap().geometry(),
            &Geometry::NurbsCurve(source.clone()),
            "{command}"
        );
        assert_eq!(document.undo_label(), history.as_deref(), "{command}");
    }

    let surface_id = document
        .add_geometry(Geometry::NurbsSurface(rational_multi_span_surface()))
        .unwrap();
    document
        .select_objects([surface_id], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "InsertKnot 0.5"),
        Err(CommandError::InsertKnotBothDirectionsRequireUv)
    ));
}
