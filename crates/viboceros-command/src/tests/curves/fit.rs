//! curves fit command regressions.
use super::*;

#[test]
fn fit_curve_adds_arc_length_parameterized_output_on_the_input_layer() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let input_layer = document
        .add_layer("Fit source", ColorRgb::new(20, 40, 60))
        .unwrap();
    let source_id = document
        .add_geometry_with_attributes(
            Geometry::Line(
                LineSegment::try_new(
                    Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                    Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                    document.tolerance(),
                )
                .unwrap(),
            ),
            ObjectAttributes::on_layer(input_layer).with_name("source"),
        )
        .unwrap();
    document
        .select_objects([source_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(
                &mut document,
                "FitCrv Degree=3 Tolerance=0.001 DeleteInput=No OutputLayer=InputObject",
            )
            .unwrap(),
        "Fitted 1 curve(s) as non-rational degree-3 NURBS within tolerance 0.001000000000, retaining the input(s) on the input-object layer"
    );
    assert_eq!(document.objects().len(), 2);
    assert!(matches!(
        document.object(source_id).unwrap().geometry(),
        Geometry::Line(_)
    ));
    assert!(document.is_selected(source_id));
    let output = document.objects().last().unwrap();
    assert_eq!(output.attributes().layer_id(), input_layer);
    assert!(!document.is_selected(output.id()));
    let Geometry::NurbsCurve(curve) = output.geometry() else {
        panic!("FitCrv must create a NURBS curve")
    };
    assert_eq!(curve.degree(), 3);
    assert_eq!(curve.domain(), 0.0..=10.0);
    assert!(!curve.is_rational());
    assert_eq!(curve.control_points().len(), 4);
    assert_eq!(document.undo_label(), Some("FitCrv"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 1);
    assert!(document.is_selected(source_id));
}

#[test]
fn fit_curve_can_replace_a_kinked_input_without_losing_identity_or_attributes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let input_layer = document
        .add_layer("Kinked", ColorRgb::new(70, 80, 90))
        .unwrap();
    let source = Polyline3::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(5.0, 0.0, 0.0).unwrap(),
            Point3::try_new(5.0, 5.0, 0.0).unwrap(),
            Point3::try_new(0.0, 5.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        ],
        document.tolerance(),
    )
    .unwrap();
    let source_id = document
        .add_geometry_with_attributes(
            Geometry::Polyline(source),
            ObjectAttributes::on_layer(input_layer).with_name("square"),
        )
        .unwrap();
    document
        .select_objects([source_id], SelectionMode::Replace)
        .unwrap();

    registry
        .execute(&mut document, "FitCurve Tolerance=0.01 AngleTolerance=5")
        .unwrap();
    assert_eq!(document.objects().len(), 1);
    let output = document.object(source_id).unwrap();
    assert_eq!(output.attributes().layer_id(), input_layer);
    assert_eq!(output.attributes().name(), Some("square"));
    assert!(document.is_selected(source_id));
    let Geometry::NurbsCurve(curve) = output.geometry() else {
        panic!("FitCrv replacement must be a NURBS curve")
    };
    assert_eq!(curve.control_points().len(), 13);
    assert!(curve.is_closed().unwrap());

    registry.execute(&mut document, "Undo").unwrap();
    assert!(matches!(
        document.object(source_id).unwrap().geometry(),
        Geometry::Polyline(_)
    ));
}

#[test]
fn fit_curve_rejects_invalid_options_and_missing_curve_selection_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let point_id = document
        .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
        .unwrap();
    document
        .select_objects([point_id], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "FitCrv"),
        Err(CommandError::NoFitCurves)
    ));

    let line_id = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(1.0, 0.0, 0.0).unwrap(),
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .select_objects([line_id], SelectionMode::Replace)
        .unwrap();
    for command in [
        "FitCrv Degree=0",
        "FitCrv Degree=12",
        "FitCrv Degree=nope",
        "FitCrv Tolerance=0",
        "FitCrv Tolerance=-1",
        "FitCrv AngleTolerance=-1",
        "FitCrv AngleTolerance=181",
        "FitCrv DeleteInput=Maybe",
        "FitCrv OutputLayer=Other",
        "FitCrv Degree=3 Degree=4",
        "FitCrv extra",
    ] {
        let object_count = document.objects().len();
        let history = document.undo_label().map(str::to_owned);
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(document.objects().len(), object_count, "{command}");
        assert_eq!(document.undo_label(), history.as_deref(), "{command}");
    }
}

#[test]
fn rebuild_curve_adds_fixed_uniform_output_on_the_input_layer() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let input_layer = document
        .add_layer("Rebuild source", ColorRgb::new(25, 50, 75))
        .unwrap();
    let source_id = document
        .add_geometry_with_attributes(
            Geometry::Line(
                LineSegment::try_new(
                    Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                    Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                    document.tolerance(),
                )
                .unwrap(),
            ),
            ObjectAttributes::on_layer(input_layer).with_name("source"),
        )
        .unwrap();
    document
        .select_objects([source_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
            registry
                .execute(
                    &mut document,
                    "Rebuild PointCount=6 Degree=3 PreserveTangents=Yes DeleteInput=No OutputLayer=InputObject",
                )
                .unwrap(),
            "Rebuilt 1 curve(s) as non-rational degree-3 NURBS with 6 requested point(s), retaining the input(s) on the input-object layer and preserving eligible open-curve end tangents"
        );
    assert_eq!(document.objects().len(), 2);
    assert!(matches!(
        document.object(source_id).unwrap().geometry(),
        Geometry::Line(_)
    ));
    let output = document.objects().last().unwrap();
    assert_eq!(output.attributes().layer_id(), input_layer);
    let Geometry::NurbsCurve(curve) = output.geometry() else {
        panic!("Rebuild must create a NURBS curve")
    };
    assert_eq!(curve.degree(), 3);
    assert_eq!(curve.control_points().len(), 6);
    assert_eq!(curve.domain(), 0.0..=3.0);
    assert!(!curve.is_rational());
    assert_eq!(document.undo_label(), Some("Rebuild"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 1);
    assert!(document.is_selected(source_id));
}

#[test]
fn rebuild_curve_replaces_closed_input_with_periodic_output() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let input_layer = document
        .add_layer("Closed rebuild", ColorRgb::new(80, 90, 100))
        .unwrap();
    let square = Polyline3::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 4.0, 0.0).unwrap(),
            Point3::try_new(0.0, 4.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        ],
        document.tolerance(),
    )
    .unwrap();
    let source_id = document
        .add_geometry_with_attributes(
            Geometry::Polyline(square),
            ObjectAttributes::on_layer(input_layer).with_name("square"),
        )
        .unwrap();
    document
        .select_objects([source_id], SelectionMode::Replace)
        .unwrap();

    registry
        .execute(&mut document, "RebuildCrv Points=8 Degree=3")
        .unwrap();
    assert_eq!(document.objects().len(), 1);
    let output = document.object(source_id).unwrap();
    assert_eq!(output.attributes().layer_id(), input_layer);
    assert_eq!(output.attributes().name(), Some("square"));
    let Geometry::NurbsCurve(curve) = output.geometry() else {
        panic!("closed Rebuild replacement must be a NURBS curve")
    };
    assert_eq!(curve.degree(), 3);
    assert_eq!(curve.control_points().len(), 11);
    assert!(curve.is_periodic());
    assert!(curve.is_closed().unwrap());

    registry.execute(&mut document, "Undo").unwrap();
    assert!(matches!(
        document.object(source_id).unwrap().geometry(),
        Geometry::Polyline(_)
    ));
}

#[test]
fn rebuild_curve_rejects_invalid_options_and_missing_selection_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let point_id = document
        .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
        .unwrap();
    document
        .select_objects([point_id], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "Rebuild"),
        Err(CommandError::NoRebuildCurves)
    ));

    let line_id = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(1.0, 0.0, 0.0).unwrap(),
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .select_objects([line_id], SelectionMode::Replace)
        .unwrap();
    for command in [
        "Rebuild Degree=0",
        "Rebuild Degree=12",
        "Rebuild Degree=nope",
        "Rebuild PointCount=0",
        "Rebuild PointCount=1001",
        "Rebuild PointCount=nope",
        "Rebuild PreserveTangents=Maybe",
        "Rebuild DeleteInput=Maybe",
        "Rebuild OutputLayer=Other",
        "Rebuild Degree=3 Degree=4",
        "Rebuild extra",
    ] {
        let object_count = document.objects().len();
        let history = document.undo_label().map(str::to_owned);
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(document.objects().len(), object_count, "{command}");
        assert_eq!(document.undo_label(), history.as_deref(), "{command}");
    }

    registry
        .execute(
            &mut document,
            "Rebuild PointCount=3 Degree=3 DeleteInput=No",
        )
        .unwrap();
    let Geometry::NurbsCurve(curve) = document.objects().last().unwrap().geometry() else {
        panic!("adjusted Rebuild output must be a NURBS curve")
    };
    assert_eq!(curve.control_points().len(), 4);
    registry.execute(&mut document, "Undo").unwrap();
}
